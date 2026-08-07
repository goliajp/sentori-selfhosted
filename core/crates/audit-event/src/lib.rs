//! # `sentori-audit-event` — append-only admin-action audit log
//!
//! Steel-tier (钢筋) crate #13. Owns the read/write surface for
//! the `audit_logs` table (shipped in K1 migration 0001;
//! K13 migration 0012 adds the actor + action + target
//! query indexes operator dashboards need).
//!
//! ## Append-only stance
//!
//! [`AuditService::record`] is the only mutating method. No
//! UPDATE / DELETE — once written, an audit row is permanent
//! from K13's perspective. (Operator-side erasure for
//! compliance is a separate ask, deferred to K13.1.) This
//! matches the legacy stance documented in
//! `server/src/audit.rs`: "audit gaps are acceptable,
//! double-write is not".
//!
//! ## Action strings stay free-form
//!
//! K13 doesn't enumerate every business action — that would
//! couple K13 to every business event (org, project, team,
//! integration, token, billing, …). Instead:
//! - K13 accepts any snake-case `&str` via
//!   [`AuditEntryDraft::action`].
//! - K-tier-stable action constants live in
//!   [`actions`] for the events emitted by other core crates
//!   (workspace identity, projects, integrations).
//! - Consumer (saas/server) adds its own `pub mod actions`
//!   for vendor-specific events.
//!
//! Validation at the boundary is minimal: trim + non-empty +
//! length ≤ 200 (matches legacy expectations).
//!
//! ## Query DSL
//!
//! [`AuditQuery`] is a simple filter struct (project /
//! actor / action / target / time range / limit). Every
//! filter is optional; absent = no constraint. Returned
//! results are ordered by `created_at DESC` and capped by
//! `limit` (default 100, max 1000).
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_audit_event::{AuditService, AuditEntryDraft, actions};
//! use sentori_workspace_identity::{ProjectId, UserId};
//!
//! # async fn demo(pool: sqlx::PgPool, project_id: ProjectId, actor: UserId) -> Result<(), Box<dyn std::error::Error>> {
//! let svc = AuditService::new(pool);
//! let _id = svc.record(
//!     AuditEntryDraft::new(actions::PROJECT_CREATED)
//!         .with_project(project_id)
//!         .with_actor(actor)
//!         .with_target("project", project_id.into_uuid().to_string())
//!         .with_payload(serde_json::json!({"name": "my-app"})),
//! ).await?;
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
    clippy::module_name_repetitions,
    clippy::format_push_string,
    clippy::items_after_statements,
    clippy::string_lit_as_bytes,
    clippy::needless_pass_by_value,
    clippy::ref_option,
    clippy::unnecessary_wraps,
    clippy::redundant_closure_for_method_calls,
    clippy::redundant_closure
)]

pub mod actions;
mod error;
mod model;
mod service;

pub use error::AuditError;
pub use model::{AuditEntry, AuditEntryDraft, AuditQuery};
pub use service::AuditService;
