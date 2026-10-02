//! Transactional e-mail (verification, password reset) behind a small provider-neutral trait.
//!
//! * [`SmtpMailer`] – production transport (IONOS or any SMTP provider), configured purely through
//!   environment / Docker secrets (`SMTP_*`). Credentials never appear in code, images or logs.
//! * [`DisabledMailer`] – used when SMTP is not configured. It never pretends to deliver.
//! * [`MemoryMailer`] – in-memory outbox for tests only.
//!
//! No mail body or recipient is ever logged; only the outcome.

use async_trait::async_trait;
use lettre::{
    message::{header::ContentType, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
#[cfg(test)]
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct OutgoingMail {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
}

#[derive(Debug)]
pub enum MailError {
    NotConfigured,
    Invalid(String),
    Transport(String),
}

impl std::fmt::Display for MailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MailError::NotConfigured => write!(f, "mail delivery is not configured"),
            MailError::Invalid(m) => write!(f, "invalid mail: {m}"),
            MailError::Transport(m) => write!(f, "mail transport error: {m}"),
        }
    }
}

impl std::error::Error for MailError {}

#[async_trait]
pub trait Mailer: Send + Sync {
    /// False for [`DisabledMailer`]: callers must not claim a mail was sent.
    fn is_configured(&self) -> bool;
    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError>;
}

// ── Disabled ────────────────────────────────────────────────────────────────

pub struct DisabledMailer;

#[async_trait]
impl Mailer for DisabledMailer {
    fn is_configured(&self) -> bool {
        false
    }
    async fn send(&self, _mail: OutgoingMail) -> Result<(), MailError> {
        Err(MailError::NotConfigured)
    }
}

// ── In-memory (tests) ───────────────────────────────────────────────────────

#[cfg(test)]
#[derive(Default)]
pub struct MemoryMailer {
    outbox: Mutex<Vec<OutgoingMail>>,
}

#[cfg(test)]
impl MemoryMailer {
    pub fn take(&self) -> Vec<OutgoingMail> {
        std::mem::take(&mut *self.outbox.lock().unwrap())
    }
    pub fn len(&self) -> usize {
        self.outbox.lock().unwrap().len()
    }
}

#[cfg(test)]
#[async_trait]
impl Mailer for MemoryMailer {
    fn is_configured(&self) -> bool {
        true
    }
    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError> {
        self.outbox.lock().unwrap().push(mail);
        Ok(())
    }
}

// ── SMTP ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMode {
    /// Implicit TLS (usually port 465).
    Tls,
    /// STARTTLS upgrade, mandatory (usually port 587).
    StartTls,
    /// Plain text. Only accepted for loopback hosts (tests / local relay).
    None,
}

#[derive(Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_email: String,
    pub from_name: String,
    pub tls: TlsMode,
}

impl std::fmt::Debug for SmtpConfig {
    // Manual impl: the password must never reach a log line through `{:?}`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username.as_ref().map(|_| "<set>"))
            .field("password", &self.password.as_ref().map(|_| "<set>"))
            .field("from_email", &self.from_email)
            .field("tls", &self.tls)
            .finish()
    }
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

impl SmtpConfig {
    /// Reads `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM_EMAIL`,
    /// `SMTP_FROM_NAME`, `SMTP_TLS_MODE` (`starttls` | `tls` | `none`). Returns `Ok(None)` when no
    /// `SMTP_HOST` is set (mail disabled) and `Err` for a half-configured / unsafe setup.
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Result<Option<SmtpConfig>, String> {
        let Some(host) = get("SMTP_HOST").filter(|h| !h.trim().is_empty()) else {
            return Ok(None);
        };
        let host = host.trim().to_string();
        let tls = match get("SMTP_TLS_MODE")
            .unwrap_or_else(|| "starttls".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "starttls" => TlsMode::StartTls,
            "tls" | "ssl" => TlsMode::Tls,
            "none" => TlsMode::None,
            other => return Err(format!("unsupported SMTP_TLS_MODE '{other}'")),
        };
        if tls == TlsMode::None && !is_loopback(&host) {
            return Err("SMTP_TLS_MODE=none is only allowed for loopback hosts".into());
        }
        let default_port = match tls {
            TlsMode::Tls => 465,
            TlsMode::StartTls => 587,
            TlsMode::None => 25,
        };
        let port = match get("SMTP_PORT") {
            Some(p) => p
                .trim()
                .parse::<u16>()
                .map_err(|_| "invalid SMTP_PORT".to_string())?,
            None => default_port,
        };
        let from_email = get("SMTP_FROM_EMAIL")
            .or_else(|| get("SMTP_USERNAME"))
            .ok_or("SMTP_FROM_EMAIL is required when SMTP_HOST is set")?;
        let username = get("SMTP_USERNAME").filter(|v| !v.is_empty());
        let password = get("SMTP_PASSWORD").filter(|v| !v.is_empty());
        if username.is_some() != password.is_some() {
            return Err("SMTP_USERNAME and SMTP_PASSWORD must be set together".into());
        }
        Ok(Some(SmtpConfig {
            host,
            port,
            username,
            password,
            from_email: from_email.trim().to_string(),
            from_name: get("SMTP_FROM_NAME").unwrap_or_else(|| "Kaspa Battle".into()),
            tls,
        }))
    }
}

pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpMailer {
    pub fn new(cfg: &SmtpConfig) -> Result<Self, String> {
        let builder = match cfg.tls {
            TlsMode::Tls => {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host).map_err(|e| e.to_string())?
            }
            TlsMode::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
                .map_err(|e| e.to_string())?,
            TlsMode::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host),
        };
        let mut builder = builder
            .port(cfg.port)
            .timeout(Some(std::time::Duration::from_secs(15)));
        if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
            builder = builder.credentials(Credentials::new(u.clone(), p.clone()));
        }
        let from = format!(
            "{} <{}>",
            cfg.from_name.replace(['<', '>', '"'], ""),
            cfg.from_email
        )
        .parse::<Mailbox>()
        .map_err(|e| format!("invalid SMTP_FROM_EMAIL: {e}"))?;
        Ok(Self {
            transport: builder.build(),
            from,
        })
    }
}

#[async_trait]
impl Mailer for SmtpMailer {
    fn is_configured(&self) -> bool {
        true
    }

    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError> {
        let to: Mailbox = mail
            .to
            .parse()
            .map_err(|e| MailError::Invalid(format!("recipient: {e}")))?;
        let builder = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(mail.subject);
        let message = match mail.html {
            Some(html) => builder.multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(mail.text),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html),
                    ),
            ),
            None => builder.header(ContentType::TEXT_PLAIN).body(mail.text),
        }
        .map_err(|e| MailError::Invalid(e.to_string()))?;
        self.transport
            .send(message)
            .await
            .map(|_| ())
            .map_err(|e| MailError::Transport(e.to_string()))
    }
}

/// Builds the production mailer from the environment / Docker secrets.
pub fn mailer_from_env(
    get: impl Fn(&str) -> Option<String>,
) -> (std::sync::Arc<dyn Mailer>, Option<String>) {
    match SmtpConfig::from_env(get) {
        Ok(Some(cfg)) => match SmtpMailer::new(&cfg) {
            Ok(m) => (std::sync::Arc::new(m), None),
            Err(e) => (std::sync::Arc::new(DisabledMailer), Some(e)),
        },
        Ok(None) => (std::sync::Arc::new(DisabledMailer), None),
        Err(e) => (std::sync::Arc::new(DisabledMailer), Some(e)),
    }
}

// ── Templates ───────────────────────────────────────────────────────────────

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn verification_mail(to: &str, username: &str, link: &str) -> OutgoingMail {
    let text = format!(
        "Hallo {username},\n\nwillkommen bei Kaspa Battle! Bitte bestätige deine E-Mail-Adresse:\n\n{link}\n\n\
         Der Link ist 24 Stunden gültig und kann nur einmal verwendet werden.\n\
         Falls du dich nicht registriert hast, kannst du diese E-Mail ignorieren.\n\n— Kaspa Battle"
    );
    let html = format!(
        "<p>Hallo {u},</p><p>willkommen bei Kaspa Battle! Bitte bestätige deine E-Mail-Adresse:</p>\
         <p><a href=\"{l}\">E-Mail-Adresse bestätigen</a></p>\
         <p>Der Link ist 24 Stunden gültig und kann nur einmal verwendet werden. \
         Falls du dich nicht registriert hast, kannst du diese E-Mail ignorieren.</p><p>— Kaspa Battle</p>",
        u = html_escape(username),
        l = html_escape(link)
    );
    OutgoingMail {
        to: to.into(),
        subject: "Kaspa Battle: E-Mail-Adresse bestätigen".into(),
        text,
        html: Some(html),
    }
}

