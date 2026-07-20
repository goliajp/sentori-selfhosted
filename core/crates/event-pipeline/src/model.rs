//! Typed domain models used by the ingest pipeline.

use std::fmt;

use sentori_workspace_identity::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

// ── enums ─────────────────────────────────────────────────────

/// What kind of event was captured. Matches the `kind` CHECK
/// constraint on `events.kind` + `issues.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Captured exception (most common — JS `throw`, Java
    /// uncaught, ObjC `@throw`, Swift `fatalError`, etc.).
    Error,
    /// Android Application-Not-Responding (≥ 5 s main-thread
    /// freeze). iOS hangs will share this kind once a dedicated
    /// detector lands.
    Anr,
    /// Pre-crash sentinel — SDK observed sustained frame budget
    /// overrun (or future memory pressure / storage low / etc).
    /// App is still running; this is an "about to die" warning.
    NearCrash,
    /// Manual report via `sentori.captureMessage(...)`. Carries
    /// a [`MessageLevel`] + body instead of an exception object.
    Message,
}

impl EventKind {
    /// All four kinds in canonical order.
    pub const ALL: [Self; 4] = [Self::Error, Self::Anr, Self::NearCrash, Self::Message];

    /// SQL wire representation (matches the CHECK constraint).
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Anr => "anr",
            Self::NearCrash => "near_crash",
            Self::Message => "message",
        }
    }

    /// Short stable tag used by the [`sentori_issue_fingerprint`]
    /// "degenerate" variant (rare; events with no body and no
    /// error object).
    #[must_use]
    pub const fn fingerprint_tag(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Anr => "anr",
            Self::NearCrash => "near_crash",
            Self::Message => "message",
        }
    }

    /// Parse from the SQL wire form.
    ///
    /// # Errors
    ///
    /// [`EventKindParseError`] for any unknown string.
    pub fn from_db_str(s: &str) -> Result<Self, EventKindParseError> {
        match s {
            "error" => Ok(Self::Error),
            "anr" => Ok(Self::Anr),
            "near_crash" => Ok(Self::NearCrash),
            "message" => Ok(Self::Message),
            other => Err(EventKindParseError(other.to_string())),
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Originating platform of the event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    /// Web / RN-JS / Node / Hermes / any V8-based runtime.
    Javascript,
    /// iOS native (Obj-C / Swift).
    Ios,
    /// Android native (Java / Kotlin).
    Android,
}

impl Platform {
    /// All three platforms.
    pub const ALL: [Self; 3] = [Self::Javascript, Self::Ios, Self::Android];

    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Javascript => "javascript",
            Self::Ios => "ios",
            Self::Android => "android",
        }
    }

    /// Parse from the SQL wire form.
    ///
    /// # Errors
    ///
    /// [`PlatformParseError`] for any unknown string.
    pub fn from_db_str(s: &str) -> Result<Self, PlatformParseError> {
        match s {
            "javascript" => Ok(Self::Javascript),
            "ios" => Ok(Self::Ios),
            "android" => Ok(Self::Android),
            other => Err(PlatformParseError(other.to_string())),
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Syslog-aligned severity for [`EventKind::Message`] events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel {
    /// "Crash imminent" — process self-destruct.
    Fatal,
    /// Unrecoverable failure of one operation.
    Error,
    /// Recoverable, but the operator should look.
    Warning,
    /// Operational message worth keeping.
    Info,
    /// Diagnostic; off by default in prod.
    Debug,
}

impl MessageLevel {
    /// All five levels, most-to-least severe.
    pub const ALL: [Self; 5] = [
        Self::Fatal,
        Self::Error,
        Self::Warning,
        Self::Info,
        Self::Debug,
    ];

    /// Stable string form (matches serde's lowercase rename).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fatal => "fatal",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }

    /// Parse from the wire string.
    ///
    /// # Errors
    ///
    /// [`MessageLevelParseError`] for any unknown string.
    pub fn parse(s: &str) -> Result<Self, MessageLevelParseError> {
        match s {
            "fatal" => Ok(Self::Fatal),
            "error" => Ok(Self::Error),
            "warning" => Ok(Self::Warning),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            other => Err(MessageLevelParseError(other.to_string())),
        }
    }
}

impl std::str::FromStr for MessageLevel {
    type Err = MessageLevelParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for MessageLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Issue lifecycle status. Matches the `issues.status` CHECK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueStatus {
    /// Default. New events bump `event_count` + `last_seen`.
    Active,
    /// Operator marked fixed. Next event flips to [`Self::Regressed`].
    Resolved,
    /// A new event landed after the operator marked the issue
    /// resolved. The ingest UPSERT atomically flips status +
    /// stamps `regressed_at`.
    Regressed,
    /// Operator chose to silence. Events still attach but
    /// notifications / dashboards filter it out by default.
    Ignored,
}

impl IssueStatus {
    /// All four statuses.
    pub const ALL: [Self; 4] = [Self::Active, Self::Resolved, Self::Regressed, Self::Ignored];

    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Resolved => "resolved",
            Self::Regressed => "regressed",
            Self::Ignored => "ignored",
        }
    }

    /// Parse from the SQL wire form.
    ///
    /// # Errors
    ///
    /// [`IssueStatusParseError`] for any unknown string.
    pub fn from_db_str(s: &str) -> Result<Self, IssueStatusParseError> {
        match s {
            "active" => Ok(Self::Active),
            "resolved" => Ok(Self::Resolved),
            "regressed" => Ok(Self::Regressed),
            "ignored" => Ok(Self::Ignored),
            other => Err(IssueStatusParseError(other.to_string())),
        }
    }
}

impl fmt::Display for IssueStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

// ── parse error types ────────────────────────────────────────

/// Error from [`EventKind::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unrecognised event kind: {0:?}")]
pub struct EventKindParseError(pub String);

/// Error from [`Platform::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unrecognised platform: {0:?}")]
pub struct PlatformParseError(pub String);

/// Error from [`MessageLevel::parse`] /
/// `<MessageLevel as std::str::FromStr>::from_str`.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unrecognised message level: {0:?}")]
pub struct MessageLevelParseError(pub String);

/// Error from [`IssueStatus::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unrecognised issue status: {0:?}")]
pub struct IssueStatusParseError(pub String);

// ── Event ────────────────────────────────────────────────────

/// One captured event, the unit of [`crate::IngestService::ingest`].
///
/// Slim typed top-level fields + JSONB `payload` (per user
/// decision 2026-06-20). Top-level carries everything the
/// dashboard facets on and everything the fingerprint algorithm
/// reads; the wide `payload` JSONB carries SDK additions
/// (device, app, breadcrumbs, tags, user, geo, bundle, flags,
/// attachments, framework, link_hashes, symbolication) so SDK
/// growth is zero-migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Server-assigned event id (UUIDv7).
    pub id: Uuid,
    /// When the event was captured on the device.
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    /// Event shape (Error / Anr / NearCrash / Message).
    pub kind: EventKind,
    /// Originating platform.
    pub platform: Platform,
    /// Release identifier of the running app (`myapp@5.3.1`).
    pub release: String,
    /// `production` / `staging` / etc.
    pub environment: String,

    /// Exception class / kind tag (e.g. `TypeError`,
    /// `java.lang.NullPointerException`). Required for
    /// `kind ∈ {Error, Anr, NearCrash}`; ignored for `Message`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,

    /// Human-readable body. For Exception-shape events this is
    /// the exception's `message`; for [`EventKind::Message`]
    /// this is the body the operator captured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Severity. Required for [`EventKind::Message`]; ignored
    /// for other kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<MessageLevel>,

    /// Identifying frame the fingerprint hashes for
    /// Exception-shape events. Caller (the symbolicator
    /// upstream of the pipeline) resolves the "first in-app
    /// frame" choice and supplies one or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameSite>,

    /// Client-supplied fingerprint override. If non-empty,
    /// short-circuits the algorithmic fingerprint and is used
    /// verbatim. Used when application context dictates
    /// grouping (e.g. "all card-decline errors are one issue
    /// regardless of message").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint_override: Option<String>,

    /// Everything else the SDK shipped — device / app /
    /// breadcrumbs / tags / user / geo / bundle / flags /
    /// attachments / framework / link_hashes / symbolication
    /// info. Zero-migration storage; structured queries land
    /// in the dashboard via JSONB ops on this column.
    #[serde(default)]
    pub payload: Value,
}

