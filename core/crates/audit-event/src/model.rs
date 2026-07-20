//! Typed domain models for the audit log.

use sentori_workspace_identity::{ProjectId, UserId, WorkspaceId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

/// One row in `audit_logs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    /// Primary key.
    pub id: Uuid,
    /// Project the action was scoped to. None for
    /// workspace-level actions.
    pub project_id: Option<ProjectId>,
    /// User who performed the action. None for system /
    /// automation actors (cron jobs, boot-time seed).
    pub actor_user_id: Option<UserId>,
    /// Snake-case action identifier (`project.created`,
    /// `member.role_changed`, …).
    pub action: String,
    /// Optional target shape — pairs with `target_id`.
    /// `"project"`, `"user"`, `"team"`, …
    pub target_type: Option<String>,
    /// Optional target id. String to fit any vendor id
    /// shape (uuid stringified, GitHub issue number,
    /// Linear identifier).
    pub target_id: Option<String>,
    /// Action-specific payload (`{name, before, after,
    /// …}`).
    pub payload: Value,
    /// Insertion ts (UTC).
    pub created_at: OffsetDateTime,
}

/// Builder for [`crate::AuditService::record`]. `workspace_id`
/// + `action` are required; all other fields start unset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntryDraft {
    /// Required workspace scope. Audit rows are always
    /// workspace-bound; system events use a reserved system
    /// workspace.
    pub workspace_id: WorkspaceId,
    /// Required snake-case action.
    pub action: String,
    /// Optional project scope.
    pub project_id: Option<ProjectId>,
    /// Optional actor.
    pub actor_user_id: Option<UserId>,
    /// Optional target type.
    pub target_type: Option<String>,
    /// Optional target id.
    pub target_id: Option<String>,
    /// Optional payload (defaults to `Value::Null`).
    pub payload: Value,
}

impl AuditEntryDraft {
    /// New draft with workspace + action set.
    #[must_use]
    pub fn new(workspace_id: WorkspaceId, action: impl Into<String>) -> Self {
        Self {
            workspace_id,
            action: action.into(),
            project_id: None,
            actor_user_id: None,
            target_type: None,
            target_id: None,
            payload: Value::Null,
        }
    }

    /// Attach the project.
    #[must_use]
    pub fn with_project(mut self, project_id: ProjectId) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Attach the actor user.
    #[must_use]
    pub fn with_actor(mut self, actor_user_id: UserId) -> Self {
        self.actor_user_id = Some(actor_user_id);
        self
    }

    /// Attach the target (type + id pair).
    #[must_use]
    pub fn with_target(
        mut self,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
    ) -> Self {
        self.target_type = Some(target_type.into());
        self.target_id = Some(target_id.into());
        self
    }

    /// Attach the payload JSON.
    #[must_use]
    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }
}

/// Filter struct for [`crate::AuditService::query`]. Every
/// field is optional; absent = no constraint.
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    /// Match `project_id` exactly (None = no constraint).
    pub project_id: Option<ProjectId>,
    /// Match `actor_user_id` exactly.
    pub actor_user_id: Option<UserId>,
    /// Match `action` exactly. Wildcard / prefix matching
    /// is deliberately not supported — keep audit queries
    /// indexable.
    pub action: Option<String>,
    /// Match (`target_type`, `target_id`) pair exactly.
    pub target: Option<(String, String)>,
    /// `created_at >= from` when set.
    pub from: Option<OffsetDateTime>,
    /// `created_at < to` when set.
    pub to: Option<OffsetDateTime>,
    /// Page size; clamped to `[1, 1000]`. 0 / None →
    /// default 100.
    pub limit: Option<u32>,
}

impl AuditQuery {
    /// Default limit when caller doesn't specify.
    pub const DEFAULT_LIMIT: u32 = 100;
    /// Hard cap.
    pub const MAX_LIMIT: u32 = 1000;

    /// Resolve the effective limit.
    #[must_use]
    pub fn resolved_limit(&self) -> u32 {
        self.limit
            .filter(|n| *n > 0)
            .map_or(Self::DEFAULT_LIMIT, |n| n.min(Self::MAX_LIMIT))
    }

    /// Builder helpers — chain to filter.
    #[must_use]
    pub fn with_project(mut self, project_id: ProjectId) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Filter by actor.
    #[must_use]
    pub fn with_actor(mut self, actor_user_id: UserId) -> Self {
        self.actor_user_id = Some(actor_user_id);
        self
    }

    /// Filter by exact action string.
    #[must_use]
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    /// Filter by target (type, id).
    #[must_use]
    pub fn with_target(
        mut self,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
    ) -> Self {
        self.target = Some((target_type.into(), target_id.into()));
        self
    }

    /// Filter by time window.
    #[must_use]
    pub fn within(mut self, from: OffsetDateTime, to: OffsetDateTime) -> Self {
        self.from = Some(from);
        self.to = Some(to);
        self
    }

    /// Set page size.
    #[must_use]
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }
}

// ── shared row mapper ──────────────────────────────────────

#[allow(clippy::unnecessary_wraps)]
pub(crate) fn row_to_entry(row: &sqlx::postgres::PgRow) -> Result<AuditEntry, crate::AuditError> {
    use sqlx::Row as _;
    Ok(AuditEntry {
        id: row.get("id"),
        project_id: row
            .get::<Option<Uuid>, _>("project_id")
            .map(ProjectId::from_uuid),
        actor_user_id: row
            .get::<Option<Uuid>, _>("actor_user_id")
            .map(UserId::from_uuid),
        action: row.get("action"),
        target_type: row.get("target_type"),
        target_id: row.get("target_id"),
        payload: row.get("payload"),
        created_at: row.get("created_at"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn draft_builder_chains() {
        let pid = ProjectId::new();
        let actor = UserId::new();
        let ws = WorkspaceId::new();
        let draft = AuditEntryDraft::new(ws, "x")
            .with_project(pid)
            .with_actor(actor)
            .with_target("project", "abc")
            .with_payload(serde_json::json!({"k": 1}));
        assert_eq!(draft.action, "x");
        assert_eq!(draft.project_id, Some(pid));
        assert_eq!(draft.actor_user_id, Some(actor));
        assert_eq!(draft.target_type.as_deref(), Some("project"));
        assert_eq!(draft.target_id.as_deref(), Some("abc"));
        assert_eq!(draft.payload["k"], 1);
    }

    #[test]
    fn query_limit_clamps() {
        assert_eq!(
            AuditQuery::default().resolved_limit(),
            AuditQuery::DEFAULT_LIMIT
        );
        assert_eq!(
            AuditQuery {
                limit: Some(0),
                ..Default::default()
            }
            .resolved_limit(),
            AuditQuery::DEFAULT_LIMIT
        );
        assert_eq!(
            AuditQuery {
                limit: Some(10),
                ..Default::default()
            }
            .resolved_limit(),
            10
        );
        assert_eq!(
            AuditQuery {
                limit: Some(10_000),
                ..Default::default()
            }
            .resolved_limit(),
            AuditQuery::MAX_LIMIT
        );
    }

    #[test]
    fn query_builder_chains() {
        let pid = ProjectId::new();
        let q = AuditQuery::default()
            .with_project(pid)
            .with_action("x.y")
            .with_target("t", "1")
            .with_limit(50);
        assert_eq!(q.project_id, Some(pid));
        assert_eq!(q.action.as_deref(), Some("x.y"));
        assert_eq!(q.target.as_ref().unwrap().0, "t");
        assert_eq!(q.resolved_limit(), 50);
    }
}
