//! [`IntegrationService`] — registry + config CRUD + dispatch
//! + link persistence.

use std::collections::HashMap;
use std::sync::Arc;

use sentori_workspace_identity::{ProjectId, UserId};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::IntegrationError;
use crate::model::{
    DispatchOutcome, IntegrationConfig, IssueContext, IssueIntegrationLink, IssueLifecycleEvent,
    row_to_config, row_to_link,
};
use crate::traits::IntegrationAdapter;

/// Public handle.
#[derive(Clone)]
pub struct IntegrationService {
    pool: PgPool,
    adapters: HashMap<&'static str, Arc<dyn IntegrationAdapter>>,
}

impl std::fmt::Debug for IntegrationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegrationService")
            .field("pool", &self.pool)
            .field("kinds", &self.adapters.keys().copied().collect::<Vec<_>>())
            .finish()
    }
}

impl IntegrationService {
    /// Construct with an empty adapter registry. Caller
    /// `register`s each adapter at boot.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            adapters: HashMap::new(),
        }
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Register an adapter. Replaces any prior adapter for
    /// the same `kind()`.
    pub fn register(&mut self, adapter: Arc<dyn IntegrationAdapter>) {
        self.adapters.insert(adapter.kind(), adapter);
    }

    /// All registered kinds.
    #[must_use]
    pub fn kinds(&self) -> Vec<&'static str> {
        self.adapters.keys().copied().collect()
    }

    /// Look up the adapter for `kind` (None if not
    /// registered).
    #[must_use]
    pub fn adapter(&self, kind: &str) -> Option<Arc<dyn IntegrationAdapter>> {
        self.adapters.get(kind).cloned()
    }

    // ── config CRUD ─────────────────────────────────────────

    /// Persist a config blob produced by the adapter's
    /// `exchange_code` / `accept_manual_config`. Idempotent
    /// on `(project_id, kind)` — re-storing overwrites the
    /// blob + flips `active` to TRUE.
    ///
    /// # Errors
    ///
    /// - [`IntegrationError::NoAdapter`] when kind isn't
    ///   registered.
    /// - [`IntegrationError::ProjectNotFound`] on FK fail.
    /// - [`IntegrationError::Db`].
    pub async fn store_config(
        &self,
        project_id: ProjectId,
        kind: &str,
        config: Value,
        connected_by: Option<UserId>,
    ) -> Result<Uuid, IntegrationError> {
        if !self.adapters.contains_key(kind) {
            return Err(IntegrationError::NoAdapter(kind.to_string()));
        }
        let id = Uuid::now_v7();
        let row: (Uuid,) = sqlx::query_as(
            r"
            INSERT INTO integrations (id, workspace_id, project_id, kind, config, connected_by, active)
            SELECT $1, p.workspace_id, $2, $3, $4, $5, TRUE
            FROM projects p WHERE p.id = $2
            ON CONFLICT (project_id, kind) DO UPDATE SET
                config = EXCLUDED.config,
                connected_by = COALESCE(EXCLUDED.connected_by, integrations.connected_by),
                active = TRUE
            RETURNING id
            ",
        )
        .bind(id)
        .bind(project_id.into_uuid())
        .bind(kind)
        .bind(&config)
        .bind(connected_by.map(UserId::into_uuid))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| translate_fk(e, project_id))?;
        Ok(row.0)
    }

    /// Load the stored config for `(project, kind)`. Returns
    /// None if no row exists.
    ///
    /// # Errors
    ///
    /// [`IntegrationError::Db`] on backend failure.
    pub async fn get_config(
        &self,
        project_id: ProjectId,
        kind: &str,
    ) -> Result<Option<IntegrationConfig>, IntegrationError> {
        let row = sqlx::query(
            r"
            SELECT id, project_id, kind, config, connected_by, connected_at, active
            FROM integrations
            WHERE project_id = $1 AND kind = $2
            ",
        )
        .bind(project_id.into_uuid())
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_config).transpose()
    }

    /// List every configured integration for a project
    /// (active + inactive). Sorted by `connected_at`.
    ///
    /// # Errors
    ///
    /// [`IntegrationError::Db`] on backend failure.
    pub async fn list_for_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<IntegrationConfig>, IntegrationError> {
        let rows = sqlx::query(
            r"
            SELECT id, project_id, kind, config, connected_by, connected_at, active
            FROM integrations
            WHERE project_id = $1
            ORDER BY connected_at ASC
            ",
        )
        .bind(project_id.into_uuid())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_config).collect()
    }

    /// Flip `active` to FALSE without losing the config blob.
    /// Idempotent.
    ///
    /// # Errors
    ///
    /// [`IntegrationError::Db`] on backend failure.
    pub async fn deactivate(
        &self,
        project_id: ProjectId,
        kind: &str,
    ) -> Result<(), IntegrationError> {
        sqlx::query(
            "UPDATE integrations SET active = FALSE \
             WHERE project_id = $1 AND kind = $2",
        )
        .bind(project_id.into_uuid())
        .bind(kind)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Drop the row entirely.
    ///
    /// # Errors
    ///
    /// [`IntegrationError::Db`] on backend failure.
    pub async fn remove_config(
        &self,
        project_id: ProjectId,
        kind: &str,
    ) -> Result<(), IntegrationError> {
        sqlx::query("DELETE FROM integrations WHERE project_id = $1 AND kind = $2")
            .bind(project_id.into_uuid())
            .bind(kind)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── dispatch ────────────────────────────────────────────

    /// Fan out one [`IssueLifecycleEvent`] over every
    /// active adapter configured for `ctx.project_id`.
    ///
    /// Behaviour:
    /// - `Created` — for each active adapter not yet
    ///   linked, call `create_issue` and persist the
    ///   resulting [`crate::ExternalRef`] in
    ///   `issue_integration_links`. Already-linked adapters
    ///   skip with reason `"already linked"`.
    /// - `Regressed` / `Resolved` — for each active adapter
    ///   that *is* linked, look up `external_id` and call
    ///   `update_status`. Adapters with no link skip with
    ///   reason `"not linked"`.
    ///
    /// Per-adapter failures are captured in the outcome and
    /// do NOT abort the loop.
    ///
    /// # Errors
    ///
    /// [`IntegrationError::Db`] on initial project-config
    /// fetch failure. Per-adapter failures are recorded in
    /// the returned outcome.
    pub async fn dispatch(
        &self,
        ctx: &IssueContext,
        event: IssueLifecycleEvent,
    ) -> Result<DispatchOutcome, IntegrationError> {
        let configs = self.active_for_project(ctx.project_id).await?;
        let mut outcome = DispatchOutcome::new();

        for cfg in configs {
            let Some(adapter) = self.adapters.get(cfg.kind.as_str()).cloned() else {
                outcome
                    .skipped
                    .push((cfg.kind.clone(), "no adapter registered".into()));
                continue;
            };
            if !adapter.is_configured() {
                outcome
                    .skipped
                    .push((cfg.kind.clone(), "not configured".into()));
                continue;
            }

            match event {
                IssueLifecycleEvent::Created => {
                    if self.is_linked(ctx.issue_id, &cfg.kind).await? {
                        outcome
                            .skipped
                            .push((cfg.kind.clone(), "already linked".into()));
                        continue;
                    }
                    match adapter.create_issue(&cfg.config, ctx).await {
                        Ok(ext) => {
                            if let Err(e) = self.record_link(ctx.issue_id, &cfg.kind, &ext).await {
                                tracing::warn!(error = %e, kind = %cfg.kind, "link record failed");
                                outcome.failures.push((cfg.kind.clone(), e.to_string()));
                            } else {
                                outcome.successes.push((cfg.kind, ext));
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, kind = %cfg.kind, "create_issue failed");
                            outcome.failures.push((cfg.kind, e.to_string()));
                        }
                    }
                }
                IssueLifecycleEvent::Regressed | IssueLifecycleEvent::Resolved => {
                    let Some(link) = self.get_link(ctx.issue_id, &cfg.kind).await? else {
                        outcome.skipped.push((cfg.kind, "not linked".into()));
                        continue;
                    };
                    match adapter
                        .update_status(&cfg.config, &link.external_id, event)
                        .await
                    {
                        Ok(()) => {
                            outcome.successes.push((
                                cfg.kind,
                                crate::ExternalRef {
                                    external_id: link.external_id,
                                    external_url: link.external_url,
                                },
                            ));
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, kind = %cfg.kind, "update_status failed");
                            outcome.failures.push((cfg.kind, e.to_string()));
                        }
                    }
                }
            }
        }
        Ok(outcome)
    }

    // ── link CRUD ───────────────────────────────────────────

    /// Persist a `(issue, kind)` → upstream `external_*`
    /// link. Idempotent on the pair — re-recording the same
    /// pair is a no-op (returns the existing row's id).
    ///
    /// # Errors
    ///
    /// - [`IntegrationError::IssueNotFound`] on FK fail.
    /// - [`IntegrationError::Db`] on backend failure.
    pub async fn record_link(
        &self,
        issue_id: Uuid,
        kind: &str,
        ext: &crate::ExternalRef,
    ) -> Result<Uuid, IntegrationError> {
        let id = Uuid::now_v7();
        let row: (Uuid,) = sqlx::query_as(
            r"
            INSERT INTO issue_integration_links
                (id, workspace_id, issue_id, kind, external_id, external_url)
            SELECT $1, i.workspace_id, $2, $3, $4, $5
            FROM issues i WHERE i.id = $2
            ON CONFLICT (issue_id, kind) DO UPDATE SET
                external_id = EXCLUDED.external_id,
                external_url = EXCLUDED.external_url
            RETURNING id
            ",
        )
        .bind(id)
        .bind(issue_id)
        .bind(kind)
        .bind(&ext.external_id)
        .bind(&ext.external_url)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| translate_issue_fk(e, issue_id))?;
        Ok(row.0)
    }

    /// One link by (issue, kind).
    ///
    /// # Errors
    ///
    /// [`IntegrationError::Db`] on backend failure.
    pub async fn get_link(
        &self,
        issue_id: Uuid,
        kind: &str,
    ) -> Result<Option<IssueIntegrationLink>, IntegrationError> {
        let row = sqlx::query(
            r"
            SELECT id, issue_id, kind, external_id, external_url, created_at
            FROM issue_integration_links
            WHERE issue_id = $1 AND kind = $2
            ",
        )
        .bind(issue_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_link).transpose()
    }

    /// All links for one issue (across every adapter kind).
    ///
    /// # Errors
    ///
    /// [`IntegrationError::Db`] on backend failure.
    pub async fn list_links_for_issue(
        &self,
        issue_id: Uuid,
    ) -> Result<Vec<IssueIntegrationLink>, IntegrationError> {
        let rows = sqlx::query(
            r"
            SELECT id, issue_id, kind, external_id, external_url, created_at
            FROM issue_integration_links
            WHERE issue_id = $1
            ORDER BY created_at ASC
            ",
        )
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_link).collect()
    }

    /// Reverse lookup: which Sentori issue maps to
    /// (kind, external_id)? Used by upstream webhook ingest
    /// (Linear close → Sentori resolve loop).
    ///
    /// # Errors
    ///
    /// [`IntegrationError::Db`] on backend failure.
    pub async fn find_link_by_external(
        &self,
        kind: &str,
        external_id: &str,
    ) -> Result<Option<IssueIntegrationLink>, IntegrationError> {
        let row = sqlx::query(
            r"
            SELECT id, issue_id, kind, external_id, external_url, created_at
            FROM issue_integration_links
            WHERE kind = $1 AND external_id = $2
            ",
        )
        .bind(kind)
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_link).transpose()
    }

    // ── internals ───────────────────────────────────────────

    async fn active_for_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<IntegrationConfig>, IntegrationError> {
        let rows = sqlx::query(
            r"
            SELECT id, project_id, kind, config, connected_by, connected_at, active
            FROM integrations
            WHERE project_id = $1 AND active = TRUE
            ORDER BY connected_at ASC
            ",
        )
        .bind(project_id.into_uuid())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_config).collect()
    }

    async fn is_linked(&self, issue_id: Uuid, kind: &str) -> Result<bool, IntegrationError> {
        let row: Option<(bool,)> = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM issue_integration_links \
             WHERE issue_id = $1 AND kind = $2)",
        )
        .bind(issue_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(b,)| b).unwrap_or(false))
    }
}

fn translate_fk(err: sqlx::Error, project_id: ProjectId) -> IntegrationError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        return IntegrationError::ProjectNotFound(project_id.into_uuid());
    }
    IntegrationError::Db(err)
}

fn translate_issue_fk(err: sqlx::Error, issue_id: Uuid) -> IntegrationError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        return IntegrationError::IssueNotFound(issue_id);
    }
    IntegrationError::Db(err)
}
