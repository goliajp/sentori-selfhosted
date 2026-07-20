//! Typed domain models for the push-provider surface.

use std::fmt;

use sentori_workspace_identity::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

// ── ProviderKind ─────────────────────────────────────────────

/// Which vendor a token / credential / provider impl belongs
/// to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    /// Apple Push Notification service (iOS / macOS).
    Apns,
    /// Firebase Cloud Messaging HTTP v1 (Android default).
    Fcm,
    /// Web Push Protocol (browser push via VAPID + push services).
    WebPush,
    /// Huawei Cloud Messaging (Huawei devices, non-GMS Android).
    Hcm,
    /// Xiaomi MiPush (Chinese-market Android).
    MiPush,
}

impl ProviderKind {
    /// All five variants in canonical UI order.
    pub const ALL: [Self; 5] = [
        Self::Apns,
        Self::Fcm,
        Self::WebPush,
        Self::Hcm,
        Self::MiPush,
    ];

    /// SQL wire form (matches the CHECK constraint on
    /// push_tokens.kind + push_credentials.kind).
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Apns => "apns",
            Self::Fcm => "fcm",
            Self::WebPush => "webpush",
            Self::Hcm => "hcm",
            Self::MiPush => "mipush",
        }
    }

    /// Parse from the SQL wire form.
    ///
    /// # Errors
    ///
    /// [`ProviderKindParseError`] for any unknown string.
    pub fn from_db_str(s: &str) -> Result<Self, ProviderKindParseError> {
        match s {
            "apns" => Ok(Self::Apns),
            "fcm" => Ok(Self::Fcm),
            "webpush" => Ok(Self::WebPush),
            "hcm" => Ok(Self::Hcm),
            "mipush" => Ok(Self::MiPush),
            other => Err(ProviderKindParseError(other.to_string())),
        }
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error from [`ProviderKind::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unrecognised provider kind: {0:?}")]
pub struct ProviderKindParseError(pub String);

// ── NativeMessage ───────────────────────────────────────────

/// What the dispatcher hands to each [`crate::PushProvider`]
/// impl.
///
/// Slim-typed common fields (title, body) + JSONB
/// `provider_extra` for vendor-specific overrides (APNs alert
/// sound, FCM notification color, Web Push urgency, etc.).
/// Per K4 / K6 precedent — typed for what the dashboard reads
/// + facets, JSONB for what only the vendor consumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMessage {
    /// Notification title (display).
    pub title: String,
    /// Notification body (display).
    pub body: String,
    /// Caller-supplied `data` payload — passed through to the
    /// provider's notification data section, never displayed.
    /// Use for deeplink URLs, action ids, etc.
    #[serde(default)]
    pub data: Value,
    /// Vendor-specific overrides. Keys recognised by the
    /// vendor impl (e.g. `{ "apns": { "sound": "alert.caf" } }`).
    /// Unrecognised keys are silently passed through.
    #[serde(default)]
    pub provider_extra: Value,
}

impl NativeMessage {
    /// Build a title-body-only message (no `data`, no
    /// `provider_extra`).
    #[must_use]
    pub fn simple(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            data: Value::Object(serde_json::Map::new()),
            provider_extra: Value::Object(serde_json::Map::new()),
        }
    }
}

// ── Credential ──────────────────────────────────────────────

/// What [`crate::PushDispatcher`] passes to provider's `send` / `validate`.
///
/// Wraps the non-secret JSONB configuration + the post-
/// unseal sensitive bytes (S12 vault has already opened the
/// envelope by this point).
#[derive(Debug, Clone)]
pub struct Credential<'a> {
    /// JSONB config — APNs key id, FCM project id, VAPID
    /// public key, etc. Shape is vendor-specific.
    pub config: &'a Value,
    /// Decrypted secret bytes (vault-unsealed).
    pub secret_payload: &'a [u8],
}

// ── Outcomes ────────────────────────────────────────────────

/// What a single dispatch attempt yielded. Mirrors the legacy
/// provider's `SendOutcome` but with `Display` + serde so
/// dashboards can serialise audit logs cleanly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SendOutcome {
    /// Provider accepted the send.
    Sent,
    /// Vendor said this token is dead (uninstalled,
    /// re-installed with new token, blocked, etc.). Dispatcher
    /// quarantines the row.
    PermanentlyInvalidToken,
    /// Token was registered as `sandbox`/`production` but the
    /// vendor said the other. Caller can retry on the opposite
    /// env or mark as failed. Not a quarantine on its own.
    EnvironmentMismatch,
    /// Try again. `retry_after_secs` is the vendor's hint;
    /// `None` means "use your own backoff".
    Transient {
        /// Seconds to wait before retrying, if the vendor supplied one.
        retry_after_secs: Option<i32>,
    },
    /// Some other terminal error (vendor auth failure, payload
    /// schema rejection, quota exhausted permanently). The
    /// dispatcher records, does NOT quarantine, and surfaces
    /// to the consumer via [`crate::PushError::Provider`].
    TerminalOther {
        /// Human-readable explanation.
        reason: String,
    },
}

impl SendOutcome {
    /// True for the one variant the dispatcher acts on by
    /// stamping `push_tokens.quarantined_at`.
    #[must_use]
    pub const fn should_quarantine(&self) -> bool {
        matches!(self, Self::PermanentlyInvalidToken)
    }

    /// True for variants the caller might want to retry.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Transient { .. } | Self::EnvironmentMismatch)
    }
}

/// One dispatch's full result.
///
/// Carries the outcome, the vendor's stable label string, an
/// HTTP status (when applicable), and a truncated response
/// body — all sufficient for an audit log row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderResult {
    /// What happened.
    pub outcome: SendOutcome,
    /// Stable label for the row's `provider_outcome` column —
    /// e.g. `"APNS_200"`, `"FCM_403"`, `"MOCK_OK"`.
    pub provider_outcome_label: String,
    /// Vendor HTTP status, when applicable.
    pub provider_status: Option<i32>,
    /// Truncated vendor body (≤ 2 KB) for the audit log.
    pub provider_body: Option<String>,
    /// Round-trip duration.
    pub duration_ms: i32,
}

/// One credential-validation attempt's result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ValidateOutcome {
    /// Parse + (where applicable) auth challenge succeeded.
    Ok,
    /// Cred parses but vendor rejected the auth challenge —
    /// stale or wrong secret.
    Rejected {
        /// Vendor's reason.
        reason: String,
    },
    /// Cred itself is malformed (missing fields, bad PEM,
    /// etc.).
    Malformed {
        /// Specific defect.
        reason: String,
    },
    /// Network unreachable / timeout. Caller is told "unknown
    /// — try again" rather than "broken".
    Unreachable {
        /// What went wrong.
        reason: String,
    },
    /// Provider doesn't expose a fast validation path. The
    /// shape parsed; that's all we can say.
    NotImplemented,
}

impl ValidateOutcome {
    /// Stable string form for the `last_validate_status`
    /// column.
    #[must_use]
    pub const fn as_db_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Rejected { .. } => "rejected",
            Self::Malformed { .. } => "malformed",
            Self::Unreachable { .. } => "unreachable",
            Self::NotImplemented => "not_implemented",
        }
    }
}

// ── DeviceToken / MintedToken ───────────────────────────────

/// `push_tokens` row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceToken {
    /// Primary key.
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Which provider this token belongs to.
    pub kind: ProviderKind,
    /// Provider-native token string.
    pub native_token: String,
    /// APNs env (`production` / `sandbox`); `None` for other
    /// providers.
    pub env: Option<String>,
    /// Caller-supplied app-side user id (matches K4 Event's
    /// `payload.user.id`). Lets the dispatcher fan out to
    /// "every device for user X".
    pub app_user_id: Option<String>,
    /// Creation timestamp.
    pub created_at: OffsetDateTime,
    /// Last activity (used by retention to drop ancient
    /// tokens).
    pub last_seen_at: OffsetDateTime,
    /// Quarantine stamp; non-None means dispatcher skips this
    /// row.
    pub quarantined_at: Option<OffsetDateTime>,
    /// Why it was quarantined (e.g.
    /// `"PermanentlyInvalidToken: vendor said device dead"`).
    pub quarantine_reason: Option<String>,
}

impl DeviceToken {
    /// True if the dispatcher should skip this row.
    #[must_use]
    pub const fn is_quarantined(&self) -> bool {
        self.quarantined_at.is_some()
    }
}

/// Return value of [`crate::DeviceTokenStore::upsert`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintedToken {
    /// Row id (server-minted or pre-existing on conflict).
    pub id: Uuid,
    /// True if this call inserted a new row; false if the
    /// `(project, kind, native_token)` tuple already existed
    /// and we just bumped `last_seen_at` / `app_user_id`.
    pub is_new: bool,
}
