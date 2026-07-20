//! Typed domain models for [`crate::SavedViewService`].

use std::fmt;

use sentori_workspace_identity::{ProjectId, UserId, WorkspaceId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Which K-tier surface the saved view filters. Matches the
/// `saved_views.target` CHECK enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    /// K5 issue list view.
    Issues,
    /// K4 event tail view.
    Events,
    /// K6 span / trace view.
    Spans,
    /// K8 replay session list view.
    Replays,
    /// K9 metric series view.
    Metrics,
}

impl Target {
    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Issues => "issues",
            Self::Events => "events",
            Self::Spans => "spans",
            Self::Replays => "replays",
            Self::Metrics => "metrics",
        }
    }

    /// Parse from wire form.
    ///
    /// # Errors
    ///
    /// [`TargetParseError`] for unknown strings.
    pub fn from_db_str(s: &str) -> Result<Self, TargetParseError> {
        match s {
            "issues" => Ok(Self::Issues),
            "events" => Ok(Self::Events),
            "spans" => Ok(Self::Spans),
            "replays" => Ok(Self::Replays),
            "metrics" => Ok(Self::Metrics),
            other => Err(TargetParseError(other.to_string())),
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error from [`Target::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown target: {0:?}")]
pub struct TargetParseError(pub String);

/// Visibility scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Visible only to the owning user. Requires `user_id`
    /// to be set.
    Personal,
    /// Visible to every workspace member. `user_id` must
    /// be NULL.
    Workspace,
}

impl Scope {
    /// SQL wire form.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Workspace => "workspace",
        }
    }

    /// Parse from wire form.
    ///
    /// # Errors
    ///
    /// [`ScopeParseError`] for unknown strings.
    pub fn from_db_str(s: &str) -> Result<Self, ScopeParseError> {
        match s {
            "personal" => Ok(Self::Personal),
            "workspace" => Ok(Self::Workspace),
            other => Err(ScopeParseError(other.to_string())),
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error from [`Scope::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown scope: {0:?}")]
pub struct ScopeParseError(pub String);

/// Full `saved_views` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedView {
    /// Primary key.
    pub id: Uuid,
    /// Owning project (None = workspace-wide).
    pub project_id: Option<ProjectId>,
    /// Target surface.
    pub target: Target,
    /// Visibility scope.
    pub scope: Scope,
    /// Owning user (Some for Personal, None for Workspace).
    pub user_id: Option<UserId>,
    /// Display name.
    pub name: String,
    /// Opaque filter snapshot.
    pub payload: Value,
    /// Creation ts.
    pub created_at: OffsetDateTime,
    /// Creator (None for system-seeded views).
    pub created_by: Option<UserId>,
    /// Last update ts.
    pub updated_at: OffsetDateTime,
}

impl SavedView {
    /// True if `viewer` can see this view (personal owner
    /// match OR workspace scope).
    #[must_use]
    pub fn is_visible_to(&self, viewer: UserId) -> bool {
        match self.scope {
            Scope::Workspace => true,
            Scope::Personal => self.user_id == Some(viewer),
        }
    }
}

/// Builder for [`crate::SavedViewService::create`]. Required:
/// name + target + scope + workspace. For Personal scope,
/// caller MUST chain `owned_by(user)` before `create` — the
/// polarity check kicks in at validate time.
#[derive(Debug, Clone)]
pub struct SavedViewDraft {
    /// Owning workspace (required; view rows are workspace-
    /// bound).
    pub workspace_id: WorkspaceId,
    /// Display name.
    pub name: String,
    /// Target surface.
    pub target: Target,
    /// Visibility scope.
    pub scope: Scope,
    /// Owning project (None = workspace-wide).
    pub project_id: Option<ProjectId>,
    /// Owning user (required when scope=Personal).
    pub user_id: Option<UserId>,
    /// Opaque payload (defaults to `Value::Object` empty).
    pub payload: Value,
    /// Creator (defaults to user_id when scope=Personal).
    pub created_by: Option<UserId>,
}

impl SavedViewDraft {
    /// New draft for the given workspace.
    #[must_use]
    pub fn new(
        workspace_id: WorkspaceId,
        name: impl Into<String>,
        target: Target,
        scope: Scope,
    ) -> Self {
        Self {
            workspace_id,
            name: name.into(),
            target,
            scope,
            project_id: None,
            user_id: None,
            payload: Value::Object(serde_json::Map::new()),
            created_by: None,
        }
    }

    /// Scope to a project.
    #[must_use]
    pub fn for_project(mut self, project_id: ProjectId) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Set the owning user (required for Personal scope).
    /// Also defaults `created_by` to the same user if not
    /// already set.
    #[must_use]
    pub fn owned_by(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        if self.created_by.is_none() {
            self.created_by = Some(user_id);
        }
        self
    }

    /// Set the payload.
    #[must_use]
    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    /// Override created_by (defaults to user_id when
    /// scope=Personal).
    #[must_use]
    pub fn created_by(mut self, user_id: UserId) -> Self {
        self.created_by = Some(user_id);
        self
    }
}

/// Patch shape — both fields optional; None = no-op.
#[derive(Debug, Clone, Default)]
pub struct SavedViewPatch {
    /// Update display name.
    pub name: Option<String>,
    /// Update payload.
    pub payload: Option<Value>,
}

// ── row mapping shared with service.rs ───────────────────────

pub(crate) fn row_to_view(row: &sqlx::postgres::PgRow) -> Result<SavedView, crate::SavedViewError> {
    use sqlx::Row as _;
    let target_str: &str = row.get("target");
    let target = Target::from_db_str(target_str)?;
    let scope_str: &str = row.get("scope");
    let scope = Scope::from_db_str(scope_str)?;
    Ok(SavedView {
        id: row.get("id"),
        project_id: row
            .get::<Option<Uuid>, _>("project_id")
            .map(ProjectId::from_uuid),
        target,
        scope,
        user_id: row.get::<Option<Uuid>, _>("user_id").map(UserId::from_uuid),
        name: row.get("name"),
        payload: row.get("payload"),
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
    fn target_round_trip() {
        for t in [
            Target::Issues,
            Target::Events,
            Target::Spans,
            Target::Replays,
            Target::Metrics,
        ] {
            assert_eq!(Target::from_db_str(t.as_db_str()).unwrap(), t);
        }
    }

    #[test]
    fn target_parse_rejects_unknown() {
        assert!(Target::from_db_str("alerts").is_err());
    }

    #[test]
    fn scope_round_trip() {
        for s in [Scope::Personal, Scope::Workspace] {
            assert_eq!(Scope::from_db_str(s.as_db_str()).unwrap(), s);
        }
    }

    #[test]
    fn scope_parse_rejects_team() {
        // Legacy had 'team' — v0.1 dropped it.
        assert!(Scope::from_db_str("team").is_err());
    }

    #[test]
    fn personal_visible_only_to_owner() {
        let owner = UserId::new();
        let other = UserId::new();
        let v = SavedView {
            id: Uuid::now_v7(),
            project_id: None,
            target: Target::Issues,
            scope: Scope::Personal,
            user_id: Some(owner),
            name: "x".into(),
            payload: Value::Null,
            created_at: OffsetDateTime::UNIX_EPOCH,
            created_by: Some(owner),
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };
        assert!(v.is_visible_to(owner));
        assert!(!v.is_visible_to(other));
    }

    #[test]
    fn workspace_visible_to_anyone() {
        let owner = UserId::new();
        let other = UserId::new();
        let v = SavedView {
            id: Uuid::now_v7(),
            project_id: None,
            target: Target::Issues,
            scope: Scope::Workspace,
            user_id: None,
            name: "x".into(),
            payload: Value::Null,
            created_at: OffsetDateTime::UNIX_EPOCH,
            created_by: Some(owner),
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };
        assert!(v.is_visible_to(owner));
        assert!(v.is_visible_to(other));
    }

    #[test]
    fn draft_owned_by_sets_created_by_default() {
        let user = UserId::new();
        let d = SavedViewDraft::new(WorkspaceId::new(), "x", Target::Issues, Scope::Personal)
            .owned_by(user);
        assert_eq!(d.user_id, Some(user));
        assert_eq!(d.created_by, Some(user));
    }

    #[test]
    fn draft_explicit_created_by_wins() {
        let owner = UserId::new();
        let creator = UserId::new();
        let d = SavedViewDraft::new(WorkspaceId::new(), "x", Target::Issues, Scope::Personal)
            .created_by(creator)
            .owned_by(owner);
        // owned_by skips overwriting created_by when set.
        assert_eq!(d.user_id, Some(owner));
        assert_eq!(d.created_by, Some(creator));
    }
}
