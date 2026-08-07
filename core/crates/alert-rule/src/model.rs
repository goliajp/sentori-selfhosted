//! Typed domain models for [`crate::AlertRuleService`].

use std::fmt;

use sentori_workspace_identity::{ProjectId, UserId, WorkspaceId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Trigger kind enum. Stable wire form is snake_case lower
/// case (matches the schema CHECK constraint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    /// First event of a fingerprint — synchronous fire from
    /// the ingest path.
    NewIssue,
    /// Resolved issue had a fresh event — synchronous fire.
    Regression,
    /// ≥N events match filter in `windowMinutes` — caller-
    /// driven cron tick over `AlertRuleService::list_active_by_kind`.
    EventCount,
    /// Crash-free session rate dips below `threshold` in
    /// `windowMinutes` — caller-driven cron tick.
    CrashFreeDrop,
}

impl TriggerKind {
    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::NewIssue => "new_issue",
            Self::Regression => "regression",
            Self::EventCount => "event_count",
            Self::CrashFreeDrop => "crash_free_drop",
        }
    }

    /// Parse from wire form.
    ///
    /// # Errors
    ///
    /// [`TriggerKindParseError`] for unknown strings.
    pub fn from_db_str(s: &str) -> Result<Self, TriggerKindParseError> {
        match s {
            "new_issue" => Ok(Self::NewIssue),
            "regression" => Ok(Self::Regression),
            "event_count" => Ok(Self::EventCount),
            "crash_free_drop" => Ok(Self::CrashFreeDrop),
            other => Err(TriggerKindParseError(other.to_string())),
        }
    }

    /// True for the synchronous on-event triggers
    /// (NewIssue, Regression).
    #[must_use]
    pub const fn is_on_event(self) -> bool {
        matches!(self, Self::NewIssue | Self::Regression)
    }
}

