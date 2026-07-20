//! [`BillingService`] — plan CRUD + atomic quota check.

use sentori_workspace_identity::{ProjectId, WorkspaceId};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::BillingError;
use crate::model::{
    CounterKind, Decision, Plan, PlanStatus, UsageRow, WorkspaceBilling, row_to_billing,
    row_to_usage,
};
use crate::period::period_key;

const BILLING_COLS: &str = r"
    id, plan, stripe_customer_id, status,
    current_period_end, created_at, updated_at
";

const USAGE_COLS: &str = r"
    project_id, period_yyyymm, counter_kind,
    count, dropped_count, updated_at
";

/// Public handle. Workspace-scoped — one billing row per
/// workspace (2026-06-22 single-DB pivot: was global singleton,
/// now keyed on workspace_id).
#[derive(Clone, Debug)]
pub struct BillingService {
    pool: PgPool,
    workspace_id: WorkspaceId,
}

impl BillingService {
    /// Construct with the workspace scope.
    #[must_use]
    pub const fn new(pool: PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// The workspace this billing service is scoped to.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    // ── billing CRUD ────────────────────────────────────────

    /// Insert this workspace's `workspace_billing` row at Free
    /// plan if absent. Idempotent.
    ///
    /// # Errors
    ///
    /// [`BillingError::Db`] on backend failure.
    pub async fn ensure_default(&self) -> Result<bool, BillingError> {
        let id = Uuid::now_v7();
        let row: Option<(Uuid,)> = sqlx::query_as(
            r"
            INSERT INTO workspace_billing
                (id, workspace_id, plan, status)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (workspace_id) DO NOTHING
            RETURNING id
            ",
        )
        .bind(id)
        .bind(self.workspace_id.into_uuid())
        .bind(Plan::Free.as_db_str())
        .bind(PlanStatus::Active.as_db_str())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// Get this workspace's billing row.
    ///
    /// # Errors
    ///
    /// [`BillingError::NotInitialised`] if no row exists.
    /// [`BillingError::Db`] on backend failure.
    pub async fn get(&self) -> Result<WorkspaceBilling, BillingError> {
        let sql = format!(
            "SELECT {BILLING_COLS} FROM workspace_billing \
             WHERE workspace_id = $1 LIMIT 1"
        );
        let row = sqlx::query(&sql)
            .bind(self.workspace_id.into_uuid())
            .fetch_optional(&self.pool)
            .await?
            .ok_or(BillingError::NotInitialised)?;
        row_to_billing(&row)
    }

    /// Set the plan + optional Stripe customer ref +
    /// period_end for this workspace.
    ///
    /// # Errors
    ///
    /// [`BillingError::Db`] on backend failure.
    pub async fn set_plan(
        &self,
        plan: Plan,
        stripe_customer_id: Option<&str>,
        current_period_end: Option<OffsetDateTime>,
    ) -> Result<(), BillingError> {
        let id = Uuid::now_v7();
        sqlx::query(
            r"
            INSERT INTO workspace_billing
                (id, workspace_id, plan, stripe_customer_id, current_period_end, status)
            VALUES ($1, $2, $3, $4, $5, 'active')
            ON CONFLICT (workspace_id) DO UPDATE SET
                plan = EXCLUDED.plan,
                stripe_customer_id = COALESCE(EXCLUDED.stripe_customer_id,
                                              workspace_billing.stripe_customer_id),
                current_period_end = EXCLUDED.current_period_end,
                updated_at = now()
            ",
        )
        .bind(id)
        .bind(self.workspace_id.into_uuid())
        .bind(plan.as_db_str())
        .bind(stripe_customer_id)
        .bind(current_period_end)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Set the subscription status for this workspace.
    ///
    /// # Errors
    ///
    /// [`BillingError::NotInitialised`] if no row. /
    /// [`BillingError::Db`] on backend failure.
    pub async fn set_status(&self, status: PlanStatus) -> Result<(), BillingError> {
        let res = sqlx::query(
            "UPDATE workspace_billing SET status = $1, updated_at = now() \
             WHERE workspace_id = $2",
        )
        .bind(status.as_db_str())
        .bind(self.workspace_id.into_uuid())
        .execute(&self.pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(BillingError::NotInitialised);
        }
        Ok(())
    }

    // ── quota check ─────────────────────────────────────────

    /// Atomic check-and-record. Increments the
    /// `(project, period, counter_kind)` row by `delta` then
    /// compares vs the workspace plan's limit. Returns
    /// [`Decision::Allow`] when under, [`Decision::AtLimit`]
    /// when exactly at, [`Decision::OverLimit`] when the
    /// increment would push past the limit (in which case
    /// the row is rolled back — net effect is no change).
    ///
    /// The over-limit path uses a CTE to:
    /// 1. INSERT…ON CONFLICT…RETURNING the would-be new count.
    /// 2. If over-limit, DELETE the row OR DECREMENT — for
    ///    UPSERT we instead use a CHECK in the math: do the
    ///    increment iff `count + delta <= limit`.
    ///
    /// In v0.1, we do INSERT/UPDATE conditionally based on
    /// a pre-read + compare. Race losers retry.
    ///
    /// # Errors
    ///
    /// - [`BillingError::InvalidInput`] for `delta <= 0`.
    /// - [`BillingError::NotInitialised`] if `ensure_default`
    ///   hasn't been called.
    /// - [`BillingError::ProjectNotFound`] on FK violation.
    /// - [`BillingError::Db`] on backend failure.
    pub async fn check_and_record(
        &self,
        project_id: ProjectId,
        kind: CounterKind,
        delta: i64,
        now: OffsetDateTime,
    ) -> Result<Decision, BillingError> {
        if delta <= 0 {
            return Err(BillingError::InvalidInput("delta must be > 0".into()));
        }
        let billing = self.get().await?;
        let limit = billing.plan.limits().for_kind(kind);
        let period = period_key(now);

        // Atomic compare-and-increment via single statement.
        // The WHERE clause caps the UPDATE; INSERT branch
        // applies only when row is absent. Returns the new
        // count when the increment ran, NULL otherwise.
        //
        // We use two paths to keep the SQL readable —
        // try-update first, then INSERT on absence.
        let updated: Option<(i64,)> = sqlx::query_as(
            r"
            UPDATE usage_counters
            SET count = count + $4, updated_at = now()
            WHERE project_id = $1
              AND period_yyyymm = $2
              AND counter_kind = $3
              AND count + $4 <= $5
            RETURNING count
            ",
        )
        .bind(project_id.into_uuid())
        .bind(&period)
        .bind(kind.as_db_str())
        .bind(delta)
        .bind(limit)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| translate_fk(e, project_id))?;

        if let Some((new_count,)) = updated {
            return Ok(if new_count == limit {
                Decision::AtLimit { new_count, limit }
            } else {
                Decision::Allow { new_count, limit }
            });
        }

        // UPDATE didn't fire — either the row is missing
        // (try INSERT) or the increment would have busted
        // the cap (compute current_count, return OverLimit).
        // Use a single statement that INSERTs OR reads the
        // existing count.
        let inserted: Option<(i64,)> = sqlx::query_as(
            r"
            INSERT INTO usage_counters
                (workspace_id, project_id, period_yyyymm, counter_kind, count)
            SELECT p.workspace_id, $1, $2, $3, $4 FROM projects p WHERE p.id = $1
            ON CONFLICT (project_id, period_yyyymm, counter_kind) DO NOTHING
            RETURNING count
            ",
        )
        .bind(project_id.into_uuid())
        .bind(&period)
        .bind(kind.as_db_str())
        .bind(delta)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| translate_fk(e, project_id))?;

        if let Some((new_count,)) = inserted {
            // Row was absent — fresh insert. delta is the
            // new count. delta might already exceed limit if
            // caller passes a huge delta on Free plan.
            return Ok(if new_count >= limit {
                if new_count > limit {
                    // Insert succeeded but exceeded. Roll it
                    // back to zero + report over-limit.
                    sqlx::query("DELETE FROM usage_counters \
                                 WHERE project_id = $1 AND period_yyyymm = $2 AND counter_kind = $3")
                        .bind(project_id.into_uuid())
                        .bind(&period)
                        .bind(kind.as_db_str())
                        .execute(&self.pool)
                        .await?;
                    Decision::OverLimit {
                        current_count: 0,
                        limit,
                    }
                } else {
                    Decision::AtLimit { new_count, limit }
                }
            } else {
                Decision::Allow { new_count, limit }
            });
        }

        // INSERT conflicted → row exists and the prior
        // UPDATE was blocked by the cap. Read the current
        // count + return OverLimit.
        let current = self
            .read_count(project_id, &period, kind)
            .await?
            .unwrap_or(0);
        Ok(Decision::OverLimit {
            current_count: current,
            limit,
        })
    }

    /// Record a dropped event (over-limit) on the
    /// `dropped_count` field of the same row. Caller
    /// usually invokes after `check_and_record` returns
    /// [`Decision::OverLimit`].
    ///
    /// Idempotent at the row level (UPSERT) but cumulative
    /// (delta adds to dropped_count). delta must be > 0.
    ///
    /// # Errors
    ///
    /// - [`BillingError::InvalidInput`] for `delta <= 0`.
    /// - [`BillingError::ProjectNotFound`] on FK violation.
    /// - [`BillingError::Db`] on backend failure.
    pub async fn record_drop(
        &self,
        project_id: ProjectId,
        kind: CounterKind,
        delta: i64,
        now: OffsetDateTime,
    ) -> Result<(), BillingError> {
        if delta <= 0 {
            return Err(BillingError::InvalidInput("delta must be > 0".into()));
        }
        let period = period_key(now);
        sqlx::query(
            r"
            INSERT INTO usage_counters
                (workspace_id, project_id, period_yyyymm, counter_kind, count, dropped_count)
            SELECT p.workspace_id, $1, $2, $3, 0, $4 FROM projects p WHERE p.id = $1
            ON CONFLICT (project_id, period_yyyymm, counter_kind) DO UPDATE SET
                dropped_count = usage_counters.dropped_count + $4,
                updated_at = now()
            ",
        )
        .bind(project_id.into_uuid())
        .bind(&period)
        .bind(kind.as_db_str())
        .bind(delta)
        .execute(&self.pool)
        .await
        .map_err(|e| translate_fk(e, project_id))?;
        Ok(())
    }

    // ── usage read ──────────────────────────────────────────

    /// All counter rows for one (project, period).
    ///
    /// # Errors
    ///
    /// [`BillingError::Db`] on backend failure.
    pub async fn usage(
        &self,
        project_id: ProjectId,
        period_yyyymm: &str,
    ) -> Result<Vec<UsageRow>, BillingError> {
        let sql = format!(
            "SELECT {USAGE_COLS} FROM usage_counters \
             WHERE project_id = $1 AND period_yyyymm = $2 \
             ORDER BY counter_kind ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(project_id.into_uuid())
            .bind(period_yyyymm)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_usage).collect()
    }

    /// Sum of all counters across every project for one
    /// period — dashboard "this month workspace-wide" panel.
    ///
    /// # Errors
    ///
    /// [`BillingError::Db`] on backend failure.
    pub async fn workspace_usage(
        &self,
        period_yyyymm: &str,
    ) -> Result<Vec<(CounterKind, i64, i64)>, BillingError> {
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT counter_kind, SUM(count)::bigint, SUM(dropped_count)::bigint \
             FROM usage_counters WHERE period_yyyymm = $1 \
             GROUP BY counter_kind ORDER BY counter_kind ASC",
        )
        .bind(period_yyyymm)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|(kind, count, dropped)| {
                CounterKind::from_db_str(&kind)
                    .map(|k| (k, count, dropped))
                    .map_err(Into::into)
            })
            .collect()
    }

    // ── internals ───────────────────────────────────────────

    async fn read_count(
        &self,
        project_id: ProjectId,
        period: &str,
        kind: CounterKind,
    ) -> Result<Option<i64>, BillingError> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT count FROM usage_counters \
             WHERE project_id = $1 AND period_yyyymm = $2 AND counter_kind = $3",
        )
        .bind(project_id.into_uuid())
        .bind(period)
        .bind(kind.as_db_str())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(c,)| c))
    }
}

fn translate_fk(err: sqlx::Error, project_id: ProjectId) -> BillingError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        return BillingError::ProjectNotFound(project_id.into_uuid());
    }
    BillingError::Db(err)
}
