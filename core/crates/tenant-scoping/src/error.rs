//! Typed denial reasons + plumbing errors.

use sentori_workspace_identity::Role;
use thiserror::Error;
use uuid::Uuid;

use crate::permission::Permission;

/// Failure modes from [`crate::TenantGuard`].
#[derive(Debug, Error)]
pub enum TenantError {
    /// Actor isn't a workspace member at all — every
    /// permission denies.
    #[error("user {0} is not a workspace member")]
    NotAMember(Uuid),

    /// Actor's role doesn't include the requested
    /// permission (e.g. User asking for EditProject).
    #[error("role {role} cannot perform {permission}")]
    InsufficientRole {
        /// The actor's current role.
        role: Role,
        /// The permission requested.
        permission: Permission,
    },

    /// Actor is a User-role member but no
    /// `project_user_visibility` row grants them access to
    /// this project.
    #[error("user {user} cannot see project {project}")]
    NotVisible {
        /// Actor.
        user: Uuid,
        /// The denied project.
        project: Uuid,
    },

    /// Database error.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

impl TenantError {
    /// True for variants safe to surface verbatim to the
    /// dashboard. (All denial variants are; Db is operator-
    /// only.)
    #[must_use]
    pub const fn is_safe_for_end_user(&self) -> bool {
        matches!(
            self,
            Self::NotAMember(_) | Self::InsufficientRole { .. } | Self::NotVisible { .. }
        )
    }

    /// True for variants that mean "denied" (not infra
    /// failure). Useful for dashboards that render different
    /// "permission denied" vs "something went wrong" copy.
    #[must_use]
    pub const fn is_denial(&self) -> bool {
        matches!(
            self,
            Self::NotAMember(_) | Self::InsufficientRole { .. } | Self::NotVisible { .. }
        )
    }
}
