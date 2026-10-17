//! Operator-only compatibility for clients sharing a credential before a project split.
//! No request can select its own destination; every delegation names an existing token.

use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[serde(default)]
    routes: Vec<Route>,
    #[serde(default)]
    trace_filters: Vec<TraceFilter>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Route {
    token: Uuid,
    source: Uuid,
    target: Uuid,
    identifiers: Vec<String>,
    namespaces: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TraceFilter {
    project: Uuid,
    names: Vec<String>,
}

impl Policy {
    pub async fn load(pool: &sqlx::PgPool) -> anyhow::Result<Self> {
        let raw = match std::env::var("SENTORI_INGEST_POLICY") {
            Ok(raw) if !raw.trim().is_empty() => Some(raw),
            _ => std::env::var("SENTORI_INGEST_POLICY_FILE")
                .ok()
                .map(std::fs::read_to_string)
                .transpose()?,
        };
        let policy: Self = raw
            .map(|raw| serde_json::from_str(&raw))
            .transpose()?
            .unwrap_or_default();
        for (index, route) in policy.routes.iter().enumerate() {
            anyhow::ensure!(
                route.source != route.target,
                "ingest route cannot target its source"
            );
            anyhow::ensure!(
                !route.identifiers.is_empty() || !route.namespaces.is_empty(),
                "empty ingest route"
            );
            anyhow::ensure!(
                route
                    .identifiers
                    .iter()
                    .chain(&route.namespaces)
                    .all(|s| !s.trim().is_empty()),
                "empty ingest selector"
            );
            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tokens t JOIN projects p ON p.id=$3 WHERE t.id=$1 AND t.project_id=$2 AND t.revoked_at IS NULL)")
                .bind(route.token).bind(route.source).bind(route.target).fetch_one(pool).await?;
            anyhow::ensure!(
                exists,
                "ingest route requires an active source token and existing target"
            );
            for other in &policy.routes[..index] {
                anyhow::ensure!(
                    other.token != route.token
                        || other.source != route.source
                        || (!route
                            .identifiers
                            .iter()
                            .any(|s| other.identifiers.contains(s))
                            && !route
                                .namespaces
                                .iter()
                                .any(|s| other.namespaces.contains(s))),
                    "ambiguous ingest routes"
                );
            }
        }
        for filter in &policy.trace_filters {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id=$1)")
                    .bind(filter.project)
                    .fetch_one(pool)
                    .await?;
            anyhow::ensure!(exists, "trace filter requires an existing project");
        }
        Ok(policy)
    }

    pub fn project(&self, ctx: &IngestContext, release: &str, payload: &Value) -> Uuid {
        let identity = ["identifier", "bundleIdentifier", "packageName"]
            .iter()
            .filter_map(|key| payload.get("app")?.get(key)?.as_str())
            .map(str::trim)
            .find(|s| !s.is_empty() && s.len() <= 200 && !s.chars().any(char::is_control));
        let namespace = release.rsplit_once('@').map_or("", |(name, _)| name.trim());
        self.routes
            .iter()
            .find(|route| {
                route.token == ctx.token_id
                    && route.source == ctx.project_id
                    && match identity {
                        Some(id) => route.identifiers.iter().any(|v| v == id),
                        None => route.namespaces.iter().any(|v| v == namespace),
                    }
            })
            .map_or(ctx.project_id, |route| route.target)
    }

    pub fn filtered(&self, project: Uuid, kind: crate::pipeline::Kind, name: Option<&str>) -> bool {
        kind == crate::pipeline::Kind::Trace
            && name.is_some_and(|name| {
                self.trace_filters
                    .iter()
                    .any(|f| f.project == project && f.names.iter().any(|n| n == name))
            })
    }

    pub fn permits(&self, ctx: &IngestContext, target: Uuid) -> bool {
        target == ctx.project_id
            || self.routes.iter().any(|r| {
                r.token == ctx.token_id && r.source == ctx.project_id && r.target == target
            })
    }

    /// Serialize attachment reconciliation with uploads, including uploads before an event.
    pub async fn attachment_project(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ctx: &IngestContext,
        event: Uuid,
    ) -> Result<Uuid, sqlx::Error> {
        if !self
            .routes
            .iter()
            .any(|r| r.token == ctx.token_id && r.source == ctx.project_id)
        {
            return Ok(ctx.project_id);
        }
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('sentori-event-attachment:' || $1::uuid::text,0))")
            .bind(event).execute(&mut **tx).await?;
        let existing: Option<Uuid> =
            sqlx::query_scalar("SELECT project_id FROM events WHERE id=$1")
                .bind(event)
                .fetch_optional(&mut **tx)
                .await?;
        Ok(existing
            .filter(|id| self.permits(ctx, *id))
            .unwrap_or(ctx.project_id))
    }

    pub async fn reconcile_attachments(
        &self,
        pool: &sqlx::PgPool,
        ctx: &IngestContext,
        event: Uuid,
        target: Uuid,
    ) -> Result<(), sqlx::Error> {
        if target == ctx.project_id {
            return Ok(());
        }
        let mut tx = pool.begin().await?;
        let project = self.attachment_project(&mut tx, ctx, event).await?;
        sqlx::query(
            "UPDATE event_attachments SET project_id=$3 WHERE event_id=$1 AND project_id=$2",
        )
        .bind(event)
        .bind(ctx.project_id)
        .bind(project)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO cloud_replay_usage(project_id,event_id,created_at) SELECT $3,event_id,created_at FROM cloud_replay_usage WHERE event_id=$1 AND project_id=$2 ON CONFLICT DO NOTHING")
            .bind(event).bind(ctx.project_id).bind(project).execute(&mut *tx).await?;
        sqlx::query(
            "DELETE FROM cloud_replay_usage WHERE event_id=$1 AND project_id=$2 AND project_id<>$3",
        )
        .bind(event)
        .bind(ctx.project_id)
        .bind(project)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::Kind;
    #[test]
    fn delegation_is_explicit_and_metadata_wins() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let token = Uuid::new_v4();
        let ctx = IngestContext {
            token_id: token,
            project_id: source,
            scope: sentori_ingest_token::Scope::Ingest,
        };
        let p = Policy {
            routes: vec![Route {
                token,
                source,
                target,
                identifiers: vec!["home.app".into()],
                namespaces: vec!["consumer".into()],
            }],
            trace_filters: vec![TraceFilter {
                project: target,
                names: vec!["app.launch".into()],
            }],
        };
        assert_eq!(p.project(&ctx, "consumer@1", &Value::Null), target);
        assert_eq!(p.project(&ctx, "consumer-other@1", &Value::Null), source);
        assert_eq!(
            p.project(
                &ctx,
                "consumer@1",
                &serde_json::json!({"app":{"identifier":"other.app"}})
            ),
            source
        );
        assert_eq!(
            p.project(
                &ctx,
                "unknown",
                &serde_json::json!({"app":{"packageName":"home.app"}})
            ),
            target
        );
        assert_eq!(
            p.project(
                &IngestContext {
                    token_id: Uuid::new_v4(),
                    ..ctx
                },
                "consumer@1",
                &Value::Null
            ),
            source
        );
        assert!(!p.permits(&ctx, Uuid::new_v4()));
        assert!(p.filtered(target, Kind::Trace, Some("app.launch")));
        assert!(!p.filtered(target, Kind::Warn, Some("app.launch")));
        assert!(!p.filtered(source, Kind::Trace, Some("app.launch")));
    }
}
