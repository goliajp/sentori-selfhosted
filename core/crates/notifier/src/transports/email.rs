//! [`EmailTransport`] — async SMTP via [`lettre`].

use async_trait::async_trait;
use lettre::transport::smtp::AsyncSmtpTransport;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};

use crate::error::TransportError;
use crate::model::{Channel, Notification};
use crate::transports::Notifier;

/// SMTP connection mode. Match the SMTP relay's expectations:
/// public relays (Let's Encrypt-fronted, SendGrid, SES) want
/// `Starttls` on port 587; mailcatchers / dev relays
/// (mailpit, mailcrab) only speak `Plain` on port 1025.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmtpTls {
    /// STARTTLS upgrade after plaintext greeting (RFC 3207).
    Starttls,
    /// No TLS at all. Dev/test only.
    Plain,
}

impl SmtpTls {
    /// Parse from env-var-style strings (`plain` / `none` /
    /// `off` → Plain; anything else → Starttls). Convenience
    /// for the consumer crate's `config_from_env`.
    #[must_use]
    pub fn from_env_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "plain" | "none" | "off" | "false" => Self::Plain,
            _ => Self::Starttls,
        }
    }
}

/// SMTP connection + identity config.
#[derive(Clone, Debug)]
pub struct EmailConfig {
    /// Relay hostname.
    pub smtp_host: String,
    /// Relay port (587 STARTTLS / 25 plain / 1025 dev).
    pub smtp_port: u16,
    /// Optional auth username.
    pub smtp_user: Option<String>,
    /// Optional auth password.
    pub smtp_pass: Option<String>,
    /// `From:` mailbox.
    pub from: String,
    /// TLS mode.
    pub tls: SmtpTls,
}

/// Email transport. Cheap to clone (lettre's
/// `AsyncSmtpTransport` is `Arc`-backed internally; we wrap
/// it for ergonomic `Clone`).
pub struct EmailTransport {
    inner: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl std::fmt::Debug for EmailTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailTransport")
            .field("from", &self.from)
            .finish_non_exhaustive()
    }
}

impl Clone for EmailTransport {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            from: self.from.clone(),
        }
    }
}

impl EmailTransport {
    /// Build a transport.
    ///
    /// # Errors
    ///
    /// [`TransportError::Email`] when lettre's builder
    /// rejects the host (rare; surfaces TLS setup failures).
    pub fn new(cfg: EmailConfig) -> Result<Self, TransportError> {
        let mut builder = match cfg.tls {
            SmtpTls::Starttls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.smtp_host)
                    .map_err(|e| TransportError::Email(format!("starttls builder: {e}")))?
            }
            SmtpTls::Plain => {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.smtp_host)
            }
        }
        .port(cfg.smtp_port);
        if let (Some(u), Some(p)) = (&cfg.smtp_user, &cfg.smtp_pass) {
            builder = builder.credentials(Credentials::new(u.clone(), p.clone()));
        }
        Ok(Self {
            inner: builder.build(),
            from: cfg.from,
        })
    }

    /// Borrow the `From:` mailbox.
    #[must_use]
    pub fn from(&self) -> &str {
        &self.from
    }

    /// Build the lettre `Message` for `n` against this
    /// transport's `From:`. Exposed pub-test so unit tests
    /// can assert envelope shape without hitting an SMTP
    /// server.
    ///
    /// # Errors
    ///
    /// [`TransportError::Email`] when the `From` / `To`
    /// address fails to parse, or the body fails to encode.
    pub fn build_message(&self, n: &Notification) -> Result<Message, TransportError> {
        Message::builder()
            .from(
                self.from
                    .parse()
                    .map_err(|e| TransportError::Email(format!("from parse: {e}")))?,
            )
            .to(n
                .recipient
                .parse()
                .map_err(|e| TransportError::Email(format!("to parse: {e}")))?)
            .subject(&n.subject)
            .body(n.body.clone())
            .map_err(|e| TransportError::Email(format!("body build: {e}")))
    }
}

#[async_trait]
impl Notifier for EmailTransport {
    fn channel(&self) -> Channel {
        Channel::Email
    }

    async fn send(&self, n: &Notification) -> Result<(), TransportError> {
        let msg = self.build_message(n)?;
        self.inner
            .send(msg)
            .await
            .map_err(|e| TransportError::Email(format!("smtp send: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use sentori_workspace_identity::WorkspaceId;

    fn cfg() -> EmailConfig {
        EmailConfig {
            smtp_host: "localhost".into(),
            smtp_port: 1025,
            smtp_user: None,
            smtp_pass: None,
            from: "sentori@example.com".into(),
            tls: SmtpTls::Plain,
        }
    }

    #[test]
    fn smtp_tls_env_parsing() {
        assert_eq!(SmtpTls::from_env_str("starttls"), SmtpTls::Starttls);
        assert_eq!(SmtpTls::from_env_str("plain"), SmtpTls::Plain);
        assert_eq!(SmtpTls::from_env_str("None"), SmtpTls::Plain);
        assert_eq!(SmtpTls::from_env_str("OFF"), SmtpTls::Plain);
        assert_eq!(SmtpTls::from_env_str(""), SmtpTls::Starttls);
    }

    #[test]
    fn build_message_constructs_envelope() {
        let t = EmailTransport::new(cfg()).unwrap();
        let n = Notification::new(
            WorkspaceId::new(),
            Channel::Email,
            "alice@example.com",
            "hello",
            "world body",
        );
        let msg = t.build_message(&n).unwrap();
        let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
        assert!(raw.contains("From: sentori@example.com"));
        assert!(raw.contains("To: alice@example.com"));
        assert!(raw.contains("Subject: hello"));
        assert!(raw.contains("world body"));
    }

    #[test]
    fn build_message_rejects_bad_recipient() {
        let t = EmailTransport::new(cfg()).unwrap();
        let n = Notification::new(WorkspaceId::new(), Channel::Email, "not-an-email", "x", "y");
        assert!(matches!(t.build_message(&n), Err(TransportError::Email(_))));
    }

    #[test]
    fn channel_is_email() {
        let t = EmailTransport::new(cfg()).unwrap();
        assert_eq!(t.channel(), Channel::Email);
    }
}
