//! Typed domain models for cert-monitor.

use sentori_workspace_identity::{ProjectId, UserId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// `cert_watch_domains` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedDomain {
    /// Primary key.
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Apex domain being watched.
    pub domain: String,
    /// Who added the watch (NULL if added by a tool /
    /// migration with no actor).
    pub added_by: Option<UserId>,
    /// When the watch was added.
    #[serde(with = "time::serde::rfc3339")]
    pub added_at: OffsetDateTime,
    /// Last successful crt.sh poll. NULL until first poll.
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_polled_at: Option<OffsetDateTime>,
}

/// `cert_observations` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertObservation {
    /// Primary key.
    pub id: Uuid,
    /// Owning project.
    pub project_id: ProjectId,
    /// Domain the cert was matched against.
    pub domain: String,
    /// crt.sh's integer cert id.
    pub cert_id: i64,
    /// Cert's CN.
    pub common_name: Option<String>,
    /// SAN list (comma-separated, may be truncated to 8 KB).
    pub name_value: Option<String>,
    /// Cert's issuer string.
    pub issuer_name: String,
    /// Cert validity start.
    #[serde(with = "time::serde::rfc3339")]
    pub not_before: OffsetDateTime,
    /// Cert validity end.
    #[serde(with = "time::serde::rfc3339")]
    pub not_after: OffsetDateTime,
    /// When K10 first observed this cert.
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

impl CertObservation {
    /// True if `not_after` is on or before `now + within`.
    /// Operator's "expires soon" badge predicate.
    #[must_use]
    pub fn expires_within(&self, now: OffsetDateTime, within: time::Duration) -> bool {
        self.not_after <= now + within
    }
}

/// Aggregate result of a single [`crate::CertMonitor::poll_once`] call.
#[derive(Debug, Clone, Default)]
pub struct PollOutcome {
    /// Number of distinct watched domains polled.
    pub domains_polled: usize,
    /// Domains where the poll succeeded without error.
    pub domains_ok: usize,
    /// Newly-observed certs across all domains (i.e. ON
    /// CONFLICT … RETURNING returned a row).
    pub new_observations: Vec<CertObservation>,
    /// Domain → error message map for failures. Caller logs
    /// these and continues; per-domain failure does NOT abort
    /// the rest of the poll.
    pub per_domain_errors: Vec<(String, String)>,
}

impl PollOutcome {
    /// How many domains errored on this tick.
    #[must_use]
    pub const fn domains_failed(&self) -> usize {
        self.per_domain_errors.len()
    }

    /// Total new certs surfaced this tick.
    #[must_use]
    pub const fn new_count(&self) -> usize {
        self.new_observations.len()
    }
}

// ── helpers shared with monitor.rs ──────────────────────────

#[allow(clippy::unnecessary_wraps)]
pub(crate) fn row_to_watched(
    row: &sqlx::postgres::PgRow,
) -> Result<WatchedDomain, crate::CertMonitorError> {
    use sqlx::Row as _;
    Ok(WatchedDomain {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        domain: row.get("domain"),
        added_by: row
            .get::<Option<Uuid>, _>("added_by")
            .map(UserId::from_uuid),
        added_at: row.get("added_at"),
        last_polled_at: row.get("last_polled_at"),
    })
}

#[allow(clippy::unnecessary_wraps)]
pub(crate) fn row_to_observation(
    row: &sqlx::postgres::PgRow,
) -> Result<CertObservation, crate::CertMonitorError> {
    use sqlx::Row as _;
    Ok(CertObservation {
        id: row.get("id"),
        project_id: ProjectId::from_uuid(row.get("project_id")),
        domain: row.get("domain"),
        cert_id: row.get("cert_id"),
        common_name: row.get("common_name"),
        name_value: row.get("name_value"),
        issuer_name: row.get("issuer_name"),
        not_before: row.get("not_before"),
        not_after: row.get("not_after"),
        observed_at: row.get("observed_at"),
    })
}
