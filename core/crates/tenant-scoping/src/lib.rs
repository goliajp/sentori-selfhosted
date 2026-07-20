//! # `sentori-tenant-scoping` — per-project ACL gate
//!
//! Steel-tier (钢筋) crate #16. Composes K1
//! workspace-identity primitives
//! (`workspace_members.role` + `project_user_visibility`)
//! into a single permission gate that every API endpoint
//! calls before mutating.
//!
//! ## No new tables
//!
//! K16 owns no schema. It reads `workspace_members` +
//! `project_user_visibility` (shipped in K1 migration 0001)
//! and answers: "given this user + this project + this
//! action, are they allowed?"
//!
//! ## Role × Permission matrix
//!
//! v0.1 K1 ships 3 roles: [`sentori_workspace_identity::Role`]
//! Owner / Admin / User. K16 defines 8 [`Permission`]s:
//!
//! ```text
//!                          Owner  Admin  User-visible  User-invisible
//! ViewProject              ✓      ✓      ✓             ✗
//! WriteEvent               ✓      ✓      ✓             ✗
//! EditProject              ✓      ✓      ✗             ✗
//! DeleteProject            ✓      ✓      ✗             ✗
//! ManageMembers            ✓      ✓*     ✗             ✗
//! PromoteToAdmin           ✓      ✗      ✗             ✗
//! TransferOwner            ✓      ✗      ✗             ✗
//! ManageIntegrations       ✓      ✓      ✗             ✗
//! ```
//!
//! `*` Admin can manage members but cannot promote-to-admin
//! or transfer-ownership (those stay owner-only — matches
//! the legacy "admin can't elevate themselves" stance).
//!
//! "User-visible" = the user has a row in
//! `project_user_visibility` for this project. "User-
//! invisible" = no such row → reads + writes both
//! deny. Owners + Admins auto-see every project (they
//! never appear in the visibility table — legacy
//! constraint documented in K1's migration).
//!
//! ## Question vs assert API
//!
//! - `can_*` methods return `Result<bool, TenantError>` —
//!   for "should I render the Edit button?" UI questions
//!   that handle both Some(false) and Err on their own.
//! - `assert_can_*` methods return `Result<(), TenantError>`
//!   — for "I'm about to mutate; deny anything not
//!   explicitly allowed". Use these in endpoint handlers.
//!
//! ## Visibility CRUD (self-gated)
//!
//! [`TenantGuard::grant_visibility`] +
//! [`TenantGuard::revoke_visibility`] wrap K1's primitives
//! with a role check on the `actor` — only Owners and
//! Admins can grant. Useful so the dashboard's "add user
//! to project" API doesn't have to repeat the role check.
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_tenant_scoping::{TenantGuard, Permission, TenantError};
//! use sentori_workspace_identity::{ProjectId, UserId};
//!
//! # async fn endpoint_handler(
//! #     pool: sqlx::PgPool,
//! #     actor: UserId,
//! #     project: ProjectId,
//! # ) -> Result<(), Box<dyn std::error::Error>> {
//! let guard = TenantGuard::new(pool);
//! guard.assert_can_perform(actor, project, Permission::EditProject).await?;
//! // mutation goes here
//! # Ok(()) }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(
    clippy::doc_markdown,
    clippy::redundant_pub_crate,
    clippy::missing_panics_doc,
    clippy::missing_const_for_fn,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::expect_used,
    clippy::derive_partial_eq_without_eq,
    clippy::option_if_let_else,
    clippy::single_match,
    clippy::too_long_first_doc_paragraph,
    clippy::module_name_repetitions
)]

mod error;
mod guard;
mod permission;

pub use error::TenantError;
pub use guard::TenantGuard;
pub use permission::{Permission, role_allows};
// Re-export from workspace-identity for convenience — callers can
// `use sentori_tenant_scoping::WorkspaceScopedPool` without an
// extra workspace-identity dep.
pub use sentori_workspace_identity::WorkspaceScopedPool;
