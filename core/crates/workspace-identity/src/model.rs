//! Typed domain models used by the stores.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

// ── newtypes over Uuid ────────────────────────────────────────

macro_rules! id_newtype {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            Serialize,
            Deserialize,
            sqlx::Type,
        )]
        #[sqlx(transparent)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Mint a new id (`UUIDv7` — monotonic-with-time;
            /// preferable to v4 for indexed PKs).
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Construct from a raw [`Uuid`] (round-trip with
            /// stored values).
            #[must_use]
            pub const fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            /// Borrow the underlying [`Uuid`].
            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            /// Consume into the underlying [`Uuid`].
            #[must_use]
            pub const fn into_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }
    };
}

id_newtype!(WorkspaceId, "Strongly-typed `workspaces.id` newtype.");
id_newtype!(UserId, "Strongly-typed `users.id` newtype.");
id_newtype!(ProjectId, "Strongly-typed `projects.id` newtype.");

// ── Role / InviteRole enums ───────────────────────────────────

/// Workspace-level RBAC role. Three values — see crate docs for
/// the capability matrix.
///
/// Roundtrips to/from the SQL CHECK constraint via [`Role::as_db_str`]
/// and [`Role::from_db_str`]; do not write the strings inline at
/// callsites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Exactly one per workspace. DB-level partial unique index
    /// enforces uniqueness. Owners can transfer this title.
    Owner,
    /// Workspace admin: full mutate rights except promote to
    /// admin / transfer owner.
    Admin,
    /// Project-scoped user. Sees only projects explicitly
    /// granted via `project_user_visibility`. Cannot mutate the
    /// workspace itself.
    User,
}

impl Role {
    /// All three role values, in their canonical UI order.
    pub const ALL: [Self; 3] = [Self::Owner, Self::Admin, Self::User];

    /// Stable wire representation matching the SQL CHECK
    /// constraint (`'owner' | 'admin' | 'user'`).
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::User => "user",
        }
    }

    /// Parse the wire representation returned by `as_db_str`.
    ///
    /// # Errors
    ///
    /// Returns [`RoleParseError`] for any string outside the
    /// canonical three. The DB CHECK constraint makes this
    /// unreachable for well-formed rows — see
    /// [`crate::IdentityError::InvalidRoleInDatabase`].
    pub fn from_db_str(s: &str) -> Result<Self, RoleParseError> {
        match s {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            other => Err(RoleParseError(other.to_string())),
        }
    }

    // ── capability predicates ─────────────────────────────────

    /// Can mutate workspace-level settings (name, deletion,
    /// retention, etc.).
    #[must_use]
    pub const fn can_manage_workspace(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    /// Can create or delete projects.
    #[must_use]
    pub const fn can_create_project(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    /// Can add / remove regular `user` members and grant/revoke
    /// their per-project visibility.
    #[must_use]
    pub const fn can_manage_users(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    /// Can promote a user to admin or demote an admin to user.
    /// Owner only — admins cannot create more admins.
    #[must_use]
    pub const fn can_grant_admin(self) -> bool {
        matches!(self, Self::Owner)
    }

    /// Can transfer the owner role. Owner only.
    #[must_use]
    pub const fn can_transfer_owner(self) -> bool {
        matches!(self, Self::Owner)
    }

    /// True if this role automatically sees every project (no
    /// `project_user_visibility` rows required). Owners and
    /// admins are always elevated; users need explicit grants.
    #[must_use]
    pub const fn auto_sees_all_projects(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Role that may appear in [`WorkspaceInvite::role`]. Owners
/// cannot be invited directly — they emerge only via
/// [`crate::Members::transfer_owner`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteRole {
    /// Invitee becomes admin on accept.
    Admin,
    /// Invitee becomes user on accept. Caller must grant
    /// project visibility separately.
    User,
}

impl InviteRole {
    /// All values in canonical UI order (more privileged first).
    pub const ALL: [Self; 2] = [Self::Admin, Self::User];

    /// SQL wire representation.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
        }
    }

    /// Parse from the SQL wire representation.
    ///
    /// # Errors
    ///
    /// Returns [`InviteRoleParseError`] for any string outside
    /// `'admin' | 'user'`.
    pub fn from_db_str(s: &str) -> Result<Self, InviteRoleParseError> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            other => Err(InviteRoleParseError(other.to_string())),
        }
    }

    /// Promote to the corresponding workspace [`Role`] (admin →
    /// admin, user → user). Used by `accept_invite` to insert
    /// the new `workspace_members` row.
    #[must_use]
    pub const fn to_role(self) -> Role {
        match self {
            Self::Admin => Role::Admin,
            Self::User => Role::User,
        }
    }
}

impl fmt::Display for InviteRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

/// Error returned by [`Role::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unrecognised role string: {0:?}")]
pub struct RoleParseError(pub String);

/// Error returned by [`InviteRole::from_db_str`].
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unrecognised invite-role string: {0:?}")]
pub struct InviteRoleParseError(pub String);

// ── row structs ───────────────────────────────────────────────

/// `users` row (sentori account holder).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// Primary key.
    pub id: UserId,
    /// Account email (case is preserved as entered; uniqueness
    /// is enforced case-insensitively via `LOWER(email)` index).
    pub email: String,
    /// True once the user verified email ownership.
    pub email_verified: bool,
    /// Account creation timestamp.
    pub created_at: OffsetDateTime,
}

/// `workspace_members` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// References [`User::id`].
    pub user_id: UserId,
    /// Workspace-level RBAC role.
    pub role: Role,
    /// Who added this member (NULL for bootstrap owner /
    /// migration-imported rows).
    pub added_by: Option<UserId>,
    /// When this member was added.
    pub added_at: OffsetDateTime,
}

/// `projects` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Primary key.
    pub id: ProjectId,
    /// Human-readable name.
    pub name: String,
    /// URL-safe unique slug.
    pub slug: String,
    /// Project-level `sentori-privacy-salt`-style salt, owned
    /// via FK to `privacy_salts`.
    pub privacy_salt_id: Uuid,
    /// Creation timestamp.
    pub created_at: OffsetDateTime,
}

/// `workspace_invites` row, minus the token (the token only
/// exists on the wire in [`crate::MintedInvite::plaintext_token`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceInvite {
    /// Primary key.
    pub id: Uuid,
    /// Email the invite was sent to (cleartext for display).
    pub email: String,
    /// Role the invitee gets on accept.
    pub role: InviteRole,
    /// Workspace member who created the invite.
    pub invited_by: UserId,
    /// Token expiration. Past-expiration invites cannot be
    /// accepted (collapsed into [`crate::IdentityError::InviteInvalid`]).
    pub expires_at: OffsetDateTime,
    /// When the invite was accepted. `None` for pending.
    pub accepted_at: Option<OffsetDateTime>,
    /// Creation timestamp.
    pub created_at: OffsetDateTime,
}

impl WorkspaceInvite {
    /// True if the invite has not yet been accepted.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.accepted_at.is_none()
    }

    /// True if the invite's `expires_at` is in the past
    /// relative to `now`. Caller passes `now` for determinism.
    #[must_use]
    pub fn is_expired(&self, now: OffsetDateTime) -> bool {
        self.expires_at <= now
    }
}
