use anyhow::Result;
use dialoguer::{Input, Password};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub email: String,
    pub app_password: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    /// HTTP 代理，格式 http://host:port，留空表示直连
    #[serde(default)]
    pub proxy: Option<String>,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mail-cli")
        .join("config.json")
}

pub fn load_or_setup() -> Result<Config> {
    let path = config_path();

    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        return Ok(serde_json::from_str(&content)?);
    }

    println!("首次使用，请配置邮箱信息。");
    println!("163 用户：需在网页版邮箱 → 设置 → POP3/SMTP/IMAP 中开启 IMAP 服务");
    println!("Gmail 用户：需开启 IMAP 并生成应用专用密码");
    println!();

    let email: String = Input::new().with_prompt("邮箱地址").interact_text()?;

    let raw_pass: String = Password::new()
        .with_prompt("密码 / 授权码")
        .interact()?;
    let app_password = raw_pass.trim().to_string();

    let (imap_host, imap_port, smtp_host, smtp_port) =
        if email.ends_with("@163.com") || email.ends_with("@126.com") || email.ends_with("@yeah.net") {
            ("imap.163.com".into(), 993u16, "smtp.163.com".into(), 465u16)
        } else if email.ends_with("@gmail.com") || email.ends_with("@googlemail.com") {
            ("imap.gmail.com".into(), 993, "smtp.gmail.com".into(), 587)
        } else if email.ends_with("@qq.com") {
            ("imap.qq.com".into(), 993, "smtp.qq.com".into(), 465)
        } else {
            let ih: String = Input::new().with_prompt("IMAP 服务器").interact_text()?;
            let ip: u16 = Input::new().with_prompt("IMAP 端口").default(993u16).interact_text()?;
            let sh: String = Input::new().with_prompt("SMTP 服务器").interact_text()?;
            let sp: u16 = Input::new().with_prompt("SMTP 端口").default(465u16).interact_text()?;
            (ih, ip, sh, sp)
        };

    let proxy_raw: String = Input::new()
        .with_prompt("HTTP 代理（可选，格式 http://host:port，留空跳过）")
        .allow_empty(true)
        .interact_text()?;
    let proxy = if proxy_raw.trim().is_empty() { None } else { Some(proxy_raw.trim().to_string()) };

    let config = Config { email, app_password, imap_host, imap_port, smtp_host, smtp_port, proxy };

    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, serde_json::to_string_pretty(&config)?)?;
    println!("配置已保存到 {}", path.display());
    println!();

    Ok(config)
}
