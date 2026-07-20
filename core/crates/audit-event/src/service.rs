//! [`AuditService`] — append-only writer + query DSL.

use sentori_workspace_identity::{ProjectId, UserId};
use sqlx::{PgPool, postgres::PgArguments, query::Query, query::QueryAs};
use uuid::Uuid;

use crate::error::AuditError;
use crate::model::{AuditEntry, AuditEntryDraft, AuditQuery, row_to_entry};

/// Hard cap on action string length.
const MAX_ACTION_LEN: usize = 200;
/// Hard cap on target_type / target_id string length.
const MAX_TARGET_LEN: usize = 200;

/// Public handle.
#[derive(Clone, Debug)]
pub struct AuditService {
    pool: PgPool,
}

impl AuditService {
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

    // ── record ──────────────────────────────────────────────

    /// Append an audit entry. Generates the id (UUIDv7) +
    /// `created_at` server-side.
    ///
    /// # Errors
    ///
    /// - [`AuditError::InvalidInput`] for empty / oversize
    ///   action or target.
    /// - [`AuditError::ProjectNotFound`] /
    ///   [`AuditError::ActorNotFound`] on FK fail.
    /// - [`AuditError::Db`].
    pub async fn record(&self, draft: AuditEntryDraft) -> Result<Uuid, AuditError> {
        let action = validate_action(&draft.action)?;
        let target_type = validate_optional_target(&draft.target_type, "target_type")?;
        let target_id = validate_optional_target(&draft.target_id, "target_id")?;

        let id = Uuid::now_v7();
        let row: (Uuid,) = sqlx::query_as(
            r"
            INSERT INTO audit_logs
                (id, workspace_id, project_id, actor_user_id, action, target_type, target_id, payload)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            ",
        )
        .bind(id)
        .bind(draft.workspace_id.into_uuid())
        .bind(draft.project_id.map(ProjectId::into_uuid))
        .bind(draft.actor_user_id.map(UserId::into_uuid))
        .bind(action)
        .bind(target_type)
        .bind(target_id)
        .bind(&draft.payload)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| translate_fk(e, draft.project_id, draft.actor_user_id))?;
        Ok(row.0)
    }

    // ── read ────────────────────────────────────────────────

    /// Look up one entry by id.
    ///
    /// # Errors
    ///
    /// [`AuditError::Db`] on backend failure.
    pub async fn find(&self, id: Uuid) -> Result<Option<AuditEntry>, AuditError> {
        let row = sqlx::query(
            r"
            SELECT id, project_id, actor_user_id, action,
                   target_type, target_id, payload, created_at
            FROM audit_logs
            WHERE id = $1
            ",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_entry).transpose()
    }

    /// Recent entries for a project (`limit` rows, ordered
    /// by `created_at` descending). Convenience wrapper over
    /// [`Self::query`].
    ///
    /// # Errors
    ///
    /// [`AuditError::Db`] on backend failure.
    pub async fn find_recent(
        &self,
        project_id: ProjectId,
        limit: u32,
    ) -> Result<Vec<AuditEntry>, AuditError> {
        self.query(
            AuditQuery::default()
                .with_project(project_id)
                .with_limit(limit),
        )
        .await
    }

    /// Filter audit_logs by the [`AuditQuery`] DSL.
    /// Results ordered by `created_at DESC`, capped by
    /// `q.resolved_limit()`.
    ///
    /// # Errors
    ///
    /// [`AuditError::Db`] on backend failure.
    pub async fn query(&self, q: AuditQuery) -> Result<Vec<AuditEntry>, AuditError> {
        let mut sql = String::from(
            "SELECT id, project_id, actor_user_id, action, \
                    target_type, target_id, payload, created_at \
             FROM audit_logs WHERE TRUE",
        );
        bind_where(&mut sql, &q);
        sql.push_str(" ORDER BY created_at DESC LIMIT ");
        sql.push_str(&q.resolved_limit().to_string());
        let query: QueryAs<'_, sqlx::Postgres, AuditRow, PgArguments> = sqlx::query_as(&sql);
        let query = bind_args_as(query, &q);
        let rows = query.fetch_all(&self.pool).await?;
        rows.into_iter().map(AuditRow::into_entry).collect()
    }

    /// Count rows matching the filter (ignores limit).
    ///
    /// # Errors
    ///
    /// [`AuditError::Db`] on backend failure.
    pub async fn count(&self, q: AuditQuery) -> Result<i64, AuditError> {
        let mut sql = String::from("SELECT COUNT(*)::bigint FROM audit_logs WHERE TRUE");
        bind_where(&mut sql, &q);
        let query: Query<'_, sqlx::Postgres, PgArguments> = sqlx::query(&sql);
        let query = bind_args(query, &q);
        let row = query.fetch_one(&self.pool).await?;
        use sqlx::Row as _;
        Ok(row.get::<i64, _>(0))
    }
}

