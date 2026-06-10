use anyhow::{Context, Result};
use imap::Session;
use mailparse::MailHeaderMap;
use native_tls::TlsStream;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::config::Config;
use crate::models::{EmailSummary, ParsedEmail};

pub type ImapSession = Session<TlsStream<TcpStream>>;

fn open_tcp(config: &Config) -> Result<TcpStream> {
    if let Some(proxy) = &config.proxy {
        let proxy_addr = proxy.strip_prefix("http://").unwrap_or(proxy);
        let mut tcp = TcpStream::connect(proxy_addr)
            .context(format!("无法连接到代理 {proxy}"))?;
        tcp.set_read_timeout(Some(Duration::from_secs(8)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(8)))?;

        let req = format!(
            "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n\r\n",
            config.imap_host, config.imap_port,
            config.imap_host, config.imap_port,
        );
        tcp.write_all(req.as_bytes()).context("发送代理请求失败")?;

        let mut response = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            tcp.read_exact(&mut byte).context("读取代理响应失败")?;
            response.push(byte[0]);
            if response.ends_with(b"\r\n\r\n") {
                break;
            }
            if response.len() > 4096 {
                return Err(anyhow::anyhow!("代理响应过长"));
            }
        }
        let resp = String::from_utf8_lossy(&response);
        if !resp.contains("200") {
            return Err(anyhow::anyhow!(
                "代理拒绝连接: {}",
                resp.lines().next().unwrap_or("").trim()
            ));
        }
        Ok(tcp)
    } else {
        use std::net::ToSocketAddrs;
        let addr = format!("{}:{}", config.imap_host, config.imap_port)
            .to_socket_addrs()
            .context("DNS 解析失败")?
            .next()
            .context("无法解析服务器地址")?;
        TcpStream::connect_timeout(&addr, Duration::from_secs(100))
            .context(format!("无法连接到 {}，请检查网络连接", config.imap_host))
    }
}

pub fn connect(config: &Config) -> Result<ImapSession> {
    let tcp = open_tcp(config)?;
    tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(30)))?;

    let tls = ::native_tls::TlsConnector::builder().build()?;
    let tls_stream = tls.connect(&config.imap_host, tcp)?;

    let mut client = imap::Client::new(tls_stream);
    client.read_greeting().context("读取服务器响应失败")?;

    let mut session = client
        .login(&config.email, &config.app_password)
        .map_err(|(e, _)| anyhow::anyhow!("IMAP 登录失败: {e}\n请检查邮箱地址和授权码"))?;

    // 163/126/yeah 要求登录后发送 IMAP ID，否则后续操作会被拒绝
    let needs_id = config.email.ends_with("@163.com")
        || config.email.ends_with("@126.com")
        || config.email.ends_with("@yeah.net");
    if needs_id {
        session.run_command_and_read_response(
            r#"ID ("name" "mail-cli" "version" "1.0.0" "vendor" "mail-cli" "support-email" "mail-cli@example.com")"#,
        )?;
    }

    Ok(session)
}

pub fn list_inbox(session: &mut ImapSession, count: u32) -> Result<Vec<EmailSummary>> {
    let mailbox = session.select("INBOX")?;
    let total = mailbox.exists;
    if total == 0 {
        return Ok(vec![]);
    }

    let start = total.saturating_sub(count.saturating_sub(1)).max(1);
    let msgs = session.fetch(format!("{start}:{total}"), "(UID FLAGS RFC822.HEADER)")?;

    let mut summaries: Vec<EmailSummary> = msgs.iter().filter_map(parse_summary).collect();
    summaries.sort_by(|a, b| b.uid.cmp(&a.uid));
    Ok(summaries)
}

pub fn search_messages(session: &mut ImapSession, query: &str) -> Result<Vec<EmailSummary>> {
    session.select("INBOX")?;

    let imap_query = translate_query(query);
    let uids = session.uid_search(&imap_query)?;
    if uids.is_empty() {
        return Ok(vec![]);
    }

    let mut uid_list: Vec<u32> = uids.into_iter().collect();
    uid_list.sort_unstable_by(|a, b| b.cmp(a));
    uid_list.truncate(50);

    let uid_set = uid_list.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
    let msgs = session.uid_fetch(&uid_set, "(UID FLAGS RFC822.HEADER)")?;

    let mut summaries: Vec<EmailSummary> = msgs.iter().filter_map(parse_summary).collect();
    summaries.sort_by(|a, b| b.uid.cmp(&a.uid));
    Ok(summaries)
}

pub fn fetch_full(session: &mut ImapSession, uid: u32) -> Result<ParsedEmail> {
    session.select("INBOX")?;
    let msgs = session.uid_fetch(uid.to_string(), "RFC822")?;
    let msg = msgs.first().context("邮件不存在")?;
    let raw = msg.body().context("邮件内容为空")?;
    let parsed = mailparse::parse_mail(raw)?;
    Ok(ParsedEmail::from_parsed(&parsed))
}

pub fn mark_read(session: &mut ImapSession, uid: u32) -> Result<()> {
    session.uid_store(uid.to_string(), "+FLAGS (\\Seen)")?;
    Ok(())
}

pub fn delete_message(session: &mut ImapSession, uid: u32) -> Result<()> {
    session.uid_store(uid.to_string(), "+FLAGS (\\Deleted)")?;
    session.expunge()?;
    Ok(())
}

fn parse_summary(msg: &imap::types::Fetch) -> Option<EmailSummary> {
    let uid = msg.uid?;
    let raw = msg.header()?;
    let (headers, _) = mailparse::parse_headers(raw).ok()?;

    let from = headers.get_first_value("From").unwrap_or_else(|| "Unknown".into());
    let subject = headers.get_first_value("Subject").unwrap_or_else(|| "(无主题)".into());
    let date = headers.get_first_value("Date").unwrap_or_default();
    let is_unread = !msg.flags().iter().any(|f| matches!(f, imap::types::Flag::Seen));

    Some(EmailSummary { uid, from, subject, date, is_unread })
}

fn translate_query(query: &str) -> String {
    let mut parts = Vec::new();
    for token in query.trim().split_whitespace() {
        match token.to_ascii_lowercase().as_str() {
            "is:unread" => parts.push("UNSEEN".into()),
            "is:read"   => parts.push("SEEN".into()),
            _ => {
                if let Some(v) = token.strip_prefix("from:")         { parts.push(format!("FROM \"{v}\"")); }
                else if let Some(v) = token.strip_prefix("to:")      { parts.push(format!("TO \"{v}\"")); }
                else if let Some(v) = token.strip_prefix("subject:") { parts.push(format!("SUBJECT \"{v}\"")); }
                else                                                  { parts.push(format!("TEXT \"{token}\"")); }
            }
        }
    }
    if parts.is_empty() { "ALL".into() } else { parts.join(" ") }
}
