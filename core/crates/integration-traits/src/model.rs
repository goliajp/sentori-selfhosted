//! Typed domain models for [`crate::IntegrationService`].

use sentori_workspace_identity::{ProjectId, UserId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

/// How a user goes from "not connected" → "connected" for a
/// given adapter. Drives the UI flow + API surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectMode {
    /// OAuth — user clicks Connect, app builds an
    /// authorise URL, vendor calls back, we exchange code
    /// for a token. Linear / Jira / GitHub / GitLab.
    OAuth,
    /// Manual — user pastes credentials (Slack incoming
    /// webhook URL is the canonical example). No vendor
    /// callback; `accept_manual_config` validates inline.
    Manual,
}

/// Issue lifecycle transition that triggers an adapter call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueLifecycleEvent {
    /// First seen — adapter creates upstream item.
    Created,
    /// Resolved issue had a fresh event — adapter re-opens.
    Regressed,
    /// Operator marked resolved — adapter closes / comments.
    Resolved,
}

/// Per-issue context handed to
/// [`crate::IntegrationAdapter::create_issue`] +
/// [`crate::IntegrationAdapter::update_status`]. Keep
/// adapter-agnostic so each vendor maps fields to its own
/// payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueContext {
    /// Sentori issue id.
    pub issue_id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Top-level error type (e.g. `TypeError`,
    /// `NullPointerException`).
    pub error_type: String,
    /// Issue message (single-line summary suitable for
    /// upstream item title).
    pub error_message: String,
    /// Release that produced the issue (semver / git sha).
    pub release: String,
    /// `production` / `staging` / `development`.
    pub environment: String,
    /// Dashboard URL for the issue — adapter embeds as a
    /// back-link.
    pub url: String,
    /// Aggregate event count at dispatch time.
    pub event_count: i64,
    /// Top in-app frame's `file:line`, if known.
    #[serde(default)]
    pub crash_site: Option<String>,
}

/// Returned by [`crate::IntegrationAdapter::create_issue`] —
/// caller persists into `issue_integration_links`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRef {
    /// Upstream item id (Linear issue id, Slack message ts,
    /// GitHub issue number, …). String to avoid
    /// constraining vendor id shapes.
    pub external_id: String,
    /// Browser-clickable URL.
    pub external_url: String,
}

/// `integrations` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationConfig {
    /// Primary key.
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Adapter `kind()` value.
    pub kind: String,
    /// Vendor-specific config JSON.
    pub config: Value,
    /// User who connected (None if connected via
    /// migration / boot script).
    pub connected_by: Option<UserId>,
    /// When connected.
    #[serde(with = "time::serde::rfc3339")]
    pub connected_at: OffsetDateTime,
    /// Active flag — operator deactivates without losing
    /// the OAuth token.
    pub active: bool,
}

/// `issue_integration_links` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueIntegrationLink {
    /// Primary key.
    pub id: Uuid,
    /// K5 issue id.
    pub issue_id: Uuid,
    /// Adapter `kind()` value.
    pub kind: String,
    /// Upstream id.
    pub external_id: String,
    /// Upstream URL.
    pub external_url: String,
    /// When the link was created.
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// Aggregate result of one
/// [`crate::IntegrationService::dispatch`] call.
#[derive(Debug, Clone)]
pub struct DispatchOutcome {
    /// Adapters that opened / updated successfully, with
    /// the (kind, ExternalRef) pair.
    pub successes: Vec<(String, ExternalRef)>,
    /// Adapters that skipped (no config / not active /
    /// already linked for Created event).
    pub skipped: Vec<(String, String)>,
    /// Adapters that errored — caller logs + decides retry.
    pub failures: Vec<(String, String)>,
}

impl DispatchOutcome {
    pub(crate) fn new() -> Self {
        Self {
            successes: Vec::new(),
            skipped: Vec::new(),
            failures: Vec::new(),
        }
    }

    /// Total adapters attempted.
    #[must_use]
    pub fn total(&self) -> usize {
        self.successes.len() + self.skipped.len() + self.failures.len()
    }

    /// True when every attempted adapter succeeded or was
    /// deliberately skipped (no failures).
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }
}

// ── row mapping ──────────────────────────────────────────────

#[allow(clippy::unnecessary_wraps)]
pub(crate) fn row_to_config(
    row: &sqlx::postgres::PgRow,
) -> Result<IntegrationConfig, crate::IntegrationError> {
    use sqlx::Row as _;
    Ok(IntegrationConfig {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        kind: row.get("kind"),
        config: row.get("config"),
        connected_by: row
            .get::<Option<Uuid>, _>("connected_by")
            .map(UserId::from_uuid),
        connected_at: row.get("connected_at"),
        active: row.get("active"),
    })
}

#[allow(clippy::unnecessary_wraps)]
pub(crate) fn row_to_link(
    row: &sqlx::postgres::PgRow,
) -> Result<IssueIntegrationLink, crate::IntegrationError> {
    use sqlx::Row as _;
    Ok(IssueIntegrationLink {
        id: row.get("id"),
        issue_id: row.get("issue_id"),
        kind: row.get("kind"),
        external_id: row.get("external_id"),
        external_url: row.get("external_url"),
        created_at: row.get("created_at"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn outcome_starts_empty() {
        let o = DispatchOutcome::new();
        assert_eq!(o.total(), 0);
        assert!(o.is_clean());
    }

    #[test]
    fn outcome_total_sums_all_buckets() {
        let mut o = DispatchOutcome::new();
        o.successes.push((
            "slack".into(),
            ExternalRef {
                external_id: "1".into(),
                external_url: "x".into(),
            },
        ));
        o.skipped.push(("linear".into(), "no config".into()));
        o.failures.push(("jira".into(), "boom".into()));
        assert_eq!(o.total(), 3);
        assert!(!o.is_clean());
    }
}
