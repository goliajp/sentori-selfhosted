//! Permission enum + pure Role × Permission matrix.

use std::fmt;

use sentori_workspace_identity::Role;
use serde::{Deserialize, Serialize};

/// What a caller wants to do to a project / workspace.
///
/// `Copy` so passing to multiple `can_perform` calls is
/// allocation-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Read project metadata + child resources (issues,
    /// events, replays, …). For User-role members this
    /// requires a `project_user_visibility` row.
    ViewProject,
    /// Write an event into the project (SDK ingest).
    /// Same visibility constraint as `ViewProject` for
    /// User-role.
    WriteEvent,
    /// Patch project name / privacy / settings.
    EditProject,
    /// Delete the project (cascades to events / issues /
    /// integrations / saved views).
    DeleteProject,
    /// Add / remove workspace members; change roles
    /// (Owner ↔ Admin transitions excluded — see
    /// `PromoteToAdmin` + `TransferOwner`).
    ManageMembers,
    /// Promote a User to Admin. Owner-only —
    /// admins can't elevate themselves.
    PromoteToAdmin,
    /// Hand workspace ownership to another user. Owner-only.
    TransferOwner,
    /// Connect / disconnect K12 integrations (Slack /
    /// Linear / …).
    ManageIntegrations,
}

impl Permission {
    /// All variants. Stable order — tests rely on it.
    pub const ALL: [Self; 8] = [
        Self::ViewProject,
        Self::WriteEvent,
        Self::EditProject,
        Self::DeleteProject,
        Self::ManageMembers,
        Self::PromoteToAdmin,
        Self::TransferOwner,
        Self::ManageIntegrations,
    ];

    /// True when the permission is project-scoped (i.e.
    /// per-project visibility applies for User role).
    /// Workspace-level permissions (`ManageMembers`,
    /// `PromoteToAdmin`, `TransferOwner`) ignore project
    /// visibility — the role gate suffices.
    #[must_use]
    pub const fn is_project_scoped(self) -> bool {
        matches!(
            self,
            Self::ViewProject
                | Self::WriteEvent
                | Self::EditProject
                | Self::DeleteProject
                | Self::ManageIntegrations
        )
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::ViewProject => "view_project",
            Self::WriteEvent => "write_event",
            Self::EditProject => "edit_project",
            Self::DeleteProject => "delete_project",
            Self::ManageMembers => "manage_members",
            Self::PromoteToAdmin => "promote_to_admin",
            Self::TransferOwner => "transfer_owner",
            Self::ManageIntegrations => "manage_integrations",
        };
        f.write_str(s)
    }
}

/// Pure Role × Permission matrix. Does NOT consult
/// per-project visibility — caller layers that in for
/// User-role + project-scoped permissions.
///
/// Matrix (see crate-level docs for the full table):
/// - Owner: every permission ✓.
/// - Admin: every permission except `PromoteToAdmin` +
///   `TransferOwner`.
/// - User: only `ViewProject` + `WriteEvent` (visibility-
///   gated by caller).
#[must_use]
pub const fn role_allows(role: Role, permission: Permission) -> bool {
    match role {
        Role::Owner => true,
        Role::Admin => !matches!(
            permission,
            Permission::PromoteToAdmin | Permission::TransferOwner
        ),
        Role::User => matches!(permission, Permission::ViewProject | Permission::WriteEvent),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn owner_can_do_everything() {
        for p in Permission::ALL {
            assert!(role_allows(Role::Owner, p), "owner can {p}");
        }
    }

    #[test]
    fn admin_cannot_promote_or_transfer() {
        for p in Permission::ALL {
            let want = !matches!(p, Permission::PromoteToAdmin | Permission::TransferOwner);
            assert_eq!(role_allows(Role::Admin, p), want, "admin {p}");
        }
    }

    #[test]
    fn user_only_view_and_write() {
        for p in Permission::ALL {
            let want = matches!(p, Permission::ViewProject | Permission::WriteEvent);
            assert_eq!(role_allows(Role::User, p), want, "user {p}");
        }
    }

    #[test]
    fn is_project_scoped_buckets() {
        assert!(Permission::ViewProject.is_project_scoped());
        assert!(Permission::WriteEvent.is_project_scoped());
        assert!(Permission::EditProject.is_project_scoped());
        assert!(Permission::DeleteProject.is_project_scoped());
        assert!(Permission::ManageIntegrations.is_project_scoped());
        assert!(!Permission::ManageMembers.is_project_scoped());
        assert!(!Permission::PromoteToAdmin.is_project_scoped());
        assert!(!Permission::TransferOwner.is_project_scoped());
    }

    #[test]
    fn display_round_trip_via_serde() {
        for p in Permission::ALL {
            let s = serde_json::to_string(&p).unwrap();
            // s is `"snake_case"` JSON string — matches Display.
            assert_eq!(s, format!("\"{p}\""));
            let back: Permission = serde_json::from_str(&s).unwrap();
            assert_eq!(back, p);
        }
    }
}
