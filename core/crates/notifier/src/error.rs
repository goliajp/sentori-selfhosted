//! Typed errors for [`crate::NotifierService`] and
//! [`crate::Notifier`] transports.

use thiserror::Error;
use uuid::Uuid;

use crate::model::ChannelParseError;

/// Per-transport failure. Surfaced via
/// [`NotifierError::Transport`].
#[derive(Debug, Error)]
pub enum TransportError {
    /// SMTP / mail builder rejected the message (bad
    /// address, missing field, transport error).
    #[error("email: {0}")]
    Email(String),

    /// Webhook HTTP layer (DNS, TLS, timeout, non-2xx).
    #[error("webhook: {0}")]
    Webhook(String),

    /// Mock transport's deterministic failure injection.
    #[error("mock: {0}")]
    Mock(String),

    /// Caller routed a notification to a channel with no
    /// registered transport.
    #[error("no transport registered for channel {channel}")]
    NoTransport {
        /// The channel wire name (`email` / `webhook` / …).
        channel: String,
    },
}

impl TransportError {
    /// Render to the bounded form persisted as
    /// `delivery_log.error`.
    #[must_use]
    pub fn to_log_error(&self) -> String {
        const MAX: usize = 2000;
        let s = self.to_string();
        if s.len() <= MAX {
            s
        } else {
            // UTF-8 safe truncation.
            let mut end = MAX;
            while end > 0 && !s.is_char_boundary(end) {
                end -= 1;
            }
            s[..end].to_string()
        }
    }
}

/// Service-level failure.
#[derive(Debug, Error)]
pub enum NotifierError {
    /// Caller's [`crate::Notification`] failed structural
    /// validation (empty subject, empty recipient, channel
    /// not parseable, etc.).
    #[error("invalid notification: {0}")]
    InvalidInput(String),

    /// Caller's project FK violated on dispatch.
    #[error("project {0} not found")]
    ProjectNotFound(Uuid),

    /// Channel wire form persisted in DB couldn't parse.
    #[error("malformed channel in db: {0}")]
    InvalidChannelInDb(#[from] ChannelParseError),

    /// Status string in DB doesn't match any
    /// [`crate::DeliveryStatus`] variant.
    #[error("malformed status in db: {0:?}")]
    InvalidStatusInDb(String),

    /// Log row referenced by `retry_one` wasn't found.
    #[error("delivery log {0} not found")]
    LogNotFound(Uuid),

    /// Underlying transport failure (Email / Webhook /
    /// Mock). The log row is updated to `failed` regardless.
    #[error("transport failed: {0}")]
    Transport(#[from] TransportError),

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}
