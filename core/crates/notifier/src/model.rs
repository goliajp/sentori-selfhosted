//! Typed domain models for the notifier.

use std::fmt;

use sentori_workspace_identity::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// What surface this notification rides on. Drives transport
/// lookup in [`crate::NotifierService::dispatch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// SMTP email via [`crate::EmailTransport`].
    Email,
    /// Generic HTTP POST via [`crate::WebhookTransport`].
    /// Recipient is the full URL.
    Webhook,
    /// In-memory recorder via [`crate::MockTransport`]; tests
    /// only (production callers should never construct
    /// `Channel::Mock`).
    Mock,
}

impl Channel {
    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Webhook => "webhook",
            Self::Mock => "mock",
        }
    }

    /// Parse from the SQL wire form.
    ///
    /// # Errors
    ///
    /// [`ChannelParseError`] for unknown strings.
    pub fn from_db_str(s: &str) -> Result<Self, ChannelParseError> {
        match s {
            "email" => Ok(Self::Email),
            "webhook" => Ok(Self::Webhook),
            "mock" => Ok(Self::Mock),
            other => Err(ChannelParseError(other.to_string())),
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error from [`Channel::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown channel: {0:?}")]
pub struct ChannelParseError(pub String);

/// Current state of a [`DeliveryLog`] row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    /// Row inserted; transport call not yet made (in-flight).
    /// In K11 the transport call is synchronous so this state
    /// is brief — but it exists for visibility + future
    /// queue-based dispatch.
    Pending,
    /// Transport ack'd.
    Delivered,
    /// Transport rejected; `error` field populated.
    Failed,
}

impl DeliveryStatus {
    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Delivered => "delivered",
            Self::Failed => "failed",
        }
    }

    /// Parse the wire form. Caller wraps errors into
    /// `NotifierError::InvalidStatusInDb`.
    #[must_use]
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "delivered" => Some(Self::Delivered),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// One notification handed to [`crate::NotifierService::dispatch`].
///
/// The shape is intentionally vendor-agnostic; vendor
/// adapters (K12) build a `Notification` per their schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// Owning workspace. Required even for system-level
    /// notifications (they belong to a reserved system
    /// workspace by convention).
    pub workspace_id: WorkspaceId,
    /// Owning project. `None` for workspace-level (boot-time
    /// admin notifications, infra alerts).
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    /// Which transport.
    pub channel: Channel,
    /// Email address / webhook URL / mock label.
    pub recipient: String,
    /// Email subject / webhook payload "title" / mock note.
    pub subject: String,
    /// Email body / webhook JSON body / mock detail.
    pub body: String,
    /// Adapter-specific extras (Slack Block Kit,
    /// `X-Webhook-Signature` config, lettre headers). Stored
    /// as-is in `delivery_log.metadata`.
    #[serde(default)]
    pub metadata: Value,
    /// Caller-supplied dedup token; UNIQUE in the log when
    /// not None. Two `dispatch` calls with the same key
    /// short-circuit to `DispatchOutcome::Deduplicated`.
    #[serde(default)]
    pub dedup_key: Option<String>,
}

impl Notification {
    /// Builder for the common (workspace, Email, recipient,
    /// subject, body) tuple. Chain `.with_*` to populate the
    /// rest.
    #[must_use]
    pub fn new(
        workspace_id: WorkspaceId,
        channel: Channel,
        recipient: impl Into<String>,
        subject: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            workspace_id,
            project_id: None,
            channel,
            recipient: recipient.into(),
            subject: subject.into(),
            body: body.into(),
            metadata: Value::Null,
            dedup_key: None,
        }
    }

    /// Set the owning project.
    #[must_use]
    pub fn with_project(mut self, project_id: ProjectId) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Set the dedup key.
    #[must_use]
    pub fn with_dedup_key(mut self, key: impl Into<String>) -> Self {
        self.dedup_key = Some(key.into());
        self
    }

    /// Set the metadata JSON blob.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Stored shape of one delivery attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryLog {
    /// Primary key.
    pub id: Uuid,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Owning project (None for workspace-level notifications).
    pub project_id: Option<ProjectId>,
    /// Transport.
    pub channel: Channel,
    /// Email addr / URL / label.
    pub recipient: String,
    /// Subject line.
    pub subject: String,
    /// First 500 bytes of the body for log redaction.
    pub body_preview: Option<String>,
    /// Adapter-specific extras.
    pub metadata: Value,
    /// Current state.
    pub status: DeliveryStatus,
    /// 0 on first attempt; incremented by
    /// [`crate::NotifierService::retry_one`].
    pub retries: i32,
    /// Error message when `status = Failed`.
    pub error: Option<String>,
    /// Caller's dedup token.
    pub dedup_key: Option<String>,
    /// Filled when transport ack'd.
    pub sent_at: Option<OffsetDateTime>,
    /// Row insertion ts.
    pub created_at: OffsetDateTime,
}

/// What [`crate::NotifierService::dispatch`] returns.
#[derive(Debug, Clone)]
pub enum DispatchOutcome {
    /// New log row created + transport returned ok.
    Delivered {
        /// Row id.
        log_id: Uuid,
    },
    /// dedup_key collided with an existing row; transport
    /// was NOT called. Inspect the returned log for the
    /// prior outcome.
    Deduplicated {
        /// Existing log row.
        existing: Box<DeliveryLog>,
    },
    /// New log row created + transport rejected. Row's
    /// `error` field has the detail.
    Failed {
        /// Row id.
        log_id: Uuid,
        /// Same string persisted in the row.
        error: String,
    },
}

impl DispatchOutcome {
    /// Log id for the row this dispatch created or
    /// referenced. `None` would mean "no row touched" — not
    /// reachable today.
    #[must_use]
    pub fn log_id(&self) -> Uuid {
        match self {
            Self::Delivered { log_id } | Self::Failed { log_id, .. } => *log_id,
            Self::Deduplicated { existing } => existing.id,
        }
    }

    /// True when transport returned ok.
    #[must_use]
    pub const fn is_delivered(&self) -> bool {
        matches!(self, Self::Delivered { .. })
    }

    /// True when caller's dedup_key blocked the dispatch.
    #[must_use]
    pub const fn is_deduplicated(&self) -> bool {
        matches!(self, Self::Deduplicated { .. })
    }
}

/// Maximum bytes stored in `delivery_log.body_preview`.
pub(crate) const BODY_PREVIEW_BYTES: usize = 500;

/// UTF-8 safe truncation for body preview.
pub(crate) fn truncate_body(body: &str) -> String {
    if body.len() <= BODY_PREVIEW_BYTES {
        return body.to_string();
    }
    let mut end = BODY_PREVIEW_BYTES;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    body[..end].to_string()
}

// ── row mapping shared with service.rs ───────────────────────

pub(crate) fn row_to_log(row: &sqlx::postgres::PgRow) -> Result<DeliveryLog, crate::NotifierError> {
    use sqlx::Row as _;
    let status_str: &str = row.get("status");
    let status = DeliveryStatus::from_db_str(status_str)
        .ok_or_else(|| crate::NotifierError::InvalidStatusInDb(status_str.to_string()))?;
    let channel_str: &str = row.get("channel");
    let channel = Channel::from_db_str(channel_str)?;
    Ok(DeliveryLog {
        id: row.get("id"),
        workspace_id: WorkspaceId::from_uuid(row.get::<Uuid, _>("workspace_id")),
        project_id: row
            .get::<Option<Uuid>, _>("project_id")
            .map(ProjectId::from_uuid),
        channel,
        recipient: row.get("recipient"),
        subject: row.get("subject"),
        body_preview: row.get("body_preview"),
        metadata: row.get("metadata"),
        status,
        retries: row.get("retries"),
        error: row.get("error"),
        dedup_key: row.get("dedup_key"),
        sent_at: row.get("sent_at"),
        created_at: row.get("created_at"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn channel_round_trip() {
        for c in [Channel::Email, Channel::Webhook, Channel::Mock] {
            assert_eq!(Channel::from_db_str(c.as_db_str()).unwrap(), c);
        }
    }

    #[test]
    fn channel_parse_rejects_unknown() {
        assert!(Channel::from_db_str("sms").is_err());
    }

    #[test]
    fn status_round_trip() {
        for s in [
            DeliveryStatus::Pending,
            DeliveryStatus::Delivered,
            DeliveryStatus::Failed,
        ] {
            assert_eq!(DeliveryStatus::from_db_str(s.as_db_str()), Some(s));
        }
    }

    #[test]
    fn truncate_body_caps_at_500() {
        let s: String = "x".repeat(700);
        let out = truncate_body(&s);
        assert_eq!(out.len(), 500);
    }

    #[test]
    fn truncate_body_utf8_safe() {
        // "é" is 2 bytes — 250 of them = 500 bytes exactly.
        // 251 would push to 502; preview should back off to 500.
        let s: String = "é".repeat(251);
        let out = truncate_body(&s);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.len() <= 500);
    }

    #[test]
    fn builder_helpers_chain() {
        let pid = sentori_workspace_identity::ProjectId::new();
        let n = Notification::new(
            WorkspaceId::new(),
            Channel::Email,
            "x@y.com",
            "subj",
            "body",
        )
        .with_project(pid)
        .with_dedup_key("k1")
        .with_metadata(serde_json::json!({"foo": 1}));
        assert_eq!(n.project_id, Some(pid));
        assert_eq!(n.dedup_key.as_deref(), Some("k1"));
        assert_eq!(n.metadata["foo"], 1);
    }
}
