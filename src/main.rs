mod config;
mod imap_client;
mod models;
mod smtp_client;
mod ui;

use anyhow::{Context, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use imap_client::ImapSession;
use models::EmailSummary;

const PER_PAGE: usize = 20;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = if args.is_empty() { run() } else { run_cmd(&args) };
    if let Err(e) = result {
        eprintln!("{} {e}", style("错误:").red().bold());
        std::process::exit(1);
    }
}

fn run_cmd(args: &[String]) -> Result<()> {
    let config = config::load_or_setup()?;
    let pb = spinner("连接邮件服务器…");
    let mut session = imap_client::connect(&config)?;
    pb.finish_and_clear();

    match args[0].as_str() {
        "search" | "s" => {
            if args.len() < 2 {
                anyhow::bail!("用法: mail search <搜索词>\n支持 from: to: subject: is:unread is:read");
            }
            let query = args[1..].join(" ");
            let emails = imap_client::search_messages(&mut session, &query)?;
            if emails.is_empty() {
                println!("没有找到邮件。");
            } else {
                println!("{:>6}  {:<2}  {:<32}  {:<48}  {}", "UID", "", "发件人", "主题", "日期");
                println!("{}", "─".repeat(100));
                for e in &emails {
                    let dot = if e.is_unread { "●" } else { " " };
                    println!("{:>6}  {}  {:<32}  {:<48}  {}", e.uid, dot, e.from, e.subject, e.date);
                }
            }
        }
        "read" | "r" => {
            if args.len() < 2 {
                anyhow::bail!("用法: mail read <uid>");
            }
            let uid: u32 = args[1].parse().context("UID 必须是数字")?;
            let email = imap_client::fetch_full(&mut session, uid)?;
            let _ = imap_client::mark_read(&mut session, uid);
            println!("From:    {}", email.from);
            println!("To:      {}", email.to);
            println!("Subject: {}", email.subject);
            println!("Date:    {}", email.date);
            println!("{}", "─".repeat(72));
            println!("{}", email.body.trim());
        }
        "inbox" | "i" => {
            let emails = imap_client::list_inbox(&mut session, 50)?;
            if emails.is_empty() {
                println!("收件箱为空。");
            } else {
                println!("{:>6}  {:<2}  {:<32}  {:<48}  {}", "UID", "", "发件人", "主题", "日期");
                println!("{}", "─".repeat(100));
                for e in &emails {
                    let dot = if e.is_unread { "●" } else { " " };
                    println!("{:>6}  {}  {:<32}  {:<48}  {}", e.uid, dot, e.from, e.subject, e.date);
                }
            }
        }
        cmd => anyhow::bail!(
            "未知命令: {cmd}\n用法:\n  mail inbox\n  mail search <搜索词>\n  mail read <uid>"
        ),
    }

    let _ = session.logout();
    Ok(())
}

fn run() -> Result<()> {
    ui::clear_screen();
    println!("{}", style("Mail CLI").cyan().bold());

    let config = config::load_or_setup()?;

    let pb = spinner("连接邮件服务器…");
    let mut session = imap_client::connect(&config)?;
    pb.finish_and_clear();
    println!("{}", style(format!("已登录: {}", config.email)).green());

    loop {
        ui::clear_screen();
        ui::print_header("Mail CLI");
        match ui::main_menu() {
            0 => inbox(&mut session)?,
            1 => search(&mut session)?,
            2 => compose(&config, &mut session, None, None)?,
            _ => { println!("再见！"); break; }
        }
    }

    let _ = session.logout();
    Ok(())
}

fn inbox(session: &mut ImapSession) -> Result<()> {
    let pb = spinner("加载收件箱…");
    let emails = imap_client::list_inbox(session, 100)?;
    pb.finish_and_clear();

    if emails.is_empty() {
        println!("收件箱为空。");
        pause();
        return Ok(());
    }
    email_list(session, emails, None)
}

