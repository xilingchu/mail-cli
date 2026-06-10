# mail-cli

A terminal email client written in Rust. Supports reading and sending email via IMAP/SMTP with an interactive TUI or direct command-line access.

## Features

- Interactive TUI: inbox browsing, search, compose, reply, delete
- Non-interactive CLI mode for scripting and quick access
- Auto-detects server settings for 163/126/yeah.net, Gmail, and QQ Mail
- Optional HTTP proxy support (configurable per-account)

## Installation

```bash
cargo build --release
# binary at target/release/mail
```

## First Run

On first launch, the setup wizard prompts for:

1. Email address and password / app password
2. IMAP/SMTP server (auto-detected for common providers)
3. HTTP proxy (optional, e.g. `http://127.0.0.1:7890`)

Config is saved to `~/.config/mail-cli/config.json`.

> **163/126/yeah.net**: enable IMAP in webmail settings → POP3/SMTP/IMAP before use.  
> **Gmail**: enable IMAP and generate an App Password under your Google account security settings.

## Usage

### Interactive mode

```bash
mail
```

Navigate with arrow keys, Enter to select.

### Command-line mode

```bash
mail inbox                   # latest 50 emails with UIDs
mail search <query>          # search emails
mail read <uid>              # read full email by UID (marks as read)
mail help                    # show usage
```

Short aliases: `mail i`, `mail s <query>`, `mail r <uid>`, `mail --help`.

**Search syntax** — multiple terms are ANDed:

| Query | Matches |
|-------|---------|
| `from:alice` | sender contains "alice" |
| `to:bob` | recipient contains "bob" |
| `subject:invoice` | subject contains "invoice" |
| `is:unread` | unread emails |
| `is:read` | read emails |
| `meeting` | free-text search |

```bash
mail s "is:unread from:alice"      # unread emails from alice
mail s "subject:invoice is:unread" # unread emails with invoice in subject
```

## Configuration

`~/.config/mail-cli/config.json`:

```json
{
  "email": "you@example.com",
  "app_password": "...",
  "imap_host": "imap.example.com",
  "imap_port": 993,
  "smtp_host": "smtp.example.com",
  "smtp_port": 465,
  "proxy": "http://127.0.0.1:7890"
}
```

`proxy` is optional — omit or set to `null` for direct connection. Port 465 uses implicit TLS; port 587 uses STARTTLS.

## Provider reference

| Provider | IMAP | SMTP | Port |
|----------|------|------|------|
| 163 / 126 / yeah.net | imap.163.com | smtp.163.com | 993 / 465 |
| Gmail | imap.gmail.com | smtp.gmail.com | 993 / 587 |
| QQ Mail | imap.qq.com | smtp.qq.com | 993 / 465 |