// ── helpers ──────────────────────────────────────────────────

/// Intermediate row shape so `query_as` can do the work +
/// `row_to_entry` reuses the same conversion.
#[derive(sqlx::FromRow)]
struct AuditRow {
    id: Uuid,
    project_id: Option<Uuid>,
    actor_user_id: Option<Uuid>,
    action: String,
    target_type: Option<String>,
    target_id: Option<String>,
    payload: serde_json::Value,
    created_at: time::OffsetDateTime,
}

impl AuditRow {
    fn into_entry(self) -> Result<AuditEntry, AuditError> {
        Ok(AuditEntry {
            id: self.id,
            project_id: self.project_id.map(ProjectId::from_uuid),
            actor_user_id: self.actor_user_id.map(UserId::from_uuid),
            action: self.action,
            target_type: self.target_type,
            target_id: self.target_id,
            payload: self.payload,
            created_at: self.created_at,
        })
    }
}

fn validate_action(action: &str) -> Result<String, AuditError> {
    let trimmed = action.trim();
    if trimmed.is_empty() {
        return Err(AuditError::InvalidInput("action must not be empty".into()));
    }
    if trimmed.len() > MAX_ACTION_LEN {
        return Err(AuditError::InvalidInput(format!(
            "action too long: {} > {MAX_ACTION_LEN}",
            trimmed.len()
        )));
    }
    Ok(trimmed.to_string())
}

fn validate_optional_target(s: &Option<String>, label: &str) -> Result<Option<String>, AuditError> {
    let Some(raw) = s.as_ref() else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > MAX_TARGET_LEN {
        return Err(AuditError::InvalidInput(format!(
            "{label} too long: {} > {MAX_TARGET_LEN}",
            trimmed.len()
        )));
    }
    Ok(Some(trimmed.to_string()))
}

fn bind_where(sql: &mut String, q: &AuditQuery) {
    let mut idx = 1;
    if q.project_id.is_some() {
        sql.push_str(&format!(" AND project_id = ${idx}"));
        idx += 1;
    }
    if q.actor_user_id.is_some() {
        sql.push_str(&format!(" AND actor_user_id = ${idx}"));
        idx += 1;
    }
    if q.action.is_some() {
        sql.push_str(&format!(" AND action = ${idx}"));
        idx += 1;
    }
    if q.target.is_some() {
        sql.push_str(&format!(" AND target_type = ${idx}"));
        idx += 1;
        sql.push_str(&format!(" AND target_id = ${idx}"));
        idx += 1;
    }
    if q.from.is_some() {
        sql.push_str(&format!(" AND created_at >= ${idx}"));
        idx += 1;
    }
    if q.to.is_some() {
        sql.push_str(&format!(" AND created_at < ${idx}"));
        idx += 1;
    }
    let _ = idx;
}

fn bind_args<'q>(
    mut query: Query<'q, sqlx::Postgres, PgArguments>,
    q: &'q AuditQuery,
) -> Query<'q, sqlx::Postgres, PgArguments> {
    if let Some(p) = q.project_id {
        query = query.bind(p.into_uuid());
    }
    if let Some(a) = q.actor_user_id {
        query = query.bind(a.into_uuid());
    }
    if let Some(action) = &q.action {
        query = query.bind(action);
    }
    if let Some((t, id)) = &q.target {
        query = query.bind(t);
        query = query.bind(id);
    }
    if let Some(f) = q.from {
        query = query.bind(f);
    }
    if let Some(t) = q.to {
        query = query.bind(t);
    }
    query
}