impl Event {
    /// Construct an exception-shape event with the four
    /// fingerprinted fields (release, error_type, message,
    /// optional frame).
    #[must_use]
    pub fn exception(
        id: Uuid,
        timestamp: OffsetDateTime,
        platform: Platform,
        release: impl Into<String>,
        environment: impl Into<String>,
        error_type: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id,
            timestamp,
            kind: EventKind::Error,
            platform,
            release: release.into(),
            environment: environment.into(),
            error_type: Some(error_type.into()),
            message: Some(message.into()),
            level: None,
            frame: None,
            fingerprint_override: None,
            payload: Value::Object(serde_json::Map::new()),
        }
    }

    /// Construct a manual-report message event.
    #[must_use]
    pub fn message(
        id: Uuid,
        timestamp: OffsetDateTime,
        platform: Platform,
        release: impl Into<String>,
        environment: impl Into<String>,
        level: MessageLevel,
        body: impl Into<String>,
    ) -> Self {
        Self {
            id,
            timestamp,
            kind: EventKind::Message,
            platform,
            release: release.into(),
            environment: environment.into(),
            error_type: None,
            message: Some(body.into()),
            level: Some(level),
            frame: None,
            fingerprint_override: None,
            payload: Value::Object(serde_json::Map::new()),
        }
    }

    /// Replace the identifying frame (chained-builder style).
    #[must_use]
    pub fn with_frame(mut self, frame: FrameSite) -> Self {
        self.frame = Some(frame);
        self
    }

    /// Replace the `payload` JSONB blob.
    #[must_use]
    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    /// Replace the fingerprint override.
    #[must_use]
    pub fn with_fingerprint_override(mut self, fp: impl Into<String>) -> Self {
        self.fingerprint_override = Some(fp.into());
        self
    }
}

