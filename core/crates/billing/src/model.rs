//! Typed domain models for billing + quotas.

use std::fmt;

use sentori_workspace_identity::ProjectId;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

// ── Plan ────────────────────────────────────────────────────

/// Subscription plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
    /// Free tier — every self-hosted + new SaaS account
    /// starts here.
    Free,
    /// Paid tier with substantially higher quotas.
    Pro,
    /// Custom-contract tier — effectively unlimited; use
    /// for any contract negotiated outside the standard
    /// pricing page.
    Enterprise,
}

impl Plan {
    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Pro => "pro",
            Self::Enterprise => "enterprise",
        }
    }

    /// Parse from wire form.
    ///
    /// # Errors
    ///
    /// [`PlanParseError`] for unknown strings.
    pub fn from_db_str(s: &str) -> Result<Self, PlanParseError> {
        match s {
            "free" => Ok(Self::Free),
            "pro" => Ok(Self::Pro),
            "enterprise" => Ok(Self::Enterprise),
            other => Err(PlanParseError(other.to_string())),
        }
    }

    /// Per-plan [`Limits`].
    #[must_use]
    pub const fn limits(self) -> Limits {
        match self {
            Self::Free => Limits {
                events_monthly: 100_000,
                spans_monthly: 1_000_000,
                replays_monthly: 1_000,
                retention_days: 30,
            },
            Self::Pro => Limits {
                events_monthly: 5_000_000,
                spans_monthly: 50_000_000,
                replays_monthly: 50_000,
                retention_days: 90,
            },
            // Sentinel "no cap" = i64::MAX. Caller's
            // `Limits::for_kind` returns this and the quota
            // check trivially passes.
            Self::Enterprise => Limits {
                events_monthly: i64::MAX,
                spans_monthly: i64::MAX,
                replays_monthly: i64::MAX,
                retention_days: 365,
            },
        }
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// The plan whose limits actually apply, given a subscription
/// [`PlanStatus`].
///
/// - `Canceled` / `Unpaid` → [`Plan::Free`]. The subscription has
///   ended (or lapsed past dunning); the workspace drops to the
///   free-tier quota rather than being hard-blocked, so a lapsed
///   customer keeps a working (if smaller) install. This is the
///   enforcement bite behind an operator suspend or a Stripe
///   `customer.subscription.deleted`.
/// - `PastDue` → the plan is **kept** (grace period). Stripe's
///   dunning retries a failed payment for days before giving up;
///   yanking quota on the first failure would punish transient
///   card declines.
/// - `Active` / `Trialing` → the plan as-is.
///
/// Keeping the plan column intact (rather than rewriting it to
/// `free` on cancel) means a re-activation restores the prior tier
/// without needing to re-derive it.
#[must_use]
pub const fn effective_plan(plan: Plan, status: PlanStatus) -> Plan {
    match status {
        PlanStatus::Canceled | PlanStatus::Unpaid => Plan::Free,
        PlanStatus::Active | PlanStatus::Trialing | PlanStatus::PastDue => plan,
    }
}

/// Error from [`Plan::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown plan: {0:?}")]
pub struct PlanParseError(pub String);

/// Numeric limits attached to a [`Plan`]. Use
/// [`Limits::for_kind`] to look up by counter kind in
/// quota-check call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    /// Max events per workspace per month.
    pub events_monthly: i64,
    /// Max spans per workspace per month.
    pub spans_monthly: i64,
    /// Max replay sessions per workspace per month.
    pub replays_monthly: i64,
    /// Days of raw data retention.
    pub retention_days: i32,
}

impl Limits {
    /// Look up the monthly limit for one counter kind.
    #[must_use]
    pub const fn for_kind(&self, kind: CounterKind) -> i64 {
        match kind {
            CounterKind::Events => self.events_monthly,
            CounterKind::Spans => self.spans_monthly,
            CounterKind::Replays => self.replays_monthly,
        }
    }
}

// ── PlanStatus ──────────────────────────────────────────────

/// Subscription status. Superset of Stripe Subscription
/// Status so self-hosted deployments without Stripe can
/// still represent state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Plan is active — quotas apply normally.
    Active,
    /// Payment overdue — caller may render a banner; quotas
    /// still apply.
    PastDue,
    /// Subscription canceled — caller may downgrade to Free
    /// behaviour.
    Canceled,
    /// Trial period.
    Trialing,
    /// Unpaid — typically equivalent to Canceled.
    Unpaid,
}

