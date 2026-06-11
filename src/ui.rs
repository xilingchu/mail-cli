use console::{style, Term};
use dialoguer::{Confirm, Input, Select};

use crate::models::{EmailSummary, ParsedEmail};

pub fn clear_screen() {
    let _ = Term::stdout().clear_screen();
}

pub fn print_header(title: &str) {
    println!("{}", style(format!("══ {title} ══")).cyan().bold());
}

pub fn print_list(emails: &[EmailSummary], page: usize, per_page: usize) {
    let start = page * per_page;
    let end = (start + per_page).min(emails.len());

    println!();
    println!(
        "{:>3}  {:<2}  {:<28}  {:<42}  {}",
        style("#").dim(),
        style("").dim(),
        style("发件人").dim(),
        style("主题").dim(),
        style("日期").dim(),
    );
    println!("{}", style("─".repeat(95)).dim());

    for (i, email) in emails[start..end].iter().enumerate() {
        let n = start + i + 1;
        let dot = if email.is_unread { style("●").cyan().to_string() } else { " ".into() };
        let from = fit(&email.from, 28);
        let subj = if email.is_unread {
            style(fit(&email.subject, 42)).bold().to_string()
        } else {
            style(fit(&email.subject, 42)).dim().to_string()
        };
        println!("{n:>3}  {dot}  {from:<28}  {subj:<42}  {}", style(fit(&email.date, 22)).dim());
    }

    println!("{}", style("─".repeat(95)).dim());
    println!("共 {} 封，显示 {}-{}", emails.len(), start + 1, end);
}

pub fn print_email(email: &ParsedEmail) {
    println!();
    println!("{}", style("─".repeat(80)).dim());
    println!("{}: {}", style("From   ").cyan(), email.from);
    println!("{}: {}", style("To     ").cyan(), email.to);
    println!("{}: {}", style("Subject").cyan(), style(&email.subject).bold());
    println!("{}: {}", style("Date   ").cyan(), email.date);
    println!("{}", style("─".repeat(80)).dim());
    println!();
    println!("{}", email.body.trim());
    println!();
}

pub fn main_menu() -> usize {
    println!();
    Select::new()
        .with_prompt("主菜单")
        .items(&["📥  收件箱", "🔍  搜索", "✏️   写邮件", "🚪  退出"])
        .default(0)
        .interact()
        .unwrap_or(3)
}

pub fn list_menu(has_prev: bool, has_next: bool) -> &'static str {
    let mut items: Vec<&str> = vec!["选择邮件"];
    if has_next { items.push("下一页"); }
    if has_prev { items.push("上一页"); }
    items.push("← 返回");
    let i = Select::new()
        .with_prompt("操作")
        .items(&items)
        .default(0)
        .interact()
        .unwrap_or(items.len() - 1);
    items[i]
}

pub fn email_menu(summary: &EmailSummary) -> usize {
    println!("\n{}", style(fit(&summary.subject, 70)).bold());
    Select::new()
        .with_prompt("操作")
        .items(&["📖  阅读", "↩️   回复", "🗑️   删除", "← 返回"])
        .default(0)
        .interact()
        .unwrap_or(3)
}

pub fn ask_number(max: usize) -> Option<usize> {
    let s: String = Input::new()
        .with_prompt(format!("选择编号 (1-{max}，回车取消)"))
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();
    s.trim().parse::<usize>().ok().filter(|&n| n >= 1 && n <= max)
}

pub fn ask(prompt: &str) -> String {
    Input::new().with_prompt(prompt).interact_text().unwrap_or_default()
}

pub fn ask_search() -> String {
    ask("搜索 (支持 from: to: subject: is:unread on:YYYY-MM-DD after:YYYY-MM-DD before:YYYY-MM-DD)")
}

pub fn ask_body() -> String {
    println!("正文 {}:", style("(空行输入 '.' 结束)").dim());
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).ok();
        let line = line.trim_end_matches('\n').trim_end_matches('\r').to_string();
        if line == "." { break; }
        lines.push(line);
    }
    lines.join("\n")
}

pub fn confirm(prompt: &str) -> bool {
    Confirm::new().with_prompt(prompt).default(false).interact().unwrap_or(false)
}

fn fit(s: &str, max: usize) -> String {
    let mut width = 0usize;
    let mut out = String::new();
    for ch in s.chars() {
        let w = if (ch as u32) > 0x2E7F { 2 } else { 1 };
        if width + w > max {
            out.push('…');
            break;
        }
        width += w;
        out.push(ch);
    }
    out
}