/// Identifying source-location pair used by the fingerprint
/// algorithm. Mirrors [`sentori_issue_fingerprint::FrameSite`]
/// in owned-`String` form (the fingerprint crate takes
/// borrowed `&str`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSite {
    /// Function / symbol name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// Source file / module name.
    pub file: String,
}

// ── ingest input / output / DB rows ──────────────────────────

/// Pair of project + event used in the ring buffer and in
/// [`crate::IngestService::try_enqueue`] / `flush`.
#[derive(Debug, Clone)]
pub struct EnqueuedEvent {
    /// Owning project.
    pub project_id: ProjectId,
    /// The captured event.
    pub event: Event,
}

/// Successful ingest of one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestOutcome {
    /// The persisted event row's id.
    pub event_id: Uuid,
    /// The owning issue's id (newly inserted or already
    /// present).
    pub issue_id: Uuid,
    /// True if this event created the issue row.
    pub is_new_issue: bool,
    /// True if the UPSERT atomically flipped a previously
    /// resolved issue back to regressed.
    pub regressed: bool,
}

/// `issues` row pulled from the DB. Used by tests and by
/// downstream consumers (alert rules / notifier) to inspect
/// post-UPSERT state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    /// Primary key.
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Hash key (32-char hex from [`sentori_issue_fingerprint`]
    /// or a verbatim caller override).
    pub fingerprint: String,
    /// Exception class for the first event in this group.
    pub error_type: String,
    /// First-event human message (display only; later events
    /// don't overwrite).
    pub message_sample: String,
    /// Event kind of the first event in this group.
    pub kind: EventKind,
    /// Lifecycle status.
    pub status: IssueStatus,
    /// First-event timestamp.
    pub first_seen: OffsetDateTime,
    /// Last-event timestamp.
    pub last_seen: OffsetDateTime,
    /// Number of events that landed in this group.
    pub event_count: i64,
    /// Environment of the most recent event.
    pub last_environment: String,
    /// Release of the most recent event.
    pub last_release: String,
    /// When the issue was flipped back to regressed (if ever).
    pub regressed_at: Option<OffsetDateTime>,
    /// Release in which the regression was observed.
    pub regressed_in_release: Option<String>,
    /// When the issue was last marked resolved.
    pub resolved_at: Option<OffsetDateTime>,
}

/// `events` row pulled from the DB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Primary key.
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Issue this event joined.
    pub issue_id: Uuid,
    /// Capture timestamp.
    pub timestamp: OffsetDateTime,
    /// Event kind.
    pub kind: EventKind,
    /// Platform.
    pub platform: Platform,
    /// Release.
    pub release: String,
    /// Environment.
    pub environment: String,
    /// JSONB payload (`device` / `app` / `breadcrumbs` / …).
    pub payload: Value,
    /// When the server received the event.
    pub received_at: OffsetDateTime,
}
