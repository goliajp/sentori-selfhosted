//! [`AlertRuleService`] — rule CRUD + on-event fire + atomic
//! throttle claim.

use sentori_workspace_identity::{ProjectId, UserId};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AlertRuleError;
use crate::filter::matches_filter;
use crate::model::{
    AlertRule, AlertRuleDraft, AlertRulePatch, EventContext, MatchedRule, TriggerKind, row_to_rule,
};

const MAX_NAME_LEN: usize = 200;

const SELECT_COLS: &str = r"
    id, project_id, name, enabled,
    trigger_kind, trigger_config, filter_config, channels,
    throttle_minutes, last_fired_at, muted, snoozed_until,
    created_at, created_by, updated_at
";

/// Public handle.
#[derive(Clone, Debug)]
pub struct AlertRuleService {
    pool: PgPool,
}

impl AlertRuleService {
    /// Construct.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ── CRUD ────────────────────────────────────────────────

    /// Create a rule. Returns the new id.
    ///
    /// # Errors
    ///
    /// - [`AlertRuleError::InvalidInput`] on validation fail.
    /// - [`AlertRuleError::ProjectNotFound`] /
    ///   [`AlertRuleError::UserNotFound`] on FK fail.
    /// - [`AlertRuleError::Db`].
    pub async fn create_rule(&self, draft: AlertRuleDraft) -> Result<Uuid, AlertRuleError> {
        validate_draft(&draft)?;
        let id = Uuid::now_v7();
        let row: (Uuid,) = sqlx::query_as(
            r"
            INSERT INTO alert_rules
                (id, workspace_id, project_id, name, enabled,
                 trigger_kind, trigger_config, filter_config, channels,
                 throttle_minutes, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id
            ",
        )
        .bind(id)
        .bind(draft.workspace_id.into_uuid())
        .bind(draft.project_id.map(ProjectId::into_uuid))
        .bind(&draft.name)
        .bind(draft.enabled)
        .bind(draft.trigger_kind.as_db_str())
        .bind(&draft.trigger_config)
        .bind(&draft.filter_config)
        .bind(&draft.channels)
        .bind(draft.throttle_minutes)
        .bind(draft.created_by.map(UserId::into_uuid))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| translate_fk(e, draft.project_id, draft.created_by))?;
        Ok(row.0)
    }