pub fn password_reset_mail(to: &str, username: &str, link: &str) -> OutgoingMail {
    let text = format!(
        "Hallo {username},\n\nfür dein Kaspa-Battle-Konto wurde das Zurücksetzen des Passworts angefordert:\n\n{link}\n\n\
         Der Link ist 60 Minuten gültig und kann nur einmal verwendet werden. Nach der Änderung werden alle \
         bestehenden Anmeldungen beendet.\nFalls du das nicht warst, ignoriere diese E-Mail – dein Passwort bleibt unverändert.\n\n— Kaspa Battle"
    );
    let html = format!(
        "<p>Hallo {u},</p><p>für dein Kaspa-Battle-Konto wurde das Zurücksetzen des Passworts angefordert:</p>\
         <p><a href=\"{l}\">Neues Passwort festlegen</a></p>\
         <p>Der Link ist 60 Minuten gültig und kann nur einmal verwendet werden. Nach der Änderung werden alle \
         bestehenden Anmeldungen beendet. Falls du das nicht warst, ignoriere diese E-Mail.</p><p>— Kaspa Battle</p>",
        u = html_escape(username),
        l = html_escape(link)
    );
    OutgoingMail {
        to: to.into(),
        subject: "Kaspa Battle: Passwort zurücksetzen".into(),
        text,
        html: Some(html),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn no_host_means_mail_disabled() {
        assert!(SmtpConfig::from_env(env(&[])).unwrap().is_none());
        assert!(SmtpConfig::from_env(env(&[("SMTP_HOST", "  ")]))
            .unwrap()
            .is_none());
    }

    #[test]
    fn defaults_follow_the_tls_mode() {
        let c = SmtpConfig::from_env(env(&[
            ("SMTP_HOST", "smtp.example.test"),
            ("SMTP_FROM_EMAIL", "info@example.test"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!((c.port, c.tls), (587, TlsMode::StartTls));
        let c = SmtpConfig::from_env(env(&[
            ("SMTP_HOST", "smtp.example.test"),
            ("SMTP_FROM_EMAIL", "info@example.test"),
            ("SMTP_TLS_MODE", "tls"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!((c.port, c.tls), (465, TlsMode::Tls));
    }

    #[test]
    fn rejects_unsafe_or_half_configured_setups() {
        // plaintext to a remote host
        assert!(SmtpConfig::from_env(env(&[
            ("SMTP_HOST", "smtp.example.test"),
            ("SMTP_FROM_EMAIL", "a@b.test"),
            ("SMTP_TLS_MODE", "none"),
        ]))
        .is_err());
        // username without password
        assert!(SmtpConfig::from_env(env(&[
            ("SMTP_HOST", "smtp.example.test"),
            ("SMTP_FROM_EMAIL", "a@b.test"),
            ("SMTP_USERNAME", "u"),
        ]))
        .is_err());
        // missing sender
        assert!(SmtpConfig::from_env(env(&[("SMTP_HOST", "smtp.example.test")])).is_err());
        // loopback plaintext is fine (tests / local relay)
        assert!(SmtpConfig::from_env(env(&[
            ("SMTP_HOST", "127.0.0.1"),
            ("SMTP_FROM_EMAIL", "a@b.test"),
            ("SMTP_TLS_MODE", "none"),
        ]))
        .unwrap()
        .is_some());
    }

    #[test]
    fn debug_output_never_contains_the_password() {
        let c = SmtpConfig::from_env(env(&[
            ("SMTP_HOST", "smtp.example.test"),
            ("SMTP_FROM_EMAIL", "a@b.test"),
            ("SMTP_USERNAME", "user@example.test"),
            ("SMTP_PASSWORD", "hunter2-super-secret"),
        ]))
        .unwrap()
        .unwrap();
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("hunter2"));
        assert!(!dbg.contains("user@example.test"));
    }

    #[test]
    fn templates_escape_html_and_carry_the_link() {
        let m = verification_mail(
            "x@y.test",
            "<b>evil</b>",
            "https://app.test/verify-email?token=abc&x=1",
        );
        assert!(m.html.as_ref().unwrap().contains("&lt;b&gt;evil&lt;/b&gt;"));
        assert!(m
            .text
            .contains("https://app.test/verify-email?token=abc&x=1"));
        let r = password_reset_mail(
            "x@y.test",
            "alice",
            "https://app.test/reset-password?token=t",
        );
        assert!(r.subject.contains("Passwort"));
    }

    /// Minimal fake SMTP server: records the DATA of each message.
    async fn fake_smtp() -> (u16, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let inbox = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let store = inbox.clone();
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else {
                    break;
                };
                let store = store.clone();
                tokio::spawn(async move {
                    let (r, mut w) = sock.into_split();
                    let mut r = BufReader::new(r);
                    w.write_all(b"220 fake ESMTP\r\n").await.unwrap();
                    let (mut in_data, mut data) = (false, String::new());
                    let mut line = String::new();
                    loop {
                        line.clear();
                        if r.read_line(&mut line).await.unwrap_or(0) == 0 {
                            break;
                        }
                        if in_data {
                            if line == ".\r\n" {
                                in_data = false;
                                store.lock().unwrap().push(std::mem::take(&mut data));
                                w.write_all(b"250 queued\r\n").await.unwrap();
                            } else {
                                data.push_str(&line);
                            }
                            continue;
                        }
                        let cmd = line.to_ascii_uppercase();
                        let reply: &[u8] = if cmd.starts_with("EHLO") || cmd.starts_with("HELO") {
                            b"250 fake\r\n"
                        } else if cmd.starts_with("DATA") {
                            in_data = true;
                            b"354 go\r\n"
                        } else if cmd.starts_with("QUIT") {
                            w.write_all(b"221 bye\r\n").await.unwrap();
                            break;
                        } else {
                            b"250 ok\r\n"
                        };
                        w.write_all(reply).await.unwrap();
                    }
                });
            }
        });
        (port, inbox)
    }

    #[tokio::test]
    async fn smtp_transport_delivers_through_a_local_fake_server() {
        let (port, inbox) = fake_smtp().await;
        let cfg = SmtpConfig::from_env(env(&[
            ("SMTP_HOST", "127.0.0.1"),
            ("SMTP_PORT", &port.to_string()),
            ("SMTP_TLS_MODE", "none"),
            ("SMTP_FROM_EMAIL", "info@kaspabattle.test"),
            ("SMTP_FROM_NAME", "Kaspa Battle"),
        ]))
        .unwrap()
        .unwrap();
        let mailer = SmtpMailer::new(&cfg).unwrap();
        assert!(mailer.is_configured());
        mailer
            .send(verification_mail(
                "alice@example.test",
                "Alice",
                "https://app.test/verify-email?token=abc",
            ))
            .await
            .unwrap();
        let got = inbox.lock().unwrap().clone();
        assert_eq!(got.len(), 1);
        assert!(
            got[0].contains("Subject: ")
                && got[0].contains("<info@kaspabattle.test>")
                && got[0].contains("Kaspa Battle"),
            "{}",
            got[0]
        );
        assert!(got[0].contains("To: alice@example.test"));
        assert!(
            got[0].contains("multipart/alternative"),
            "text + html alternative"
        );
        // an unreachable server is reported, not silently swallowed
        let dead = SmtpConfig::from_env(env(&[
            ("SMTP_HOST", "127.0.0.1"),
            ("SMTP_PORT", "1"),
            ("SMTP_TLS_MODE", "none"),
            ("SMTP_FROM_EMAIL", "a@b.test"),
        ]))
        .unwrap()
        .unwrap();
        let err = SmtpMailer::new(&dead)
            .unwrap()
            .send(verification_mail("a@b.test", "A", "https://x"))
            .await;
        assert!(matches!(err, Err(MailError::Transport(_))));
        // invalid recipient is rejected before any network traffic
        assert!(matches!(
            mailer
                .send(verification_mail("not an address", "A", "https://x"))
                .await,
            Err(MailError::Invalid(_))
        ));
    }
}
