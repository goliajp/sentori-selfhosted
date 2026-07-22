//! Transactional auth email sender (verify / password-reset).
//!
//! Wraps the K11 notifier [`EmailTransport`] behind env config:
//!
//! - `SENTORI_SMTP_HOST` — unset ⇒ mailer disabled; tokens are
//!   logged at WARN so a self-hosted operator without SMTP can
//!   still assist users manually. Tokens are NEVER returned in
//!   HTTP responses (that would hand account takeover to anyone
//!   who can POST /auth/forgot-password).
//! - `SENTORI_SMTP_PORT` (587) / `SENTORI_SMTP_USER` /
//!   `SENTORI_SMTP_PASS` / `SENTORI_SMTP_FROM`
//!   (sentori@golia.jp) / `SENTORI_SMTP_TLS` (starttls|plain)
//! - `SENTORI_BASE_URL` — public dashboard origin used in the
//!   links (default `http://localhost:8080`).
//!
//! Sends are spawned fire-and-forget: SMTP latency/failure must
//! not block or fail the auth endpoint; failures are logged.

use sentori_notifier::{Channel, Notification};
use sentori_notifier::{EmailConfig, EmailTransport, SmtpTls};
use sentori_workspace_identity::WorkspaceId;
use std::sync::Arc;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct Mailer {
    transport: Option<Arc<EmailTransport>>,
    base_url: String,
}

impl Mailer {
    /// Build from `SENTORI_SMTP_*` env. Never fails — a bad or
    /// missing SMTP config degrades to log-only mode.
    #[must_use]
    pub fn from_env() -> Self {
        let base_url = std::env::var("SENTORI_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        let Ok(host) = std::env::var("SENTORI_SMTP_HOST") else {
            warn!("SENTORI_SMTP_HOST unset — auth emails disabled, tokens logged at WARN");
            return Self {
                transport: None,
                base_url,
            };
        };
        if host.is_empty() {
            warn!("SENTORI_SMTP_HOST empty — auth emails disabled, tokens logged at WARN");
            return Self {
                transport: None,
                base_url,
            };
        }
        let cfg = EmailConfig {
            smtp_host: host,
            smtp_port: std::env::var("SENTORI_SMTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(587),
            smtp_user: std::env::var("SENTORI_SMTP_USER")
                .ok()
                .filter(|s| !s.is_empty()),
            smtp_pass: std::env::var("SENTORI_SMTP_PASS")
                .ok()
                .filter(|s| !s.is_empty()),
            from: std::env::var("SENTORI_SMTP_FROM")
                .unwrap_or_else(|_| "sentori@golia.jp".to_string()),
            tls: SmtpTls::from_env_str(
                &std::env::var("SENTORI_SMTP_TLS").unwrap_or_else(|_| "starttls".to_string()),
            ),
        };
        match EmailTransport::new(cfg) {
            Ok(t) => Self {
                transport: Some(Arc::new(t)),
                base_url,
            },
            Err(e) => {
                error!(%e, "SMTP transport init failed — auth emails disabled, tokens logged");
                Self {
                    transport: None,
                    base_url,
                }
            }
        }
    }

    pub fn send_verify(&self, workspace_id: WorkspaceId, email: &str, token_wire: &str) {
        let link = format!("{}/verify?token={token_wire}", self.base_url);
        self.dispatch(
            workspace_id,
            email,
            "Verify your Sentori account",
            format!(
                "Welcome to Sentori!\n\nConfirm your email address by opening:\n\n{link}\n\nThe link expires in 24 hours. If you didn't sign up, ignore this email."
            ),
            "email_verify",
            token_wire,
        );
    }

    pub fn send_reset(&self, workspace_id: WorkspaceId, email: &str, token_wire: &str) {
        let link = format!("{}/reset-password?token={token_wire}", self.base_url);
        self.dispatch(
            workspace_id,
            email,
            "Reset your Sentori password",
            format!(
                "A password reset was requested for this address.\n\nSet a new password here:\n\n{link}\n\nThe link expires soon. If you didn't request this, ignore this email — your password is unchanged."
            ),
            "password_reset",
            token_wire,
        );
    }

    fn dispatch(
        &self,
        workspace_id: WorkspaceId,
        email: &str,
        subject: &str,
        body: String,
        kind: &'static str,
        token_wire: &str,
    ) {
        let Some(transport) = self.transport.clone() else {
            // Operator-assist fallback: without SMTP the token
            // only exists here — never in an HTTP response.
            warn!(
                email,
                kind,
                token = token_wire,
                "SMTP disabled — auth token logged"
            );
            return;
        };
        let n = Notification {
            workspace_id,
            project_id: None,
            channel: Channel::Email,
            recipient: email.to_string(),
            subject: subject.to_string(),
            body,
            metadata: serde_json::Value::Null,
            dedup_key: None,
        };
        let email_owned = email.to_string();
        tokio::spawn(async move {
            use sentori_notifier::Notifier;
            match transport.send(&n).await {
                Ok(()) => info!(email = email_owned, kind, "auth email sent"),
                Err(e) => error!(email = email_owned, kind, %e, "auth email send failed"),
            }
        });
    }
}

impl std::fmt::Debug for Mailer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mailer")
            .field("enabled", &self.transport.is_some())
            .field("base_url", &self.base_url)
            .finish()
    }
}