    /// Apply a partial update — only fields set in `patch`
    /// are written. `updated_at` is bumped to now() on any
    /// touched field. Idempotent — calling with all-None
    /// patch is a no-op.
    ///
    /// # Errors
    ///
    /// - [`AlertRuleError::RuleNotFound`] if no row.
    /// - [`AlertRuleError::Db`].
    pub async fn update(&self, id: Uuid, patch: AlertRulePatch) -> Result<(), AlertRuleError> {
        let result = sqlx::query(
            r"
            UPDATE alert_rules SET
                name             = COALESCE($2, name),
                trigger_config   = COALESCE($3, trigger_config),
                filter_config    = COALESCE($4, filter_config),
                channels         = COALESCE($5, channels),
                throttle_minutes = COALESCE($6, throttle_minutes),
                updated_at       = now()
            WHERE id = $1
            ",
        )
        .bind(id)
        .bind(patch.name.as_deref())
        .bind(patch.trigger_config.as_ref())
        .bind(patch.filter_config.as_ref())
        .bind(patch.channels.as_ref())
        .bind(patch.throttle_minutes)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AlertRuleError::RuleNotFound(id));
        }
        Ok(())
    }

    /// Toggle enabled. Idempotent.
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::RuleNotFound`] if no row.
    pub async fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<(), AlertRuleError> {
        let res =
            sqlx::query("UPDATE alert_rules SET enabled = $2, updated_at = now() WHERE id = $1")
                .bind(id)
                .bind(enabled)
                .execute(&self.pool)
                .await?;
        if res.rows_affected() == 0 {
            return Err(AlertRuleError::RuleNotFound(id));
        }
        Ok(())
    }

    /// Toggle muted. Idempotent.
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::RuleNotFound`] if no row.
    pub async fn set_muted(&self, id: Uuid, muted: bool) -> Result<(), AlertRuleError> {
        let res =
            sqlx::query("UPDATE alert_rules SET muted = $2, updated_at = now() WHERE id = $1")
                .bind(id)
                .bind(muted)
                .execute(&self.pool)
                .await?;
        if res.rows_affected() == 0 {
            return Err(AlertRuleError::RuleNotFound(id));
        }
        Ok(())
    }

    /// Set snoozed_until to `until` (None = clear snooze).
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::RuleNotFound`] if no row.
    pub async fn snooze(
        &self,
        id: Uuid,
        until: Option<OffsetDateTime>,
    ) -> Result<(), AlertRuleError> {
        let res = sqlx::query(
            "UPDATE alert_rules SET snoozed_until = $2, updated_at = now() WHERE id = $1",
        )
        .bind(id)
        .bind(until)
        .execute(&self.pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(AlertRuleError::RuleNotFound(id));
        }
        Ok(())
    }

    /// Delete. Idempotent.
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::Db`] on backend failure.
    pub async fn delete(&self, id: Uuid) -> Result<(), AlertRuleError> {
        sqlx::query("DELETE FROM alert_rules WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Find by id.
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::Db`] on backend failure.
    pub async fn find(&self, id: Uuid) -> Result<Option<AlertRule>, AlertRuleError> {
        let sql = format!("SELECT {SELECT_COLS} FROM alert_rules WHERE id = $1");
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_rule).transpose()
    }

    /// List every rule for a project (project-scoped + workspace-wide).
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::Db`] on backend failure.
    pub async fn list_for_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<AlertRule>, AlertRuleError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM alert_rules \
             WHERE project_id = $1 OR project_id IS NULL \
             ORDER BY created_at ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(project_id.into_uuid())
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_rule).collect()
    }

    /// List workspace-wide rules (project_id IS NULL).
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::Db`] on backend failure.
    pub async fn list_workspace_wide(&self) -> Result<Vec<AlertRule>, AlertRuleError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM alert_rules \
             WHERE project_id IS NULL \
             ORDER BY created_at ASC"
        );
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        rows.iter().map(row_to_rule).collect()
    }

    /// List enabled + active rules of `kind`. Filters out
    /// muted and currently-snoozed rows. Used by caller cron
    /// (e.g. event_count / crash_free_drop loop).
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::Db`] on backend failure.
    pub async fn list_active_by_kind(
        &self,
        kind: TriggerKind,
    ) -> Result<Vec<AlertRule>, AlertRuleError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM alert_rules \
             WHERE enabled = TRUE AND muted = FALSE \
               AND (snoozed_until IS NULL OR snoozed_until < now()) \
               AND trigger_kind = $1 \
             ORDER BY last_fired_at NULLS FIRST"
        );
        let rows = sqlx::query(&sql)
            .bind(kind.as_db_str())
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_rule).collect()
    }

    // ── on-event fire ───────────────────────────────────────

    /// On-event fire path. For each enabled + active rule
    /// of kind NewIssue (or Regression when
    /// `ctx.is_regression`) matching the filter + this
    /// project (or workspace-wide), atomically claim the
    /// throttle slot via [`Self::try_claim`] and return the
    /// matched rule. Caller iterates the returned list +
    /// builds K11 `Notification`s from `.channels`.
    ///
    /// Filter eval is pure in-process (no DB round trip);
    /// the throttle UPDATE is one statement per matched rule.
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::Db`] on backend failure.
    pub async fn try_fire_for_event(
        &self,
        ctx: &EventContext,
    ) -> Result<Vec<MatchedRule>, AlertRuleError> {
        let kind = if ctx.is_regression {
            TriggerKind::Regression
        } else {
            TriggerKind::NewIssue
        };
        let sql = format!(
            "SELECT {SELECT_COLS} FROM alert_rules \
             WHERE trigger_kind = $1 \
               AND enabled = TRUE \
               AND muted = FALSE \
               AND (snoozed_until IS NULL OR snoozed_until < now()) \
               AND (project_id IS NULL OR project_id = $2)"
        );
        let rows = sqlx::query(&sql)
            .bind(kind.as_db_str())
            .bind(ctx.project_id.into_uuid())
            .fetch_all(&self.pool)
            .await?;
        let mut matched = Vec::new();
        for row in &rows {
            let rule = row_to_rule(row)?;
            if !matches_filter(
                &rule.filter_config,
                &ctx.error_type,
                &ctx.environment,
                &ctx.release,
            ) {
                continue;
            }
            if !self.try_claim(rule.id, rule.throttle_minutes).await? {
                continue;
            }
            // Reload to capture the freshly-stamped last_fired_at.
            let claimed = self.find(rule.id).await?.unwrap_or(rule);
            let summary = build_summary(ctx);
            let body = build_body(ctx);
            matched.push(MatchedRule {
                rule: claimed,
                summary,
                body,
            });
        }
        Ok(matched)
    }

    /// Atomic throttle claim. Returns Ok(true) when this
    /// caller successfully claimed the fire slot — i.e.
    /// no other evaluator has fired the rule within the
    /// throttle window. Caller proceeds to dispatch on
    /// `true`, skips on `false`.
    ///
    /// The WHERE clause + RETURNING in the same UPDATE
    /// statement means two evaluators racing the same rule
    /// cannot both win.
    ///
    /// # Errors
    ///
    /// [`AlertRuleError::Db`] on backend failure.
    pub async fn try_claim(
        &self,
        rule_id: Uuid,
        throttle_minutes: i32,
    ) -> Result<bool, AlertRuleError> {
        // Use a parameterised interval expression — sqlx
        // doesn't bind PG `INTERVAL` directly from i32, so
        // we build the expression with a literal minutes
        // value AFTER clamping.
        let minutes = throttle_minutes.max(0);
        let sql = format!(
            "UPDATE alert_rules \
             SET last_fired_at = now() \
             WHERE id = $1 \
               AND (last_fired_at IS NULL \
                    OR last_fired_at < now() - interval '{minutes} minutes') \
             RETURNING id"
        );
        let row: Option<(Uuid,)> = sqlx::query_as(&sql)
            .bind(rule_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }
}

// ── helpers ──────────────────────────────────────────────────

fn validate_draft(d: &AlertRuleDraft) -> Result<(), AlertRuleError> {
    let trimmed = d.name.trim();
    if trimmed.is_empty() {
        return Err(AlertRuleError::InvalidInput(
            "name must not be empty".into(),
        ));
    }
    if trimmed.len() > MAX_NAME_LEN {
        return Err(AlertRuleError::InvalidInput(format!(
            "name too long: {} > {MAX_NAME_LEN}",
            trimmed.len()
        )));
    }
    if d.throttle_minutes < 0 {
        return Err(AlertRuleError::InvalidInput(
            "throttle_minutes must be >= 0".into(),
        ));
    }
    if !d.channels.is_array() {
        return Err(AlertRuleError::InvalidInput(
            "channels must be a JSON array".into(),
        ));
    }
    Ok(())
}

fn build_summary(ctx: &EventContext) -> String {
    if ctx.is_regression {
        format!("regression of {} in {}", ctx.error_type, ctx.release)
    } else {
        format!("new {} in {}", ctx.error_type, ctx.release)
    }
}

fn build_body(ctx: &EventContext) -> String {
    let kind = if ctx.is_regression {
        "regression"
    } else {
        "new_issue"
    };
    format!(
        "Trigger: {kind}\nProject: {project_id}\nIssue:   {issue_id}\nRelease: {release}\nEnv:     {environment}\nType:    {error_type}",
        project_id = ctx.project_id.into_uuid(),
        issue_id = ctx.issue_id,
        release = ctx.release,
        environment = ctx.environment,
        error_type = ctx.error_type,
    )
}

fn translate_fk(
    err: sqlx::Error,
    project_id: Option<ProjectId>,
    created_by: Option<UserId>,
) -> AlertRuleError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        let constraint = db_err.constraint().unwrap_or("");
        if (constraint.contains("user") || constraint.contains("created_by"))
            && let Some(u) = created_by
        {
            return AlertRuleError::UserNotFound(u.into_uuid());
        }
        if let Some(p) = project_id {
            return AlertRuleError::ProjectNotFound(p.into_uuid());
        }
        if let Some(u) = created_by {
            return AlertRuleError::UserNotFound(u.into_uuid());
        }
    }
    AlertRuleError::Db(err)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use sentori_workspace_identity::WorkspaceId;

    fn ctx() -> EventContext {
        EventContext {
            project_id: ProjectId::new(),
            issue_id: Uuid::now_v7(),
            error_type: "TypeError".into(),
            environment: "production".into(),
            release: "app@1.0.0".into(),
            is_regression: false,
        }
    }

    #[test]
    fn validate_draft_rejects_empty_name() {
        let d = AlertRuleDraft::new(WorkspaceId::new(), "  ", TriggerKind::NewIssue);
        assert!(matches!(
            validate_draft(&d),
            Err(AlertRuleError::InvalidInput(_))
        ));
    }

    #[test]
    fn validate_draft_rejects_negative_throttle() {
        let d =
            AlertRuleDraft::new(WorkspaceId::new(), "x", TriggerKind::NewIssue).with_throttle(-1);
        assert!(matches!(
            validate_draft(&d),
            Err(AlertRuleError::InvalidInput(_))
        ));
    }

    #[test]
    fn validate_draft_rejects_non_array_channels() {
        let d = AlertRuleDraft::new(WorkspaceId::new(), "x", TriggerKind::NewIssue)
            .with_channels(serde_json::json!({"oops": true}));
        assert!(matches!(
            validate_draft(&d),
            Err(AlertRuleError::InvalidInput(_))
        ));
    }

    #[test]
    fn validate_draft_accepts_minimal() {
        let d = AlertRuleDraft::new(WorkspaceId::new(), "x", TriggerKind::NewIssue);
        assert!(validate_draft(&d).is_ok());
    }

    #[test]
    fn summary_shape_per_event_kind() {
        let mut c = ctx();
        assert_eq!(build_summary(&c), "new TypeError in app@1.0.0");
        c.is_regression = true;
        assert_eq!(build_summary(&c), "regression of TypeError in app@1.0.0");
    }

    #[test]
    fn body_includes_all_fields() {
        let c = ctx();
        let b = build_body(&c);
        assert!(b.contains("Trigger: new_issue"));
        assert!(b.contains("Release: app@1.0.0"));
        assert!(b.contains("Env:     production"));
        assert!(b.contains("Type:    TypeError"));
    }
}
