//! # `sentori-workspace-identity` — workspace + members + projects + visibility + invites
//!
//! Steel-tier (钢筋) crate that owns Sentori's identity SQL schema
//! and exposes typed CRUD over it. One handle, five sub-handles:
//!
//! ```text
//! Identity::new(pool)
//!   ├── .users()       — sentori account holders
//!   ├── .members()     — workspace_members RBAC (owner/admin/user)
//!   ├── .projects()    — projects (+ owning privacy_salt)
//!   ├── .visibility()  — per-project ACL for the 'user' role
//!   └── .invites()     — pending/accepted workspace_invites
//! ```
//!
//! ## Schema invariants (encoded in `core/migrations/0001_workspace_identity.sql`)
//!
//! - **Owner is singular.** A partial unique index on
//!   `workspace_members ((1)) WHERE role = 'owner'` makes it a
//!   DB-level impossibility to have two owners. Owner transfer
//!   runs in a single transaction (downgrade old owner to admin,
//!   promote new owner) — see [`Members::transfer_owner`].
//! - **Owners/admins are NEVER in `project_user_visibility`.**
//!   They see every project automatically. The visibility table
//!   holds rows only for `Role::User`. [`Members::set_role`]
//!   cleans up affected visibility rows on demotion *to* user
//!   (it does NOT auto-grant) and on promotion *from* user (it
//!   wipes the now-redundant rows).
//! - **Email is case-insensitively unique** via the
//!   `LOWER(email)` expression index. Login flows must compare
//!   case-insensitively too — see [`Users::find_by_email`].
//! - **Invite tokens are stored as SHA-256 hashes.** A leaked
//!   `workspace_invites` row cannot be replayed at the accept
//!   endpoint. The plaintext token leaves the server exactly
//!   once (in the [`MintedInvite`] return value of
//!   [`Invites::create`]) and the caller must email it to the
//!   recipient and discard.
//!
//! ## Role capability matrix (encoded in [`Role`])
//!
//! | Capability               | Owner | Admin | User    |
//! | ------------------------ | ----- | ----- | ------- |
//! | Manage workspace settings| ✓     | ✓     | ✗       |
//! | Create/delete project    | ✓     | ✓     | ✗       |
//! | Add/remove `user` member | ✓     | ✓     | ✗       |
//! | Promote to admin         | ✓     | ✗     | ✗       |
//! | Transfer owner           | ✓     | ✗     | ✗       |
//! | Auto-see all projects    | ✓     | ✓     | ✗ grant |
//! | Use dashboard / SDK      | ✓     | ✓     | ✓ (acl) |
//!
//! Handler code must call the `Role::can_*` predicates rather
//! than reproducing this table; that is the whole point of
//! moving the matrix into a typed pure function.
//!
//! ## What this crate does NOT do
//!
//! - **No HTTP / axum.** Wiring stores to routes is the K2
//!   `auth-session` crate's job.
//! - **No password verification or hashing.** Use the
//!   `sentori-cookie-session` (S9) `PasswordHash` primitive in
//!   the caller layer.
//! - **No session storage.** Sessions are a K2 concern too.
//! - **No SDK end-user identity** (`app_user_identities`
//!   schema lives here but the salting + write path is in K4
//!   `event-pipeline`; we expose the table so D6 dumps stay
//!   schema-equivalent).
//! - **No saasadmin / tenants table.** Those live in the `saas`
//!   crate, not in core.
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_workspace_identity::{Identity, InviteRole, Role};
//! use sqlx::PgPool;
//! use uuid::Uuid;
//!
//! # async fn demo(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
//! let identity = Identity::new(pool);
//!
//! // First boot: create the sole owner.
//! let owner = identity
//!     .users()
//!     .create("owner@example.com", "$2b$05$bcrypt_hash_here")
//!     .await?;
//! identity.members().add(owner.id, Role::Owner, None).await?;
//!
//! // Owner invites a teammate as admin.
//! let minted = identity
//!     .invites()
//!     .create("admin@example.com", InviteRole::Admin, owner.id, 7)
//!     .await?;
//! // Email `minted.plaintext_token` to the recipient and discard it.
//! # Ok(()) }
//! ```
//!
//! ## Database setup
//!
//! Run `core/migrations/0001_workspace_identity.sql` against the
//! target database before constructing an [`Identity`]. The
//! consuming binary (e.g. `self-hosted/server`) drives this via
//! `sqlx::migrate!` at startup; we do not embed migrations in
//! this crate so that K2 / K3 / … migrations can interleave by
//! global sequence number.

#![cfg_attr(docsrs, feature(doc_cfg))]

mod error;
mod identity;
mod invite_token;
mod model;
mod scoped_pool;
mod store;

pub use error::IdentityError;
pub use identity::{Identity, bootstrap_workspace, ensure_workspace};
pub use invite_token::{INVITE_TOKEN_BYTES, InviteToken, MintedInvite, TokenHash};
pub use model::{
    InviteRole, InviteRoleParseError, Member, Project, ProjectId, Role, RoleParseError, User,
    UserId, WorkspaceId, WorkspaceInvite,
};
pub use scoped_pool::WorkspaceScopedPool;
pub use store::{Invites, Members, Projects, Users, Visibility};
