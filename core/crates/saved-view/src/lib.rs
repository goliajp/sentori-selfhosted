//! # `sentori-saved-view` — operator-saved filter snapshots
//!
//! Steel-tier (钢筋) crate #15. Stores named "view"
//! presets — saved filter state operators rebuild the
//! dashboard UI from with one click.
//!
//! ## Scopes
//!
//! Two shipped (v0.1 simplification of legacy's three):
//! - [`Scope::Personal`] — visible only to the creating
//!   user. `user_id` is NOT NULL.
//! - [`Scope::Workspace`] — visible to every workspace
//!   member. `user_id` is NULL.
//!
//! Legacy had a third "team" scope keyed on a teams table.
//! Sentori v0.1 K1 has no teams table → team scope dropped;
//! adding it later is a code-only change to the enum +
//! migration to add the `team_id` column.
//!
//! ## Targets
//!
//! Five shipped from day one matching the K-tier surfaces:
//! [`Target::Issues`] / [`Target::Events`] /
//! [`Target::Spans`] / [`Target::Replays`] /
//! [`Target::Metrics`]. CHECK constraint keeps the enum
//! tight while payload stays opaque JSONB.
//!
//! ## Project scope vs workspace-wide
//!
//! Each view's `project_id` is a top-level column (not
//! buried in payload). `NULL = workspace-wide` view that
//! shows across every project in the picker; non-NULL = the
//! view filters to a specific project.
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_saved_view::{
//!     SavedViewService, SavedViewDraft, Scope, Target,
//! };
//! use sentori_workspace_identity::{ProjectId, UserId};
//!
//! # async fn demo(pool: sqlx::PgPool, project_id: ProjectId, user_id: UserId)
//! # -> Result<(), Box<dyn std::error::Error>> {
//! let svc = SavedViewService::new(pool);
//! let id = svc.create(
//!     SavedViewDraft::new("Production crashes", Target::Issues, Scope::Personal)
//!         .for_project(project_id)
//!         .owned_by(user_id)
//!         .with_payload(serde_json::json!({
//!             "status": "active",
//!             "environment": "production",
//!         })),
//! ).await?;
//! let visible = svc.list_visible_to(user_id, None, Target::Issues).await?;
//! assert!(visible.iter().any(|v| v.id == id));
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
    clippy::manual_let_else,
    clippy::single_match,
    clippy::too_long_first_doc_paragraph,
    clippy::module_name_repetitions
)]

mod error;
mod model;
mod service;

pub use error::SavedViewError;
pub use model::{
    SavedView, SavedViewDraft, SavedViewPatch, Scope, ScopeParseError, Target, TargetParseError,
};
pub use service::SavedViewService;