impl PlanStatus {
    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::PastDue => "past_due",
            Self::Canceled => "canceled",
            Self::Trialing => "trialing",
            Self::Unpaid => "unpaid",
        }
    }

    /// Parse from wire form.
    ///
    /// # Errors
    ///
    /// [`PlanStatusParseError`] for unknown strings.
    pub fn from_db_str(s: &str) -> Result<Self, PlanStatusParseError> {
        match s {
            "active" => Ok(Self::Active),
            "past_due" => Ok(Self::PastDue),
            "canceled" => Ok(Self::Canceled),
            "trialing" => Ok(Self::Trialing),
            "unpaid" => Ok(Self::Unpaid),
            other => Err(PlanStatusParseError(other.to_string())),
        }
    }
}

impl fmt::Display for PlanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error from [`PlanStatus::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown plan status: {0:?}")]
pub struct PlanStatusParseError(pub String);

// ── CounterKind ─────────────────────────────────────────────

/// Which usage counter the call is incrementing /
/// inspecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CounterKind {
    /// K4 captured events.
    Events,
    /// K6 spans.
    Spans,
    /// K8 replay sessions.
    Replays,
}

impl CounterKind {
    /// All three (stable order — tests rely on it).
    pub const ALL: [Self; 3] = [Self::Events, Self::Spans, Self::Replays];

    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Events => "events",
            Self::Spans => "spans",
            Self::Replays => "replays",
        }
    }

    /// Parse from wire form.
    ///
    /// # Errors
    ///
    /// [`CounterKindParseError`] for unknown strings.
    pub fn from_db_str(s: &str) -> Result<Self, CounterKindParseError> {
        match s {
            "events" => Ok(Self::Events),
            "spans" => Ok(Self::Spans),
            "replays" => Ok(Self::Replays),
            other => Err(CounterKindParseError(other.to_string())),
        }
    }
}

impl fmt::Display for CounterKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error from [`CounterKind::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown counter kind: {0:?}")]
pub struct CounterKindParseError(pub String);

// ── WorkspaceBilling + UsageRow ─────────────────────────────

/// `workspace_billing` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceBilling {
    /// Primary key.
    pub id: Uuid,
    /// Current plan.
    pub plan: Plan,
    /// Stripe Customer object id once linked.
    pub stripe_customer_id: Option<String>,
    /// Subscription status.
    pub status: PlanStatus,
    /// Current billing period end (None for Free).
    pub current_period_end: Option<OffsetDateTime>,
    /// Creation ts.
    pub created_at: OffsetDateTime,
    /// Last update ts.
    pub updated_at: OffsetDateTime,
}

/// `usage_counters` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRow {
    /// Project the counter belongs to.
    pub project_id: ProjectId,
    /// YYYYMM period bucket.
    pub period_yyyymm: String,
    /// Which counter.
    pub counter_kind: CounterKind,
    /// Successfully recorded count this period.
    pub count: i64,
    /// Dropped (over-limit) count this period.
    pub dropped_count: i64,
    /// Last update ts.
    pub updated_at: OffsetDateTime,
}

// ── Decision ────────────────────────────────────────────────

/// Outcome of [`crate::BillingService::check_and_record`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Recorded; still under limit (`new_count < limit`).
    Allow {
        /// New cumulative count after this call.
        new_count: i64,
        /// The limit for this counter+plan.
        limit: i64,
    },
    /// Recorded; exactly at the limit (`new_count == limit`).
    /// Caller may render a warn banner — next call drops.
    AtLimit {
        /// `new_count == limit`.
        new_count: i64,
        /// Limit.
        limit: i64,
    },
    /// NOT recorded — counter would have exceeded the
    /// limit. Caller drops the request + may call
    /// [`crate::BillingService::record_drop`] for
    /// observability.
    OverLimit {
        /// Current cumulative count (unchanged by this call).
        current_count: i64,
        /// The limit that would have been exceeded.
        limit: i64,
    },
}

impl Decision {
    /// Returns true for [`Self::Allow`] + [`Self::AtLimit`]
    /// — i.e. the increment was recorded.
    #[must_use]
    pub const fn was_recorded(&self) -> bool {
        matches!(self, Self::Allow { .. } | Self::AtLimit { .. })
    }

    /// True when caller should drop the request.
    #[must_use]
    pub const fn is_over_limit(&self) -> bool {
        matches!(self, Self::OverLimit { .. })
    }
}

// ── row mapping ─────────────────────────────────────────────