fn search(session: &mut ImapSession) -> Result<()> {
    let query = ui::ask_search();
    if query.is_empty() { return Ok(()); }

    let pb = spinner("搜索中…");
    let emails = imap_client::search_messages(session, &query)?;
    pb.finish_and_clear();

    if emails.is_empty() {
        println!("没有找到邮件。");
        pause();
        return Ok(());
    }
    email_list(session, emails, Some(&query))
}

fn email_list(
    session: &mut ImapSession,
    emails: Vec<EmailSummary>,
    title: Option<&str>,
) -> Result<()> {
    let total_pages = (emails.len() + PER_PAGE - 1) / PER_PAGE;
    let mut page = 0usize;

    loop {
        ui::clear_screen();
        ui::print_header(title.unwrap_or("收件箱"));
        ui::print_list(&emails, page, PER_PAGE);

        match ui::list_menu(page > 0, page + 1 < total_pages) {
            "选择邮件" => {
                let start = page * PER_PAGE;
                let end = (start + PER_PAGE).min(emails.len());
                if let Some(n) = ui::ask_number(end - start) {
                    read_email(session, &emails[start + n - 1])?;
                }
            }
            "下一页" => page += 1,
            "上一页" => { if page > 0 { page -= 1; } }
            _ => break,
        }
    }
    Ok(())
}

fn read_email(session: &mut ImapSession, summary: &EmailSummary) -> Result<()> {
    ui::clear_screen();
    ui::print_header("阅读邮件");

    let pb = spinner("加载邮件…");
    let email = imap_client::fetch_full(session, summary.uid)?;
    pb.finish_and_clear();

    ui::print_email(&email);
    let _ = imap_client::mark_read(session, summary.uid);

    match ui::email_menu(summary) {
        1 => {
            // reply
            let to = email.from.clone();
            let subject = if email.subject.to_lowercase().starts_with("re:") {
                email.subject.clone()
            } else {
                format!("Re: {}", email.subject)
            };
            compose(&get_config(), session, Some((&to, &subject)), email.message_id.as_deref())?;
        }
        2 => {
            if ui::confirm("确定删除此邮件？") {
                let pb = spinner("删除中…");
                imap_client::delete_message(session, summary.uid)?;
                pb.finish_and_clear();
                println!("{}", style("已删除").green());
                pause();
            }
        }
        _ => {}
    }
    Ok(())
}

fn compose(
    config: &config::Config,
    _session: &mut ImapSession,
    prefill: Option<(&str, &str)>,
    in_reply_to: Option<&str>,
) -> Result<()> {
    ui::clear_screen();
    ui::print_header("写邮件");

    let (to, subject) = if let Some((t, s)) = prefill {
        (t.to_string(), s.to_string())
    } else {
        (ui::ask("收件人"), ui::ask("主题"))
    };

    println!();
    let body = ui::ask_body();
    if body.is_empty() {
        println!("{}", style("内容为空，已取消").dim());
        pause();
        return Ok(());
    }

    println!("\n{}", style("─── 预览 ─────────────────────────").dim());
    println!("To: {to}");
    println!("Subject: {subject}");
    println!();
    println!("{body}");

    if !ui::confirm("\n确认发送？") {
        println!("已取消。");
        pause();
        return Ok(());
    }

    let pb = spinner("发送中…");
    let result = smtp_client::send(config, &to, &subject, &body, in_reply_to);
    pb.finish_and_clear();

    match result {
        Ok(_) => println!("{}", style("发送成功！").green().bold()),
        Err(e) => println!("{} {e}", style("发送失败:").red()),
    }
    pause();
    Ok(())
}

// Load config without re-prompting (already loaded once in run())
fn get_config() -> config::Config {
    config::load_or_setup().expect("配置丢失")
}

fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

fn pause() {
    use std::io::{stdin, Read};
    println!("\n{}", style("按 Enter 继续…").dim());
    let _ = stdin().read(&mut [0u8]);
}
