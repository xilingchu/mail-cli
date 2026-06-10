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

fn extract_body(mail: &ParsedMail) -> String {
    if mail.subparts.is_empty() {
        if mail.ctype.mimetype.starts_with("text/plain") {
            return mail.get_body().unwrap_or_default();
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
    for part in &mail.subparts {
        let body = extract_body(part);
        if !body.is_empty() {
            return body;
        }
    }
    String::new()
}