pub(crate) fn row_to_billing(
    row: &sqlx::postgres::PgRow,
) -> Result<WorkspaceBilling, crate::BillingError> {
    use sqlx::Row as _;
    let plan = Plan::from_db_str(row.get("plan"))?;
    let status = PlanStatus::from_db_str(row.get("status"))?;
    Ok(WorkspaceBilling {
        id: row.get("id"),
        plan,
        stripe_customer_id: row.get("stripe_customer_id"),
        status,
        current_period_end: row.get("current_period_end"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(crate) fn row_to_usage(row: &sqlx::postgres::PgRow) -> Result<UsageRow, crate::BillingError> {
    use sqlx::Row as _;
    let counter_kind = CounterKind::from_db_str(row.get("counter_kind"))?;
    Ok(UsageRow {
        project_id: ProjectId::from_uuid(row.get("project_id")),
        period_yyyymm: row.get("period_yyyymm"),
        counter_kind,
        count: row.get("count"),
        dropped_count: row.get("dropped_count"),
        updated_at: row.get("updated_at"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn plan_round_trip() {
        for p in [Plan::Free, Plan::Pro, Plan::Enterprise] {
            assert_eq!(Plan::from_db_str(p.as_db_str()).unwrap(), p);
        }
    }

    #[test]
    fn plan_limits_ordering() {
        // Free ≤ Pro ≤ Enterprise on every dimension.
        for k in CounterKind::ALL {
            let free = Plan::Free.limits().for_kind(k);
            let pro = Plan::Pro.limits().for_kind(k);
            let ent = Plan::Enterprise.limits().for_kind(k);
            assert!(free <= pro, "free ≤ pro {k}");
            assert!(pro <= ent, "pro ≤ ent {k}");
        }
        assert!(Plan::Free.limits().retention_days <= Plan::Pro.limits().retention_days);
        assert!(Plan::Pro.limits().retention_days <= Plan::Enterprise.limits().retention_days);
    }

    #[test]
    fn enterprise_is_effectively_unlimited() {
        let ent = Plan::Enterprise.limits();
        assert_eq!(ent.events_monthly, i64::MAX);
        assert_eq!(ent.spans_monthly, i64::MAX);
        assert_eq!(ent.replays_monthly, i64::MAX);
    }

    #[test]
    fn status_round_trip() {
        for s in [
            PlanStatus::Active,
            PlanStatus::PastDue,
            PlanStatus::Canceled,
            PlanStatus::Trialing,
            PlanStatus::Unpaid,
        ] {
            assert_eq!(PlanStatus::from_db_str(s.as_db_str()).unwrap(), s);
        }
    }

    #[test]
    fn effective_plan_downgrades_only_on_canceled_or_unpaid() {
        // Grace + healthy states keep the paid plan.
        for status in [
            PlanStatus::Active,
            PlanStatus::Trialing,
            PlanStatus::PastDue,
        ] {
            assert_eq!(effective_plan(Plan::Pro, status), Plan::Pro, "{status}");
            assert_eq!(
                effective_plan(Plan::Enterprise, status),
                Plan::Enterprise,
                "{status}"
            );
        }
        // Ended states drop to Free limits regardless of prior plan.
        for status in [PlanStatus::Canceled, PlanStatus::Unpaid] {
            assert_eq!(effective_plan(Plan::Pro, status), Plan::Free, "{status}");
            assert_eq!(
                effective_plan(Plan::Enterprise, status),
                Plan::Free,
                "{status}"
            );
        }
        // Free stays Free everywhere.
        for status in [
            PlanStatus::Active,
            PlanStatus::Trialing,
            PlanStatus::PastDue,
            PlanStatus::Canceled,
            PlanStatus::Unpaid,
        ] {
            assert_eq!(effective_plan(Plan::Free, status), Plan::Free, "{status}");
        }
    }

    #[test]
    fn counter_kind_round_trip() {
        for k in CounterKind::ALL {
            assert_eq!(CounterKind::from_db_str(k.as_db_str()).unwrap(), k);
        }
    }

    #[test]
    fn decision_was_recorded() {
        assert!(
            Decision::Allow {
                new_count: 1,
                limit: 10
            }
            .was_recorded()
        );
        assert!(
            Decision::AtLimit {
                new_count: 10,
                limit: 10
            }
            .was_recorded()
        );
        assert!(
            !Decision::OverLimit {
                current_count: 10,
                limit: 10
            }
            .was_recorded()
        );
        assert!(
            Decision::OverLimit {
                current_count: 10,
                limit: 10
            }
            .is_over_limit()
        );
    }
}