impl fmt::Display for TriggerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error from [`TriggerKind::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown trigger kind: {0:?}")]
pub struct TriggerKindParseError(pub String);

/// Full `alert_rules` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertRule {
    /// Primary key.
    pub id: Uuid,
    /// Owning project. None = workspace-wide.
    pub project_id: Option<ProjectId>,
    /// Display name.
    pub name: String,
    /// On / off flag.
    pub enabled: bool,
    /// Trigger kind.
    pub trigger_kind: TriggerKind,
    /// Trigger-shape config (vendor-specific).
    pub trigger_config: Value,
    /// Filter config (environment / release / errorType).
    pub filter_config: Value,
    /// Channels JSONB array.
    pub channels: Value,
    /// Minimum minutes between fires.
    pub throttle_minutes: i32,
    /// Last fire timestamp (None = never fired).
    pub last_fired_at: Option<OffsetDateTime>,
    /// Operator-set permanent silence.
    pub muted: bool,
    /// Operator-set temporary silence — when in the future,
    /// the rule is treated as muted.
    pub snoozed_until: Option<OffsetDateTime>,
    /// Creation ts.
    pub created_at: OffsetDateTime,
    /// Creator user (None for system-created via boot script).
    pub created_by: Option<UserId>,
    /// Last update ts.
    pub updated_at: OffsetDateTime,
}

impl AlertRule {
    /// True when the rule is effectively silenced — caller
    /// drops fires when this returns true.
    #[must_use]
    pub fn is_silenced(&self, now: OffsetDateTime) -> bool {
        !self.enabled || self.muted || self.snoozed_until.is_some_and(|until| until > now)
    }
}

/// Builder for `create_rule`. Required: workspace + name +
/// trigger_kind. `project_id` None = workspace-wide rule.
#[derive(Debug, Clone)]
pub struct AlertRuleDraft {
    /// Owning workspace (required; rule rows are workspace-
    /// bound even when `project_id` is None).
    pub workspace_id: WorkspaceId,
    /// Owning project (None = workspace-wide).
    pub project_id: Option<ProjectId>,
    /// Display name.
    pub name: String,
    /// Trigger kind.
    pub trigger_kind: TriggerKind,
    /// Defaults to `Value::Object` empty.
    pub trigger_config: Value,
    /// Defaults to `Value::Object` empty.
    pub filter_config: Value,
    /// Defaults to `Value::Array` empty.
    pub channels: Value,
    /// Defaults to 10 minutes.
    pub throttle_minutes: i32,
    /// Defaults to true.
    pub enabled: bool,
    /// Defaults to None.
    pub created_by: Option<UserId>,
}

impl AlertRuleDraft {
    /// Build a new draft with sane defaults for the given
    /// workspace.
    #[must_use]
    pub fn new(
        workspace_id: WorkspaceId,
        name: impl Into<String>,
        trigger_kind: TriggerKind,
    ) -> Self {
        Self {
            workspace_id,
            project_id: None,
            name: name.into(),
            trigger_kind,
            trigger_config: Value::Object(serde_json::Map::new()),
            filter_config: Value::Object(serde_json::Map::new()),
            channels: Value::Array(Vec::new()),
            throttle_minutes: 10,
            enabled: true,
            created_by: None,
        }
    }

    /// Scope to a project.
    #[must_use]
    pub fn for_project(mut self, project_id: ProjectId) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Set trigger config JSON.
    #[must_use]
    pub fn with_trigger_config(mut self, c: Value) -> Self {
        self.trigger_config = c;
        self
    }

    /// Set filter config JSON.
    #[must_use]
    pub fn with_filter(mut self, c: Value) -> Self {
        self.filter_config = c;
        self
    }

    /// Set channels JSON array.
    #[must_use]
    pub fn with_channels(mut self, c: Value) -> Self {
        self.channels = c;
        self
    }

    /// Set throttle minutes.
    #[must_use]
    pub fn with_throttle(mut self, minutes: i32) -> Self {
        self.throttle_minutes = minutes;
        self
    }

    /// Disable the rule at creation time.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Attach creator user.
    #[must_use]
    pub fn by(mut self, user: UserId) -> Self {
        self.created_by = Some(user);
        self
    }
}

/// Patch shape — every field optional; missing field = no-op.
#[derive(Debug, Clone, Default)]
pub struct AlertRulePatch {
    /// Update display name.
    pub name: Option<String>,
    /// Update trigger config.
    pub trigger_config: Option<Value>,
    /// Update filter config.
    pub filter_config: Option<Value>,
    /// Update channels.
    pub channels: Option<Value>,
    /// Update throttle minutes.
    pub throttle_minutes: Option<i32>,
}

/// On-event evaluation context (caller fills in from K4
/// event ingest path).
#[derive(Debug, Clone)]
pub struct EventContext {
    /// Project the event landed under.
    pub project_id: ProjectId,
    /// K5 issue id.
    pub issue_id: Uuid,
    /// Error type / class.
    pub error_type: String,
    /// Environment dim.
    pub environment: String,
    /// Release dim.
    pub release: String,
    /// True for the `regression` trigger; false for
    /// `new_issue`.
    pub is_regression: bool,
}

/// Output of [`crate::AlertRuleService::try_fire_for_event`]
/// — caller uses `channels` + computed `summary` / `body` to
/// build K11 `Notification`s.
#[derive(Debug, Clone)]
pub struct MatchedRule {
    /// The rule that fired (post-claim, last_fired_at fresh).
    pub rule: AlertRule,
    /// Computed one-line summary suitable for email subject
    /// / Slack message.
    pub summary: String,
    /// Multi-line context body.
    pub body: String,
}

// ── row mapping shared with service.rs ───────────────────────

pub(crate) fn row_to_rule(row: &sqlx::postgres::PgRow) -> Result<AlertRule, crate::AlertRuleError> {
    use sqlx::Row as _;
    let trigger_kind_str: &str = row.get("trigger_kind");
    let trigger_kind = TriggerKind::from_db_str(trigger_kind_str)?;
    Ok(AlertRule {
        id: row.get("id"),
        project_id: row
            .get::<Option<Uuid>, _>("project_id")
            .map(ProjectId::from_uuid),
        name: row.get("name"),
        enabled: row.get("enabled"),
        trigger_kind,
        trigger_config: row.get("trigger_config"),
        filter_config: row.get("filter_config"),
        channels: row.get("channels"),
        throttle_minutes: row.get("throttle_minutes"),
        last_fired_at: row.get("last_fired_at"),
        muted: row.get("muted"),
        snoozed_until: row.get("snoozed_until"),
        created_at: row.get("created_at"),
        created_by: row
            .get::<Option<Uuid>, _>("created_by")
            .map(UserId::from_uuid),
        updated_at: row.get("updated_at"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn trigger_kind_round_trip() {
        for k in [
            TriggerKind::NewIssue,
            TriggerKind::Regression,
            TriggerKind::EventCount,
            TriggerKind::CrashFreeDrop,
        ] {
            assert_eq!(TriggerKind::from_db_str(k.as_db_str()).unwrap(), k);
        }
    }

    #[test]
    fn trigger_kind_is_on_event() {
        assert!(TriggerKind::NewIssue.is_on_event());
        assert!(TriggerKind::Regression.is_on_event());
        assert!(!TriggerKind::EventCount.is_on_event());
        assert!(!TriggerKind::CrashFreeDrop.is_on_event());
    }

    #[test]
    fn trigger_kind_parse_rejects_unknown() {
        assert!(TriggerKind::from_db_str("oops").is_err());
    }

    #[test]
    fn draft_builder_chains() {
        let pid = ProjectId::new();
        let user = UserId::new();
        let ws = WorkspaceId::new();
        let d = AlertRuleDraft::new(ws, "r1", TriggerKind::EventCount)
            .for_project(pid)
            .with_trigger_config(serde_json::json!({"count": 100}))
            .with_filter(serde_json::json!({"environment": "production"}))
            .with_channels(serde_json::json!([{"type": "email"}]))
            .with_throttle(30)
            .by(user)
            .disabled();
        assert_eq!(d.name, "r1");
        assert_eq!(d.project_id, Some(pid));
        assert_eq!(d.created_by, Some(user));
        assert_eq!(d.trigger_config["count"], 100);
        assert_eq!(d.filter_config["environment"], "production");
        assert_eq!(d.throttle_minutes, 30);
        assert!(!d.enabled);
    }

    #[test]
    fn is_silenced_combines_flags() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mut r = AlertRule {
            id: Uuid::now_v7(),
            project_id: None,
            name: "x".into(),
            enabled: true,
            trigger_kind: TriggerKind::NewIssue,
            trigger_config: Value::Null,
            filter_config: Value::Null,
            channels: Value::Null,
            throttle_minutes: 10,
            last_fired_at: None,
            muted: false,
            snoozed_until: None,
            created_at: now,
            created_by: None,
            updated_at: now,
        };
        assert!(!r.is_silenced(now));
        r.enabled = false;
        assert!(r.is_silenced(now));
        r.enabled = true;
        r.muted = true;
        assert!(r.is_silenced(now));
        r.muted = false;
        r.snoozed_until = Some(now + time::Duration::minutes(5));
        assert!(r.is_silenced(now));
        r.snoozed_until = Some(now - time::Duration::minutes(5));
        assert!(!r.is_silenced(now), "expired snooze ≠ silenced");
    }
}