fn bind_args_as<'q, O>(
    mut query: QueryAs<'q, sqlx::Postgres, O, PgArguments>,
    q: &'q AuditQuery,
) -> QueryAs<'q, sqlx::Postgres, O, PgArguments>
where
    O: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    if let Some(p) = q.project_id {
        query = query.bind(p.into_uuid());
    }
    if let Some(a) = q.actor_user_id {
        query = query.bind(a.into_uuid());
    }
    if let Some(action) = &q.action {
        query = query.bind(action);
    }
    if let Some((t, id)) = &q.target {
        query = query.bind(t);
        query = query.bind(id);
    }
    if let Some(f) = q.from {
        query = query.bind(f);
    }
    if let Some(t) = q.to {
        query = query.bind(t);
    }
    query
}

fn translate_fk(
    err: sqlx::Error,
    project_id: Option<ProjectId>,
    actor: Option<UserId>,
) -> AuditError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        // FK could be project or actor. Constraint name
        // includes the column the FK references.
        let constraint = db_err.constraint().unwrap_or("");
        if constraint.contains("project")
            && let Some(p) = project_id
        {
            return AuditError::ProjectNotFound(p.into_uuid());
        }
        if let Some(a) = actor {
            return AuditError::ActorNotFound(a.into_uuid());
        }
        if let Some(p) = project_id {
            return AuditError::ProjectNotFound(p.into_uuid());
        }
    }
    AuditError::Db(err)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn validate_action_trims_and_caps() {
        assert_eq!(validate_action("  foo.bar  ").unwrap(), "foo.bar");
        assert!(matches!(
            validate_action(""),
            Err(AuditError::InvalidInput(_))
        ));
        let long = "x".repeat(MAX_ACTION_LEN + 1);
        assert!(matches!(
            validate_action(&long),
            Err(AuditError::InvalidInput(_))
        ));
    }

    #[test]
    fn validate_target_handles_none_and_empty() {
        assert_eq!(validate_optional_target(&None, "x").unwrap(), None);
        assert_eq!(
            validate_optional_target(&Some(String::new()), "x").unwrap(),
            None
        );
        assert_eq!(
            validate_optional_target(&Some("project".to_string()), "x").unwrap(),
            Some("project".to_string())
        );
    }

    #[test]
    fn validate_target_rejects_oversize() {
        let long = "x".repeat(MAX_TARGET_LEN + 1);
        assert!(matches!(
            validate_optional_target(&Some(long), "x"),
            Err(AuditError::InvalidInput(_))
        ));
    }

    #[test]
    fn bind_where_builds_full_clause() {
        let q = AuditQuery::default()
            .with_project(ProjectId::new())
            .with_actor(UserId::new())
            .with_action("x.y")
            .with_target("t", "1")
            .within(
                time::OffsetDateTime::from_unix_timestamp(0).unwrap(),
                time::OffsetDateTime::from_unix_timestamp(1).unwrap(),
            );
        let mut sql = String::new();
        bind_where(&mut sql, &q);
        // Six filters → up to $7 (project, actor, action, target_type, target_id, from, to).
        assert!(sql.contains("$1"));
        assert!(sql.contains("$7"));
        assert!(sql.contains("AND project_id"));
        assert!(sql.contains("AND actor_user_id"));
        assert!(sql.contains("AND action"));
        assert!(sql.contains("AND target_type"));
        assert!(sql.contains("AND target_id"));
        assert!(sql.contains("AND created_at >="));
        assert!(sql.contains("AND created_at <"));
    }

    #[test]
    fn bind_where_empty_when_no_filter() {
        let q = AuditQuery::default();
        let mut sql = String::new();
        bind_where(&mut sql, &q);
        assert!(sql.is_empty());
    }
}
