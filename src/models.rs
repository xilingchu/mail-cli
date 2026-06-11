use mailparse::{MailHeaderMap, ParsedMail};

#[derive(Debug, Clone)]
pub struct EmailSummary {
    pub uid: u32,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub is_unread: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedEmail {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub date: String,
    pub message_id: Option<String>,
    pub body: String,
}

impl ParsedEmail {
    pub fn from_parsed(mail: &ParsedMail) -> Self {
        Self {
            from: mail.headers.get_first_value("From").unwrap_or_default(),
            to: mail.headers.get_first_value("To").unwrap_or_default(),
            subject: mail.headers.get_first_value("Subject")
                .unwrap_or_else(|| "(无主题)".to_string()),
            date: mail.headers.get_first_value("Date").unwrap_or_default(),
            message_id: mail.headers.get_first_value("Message-ID"),
            body: extract_body(mail),
        }
    }
}

/// Get HTML body bytes and decode as UTF-8, bypassing mailparse's charset
/// conversion which can misinterpret UTF-8 content as Latin-1.
fn get_html_body(part: &ParsedMail) -> Option<String> {
    let raw = part.get_body_raw().ok()?;
    if raw.is_empty() {
        return None;
    }
    // Try strict UTF-8 first; fall back to lossy UTF-8.
    let text = String::from_utf8(raw)
        .unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned());
    Some(html_to_text(&text))
}

fn extract_body(mail: &ParsedMail) -> String {
    if mail.subparts.is_empty() {
        if mail.ctype.mimetype.starts_with("text/plain") {
            return mail.get_body().unwrap_or_default();
        }
        if mail.ctype.mimetype.starts_with("text/html") {
            return get_html_body(mail).unwrap_or_default();
        }
        return String::new();
    }
    for part in &mail.subparts {
        if part.ctype.mimetype.starts_with("text/plain") {
            if let Ok(body) = part.get_body() {
                if !body.is_empty() {
                    return body;
                }
            }
        }
    }
    // Recurse into nested multipart structures first
    for part in &mail.subparts {
        let body = extract_body(part);
        if !body.is_empty() {
            return body;
        }
    }
    // Last resort: find any text/html part
    for part in &mail.subparts {
        if part.ctype.mimetype.starts_with("text/html") {
            if let Some(html) = get_html_body(part) {
                if !html.is_empty() {
                    return html;
                }
            }
        }
    }
    String::new()
}

fn remove_tag_blocks(html: &str, tag: &str) -> String {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let lower = html.to_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut pos = 0;
    loop {
        let start = lower[pos..].find(&open).map(|i| pos + i);
        match start {
            None => { out.push_str(&html[pos..]); break; }
            Some(s) => {
                out.push_str(&html[pos..s]);
                let end = lower[s..].find(&close).map(|i| s + i + close.len());
                pos = end.unwrap_or(html.len());
            }
        }
    }
    out
}

fn html_to_text(html: &str) -> String {
    // Remove <style>…</style> and <script>…</script> blocks entirely (case-insensitive)
    let s = remove_tag_blocks(html, "style");
    let s = remove_tag_blocks(&s, "script");

    // Replace block-level tags with newlines before stripping
    let mut s = s;
    for tag in &["</p>", "</div>", "</tr>", "<br>", "<br/>", "<br />", "</h1>", "</h2>", "</h3>", "</h4>", "<li>"] {
        s = s.replace(tag, &format!("{}\n", tag));
    }
    // Strip all remaining HTML tags
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    // Decode common HTML entities
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'");
    // Decode numeric entities like &#160; or &#x00A0;
    let out = decode_numeric_entities(&out);
    // Collapse runs of blank lines into a single blank line
    let mut result = String::new();
    let mut blank_run = 0u32;
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                result.push('\n');
            }
        } else {
            blank_run = 0;
            result.push_str(trimmed);
            result.push('\n');
        }
    }
    result.trim().to_string()
}

fn decode_numeric_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find("&#") {
        out.push_str(&rest[..amp]);
        rest = &rest[amp + 2..]; // skip "&#"
        let (hex, rest2) = if rest.starts_with('x') || rest.starts_with('X') {
            (true, &rest[1..])
        } else {
            (false, rest)
        };
        if let Some(semi) = rest2.find(';') {
            let num_str = &rest2[..semi];
            let code_point = if hex {
                u32::from_str_radix(num_str, 16).ok()
            } else {
                num_str.parse::<u32>().ok()
            };
            if let Some(ch) = code_point.and_then(char::from_u32) {
                out.push(ch);
                rest = &rest2[semi + 1..];
                continue;
            }
        }
        // Not a valid entity — emit the "&#" literally and continue
        out.push_str("&#");
    }
    out.push_str(rest);
    out
}
