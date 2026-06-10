use anyhow::Result;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use crate::config::Config;

pub fn send(
    config: &Config,
    to: &str,
    subject: &str,
    body: &str,
    in_reply_to: Option<&str>,
) -> Result<()> {
    let mut builder = Message::builder()
        .from(config.email.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN);

    if let Some(id) = in_reply_to {
        builder = builder.in_reply_to(id.to_string()).references(id.to_string());
    }

    let email = builder.body(body.to_string())?;

    let creds = Credentials::new(config.email.clone(), config.app_password.clone());

    // 465 = 隐式 TLS (SMTPS)，587 = STARTTLS
    let mailer = if config.smtp_port == 465 {
        SmtpTransport::relay(&config.smtp_host)?
            .port(config.smtp_port)
            .credentials(creds)
            .build()
    } else {
        SmtpTransport::starttls_relay(&config.smtp_host)?
            .port(config.smtp_port)
            .credentials(creds)
            .build()
    };

    mailer.send(&email)?;
    Ok(())
}
