//! # `sentori-alert-rule` — alert rule storage + atomic firing
//!
//! Steel-tier (钢筋) crate #14. Owns `alert_rules` table
//! shape + filter matching + atomic throttle claim.
//!
//! ## What K14 does + doesn't do
//!
//! K14 owns:
//! - Rule CRUD (`create_rule` / `find` / `list_*` /
//!   `update` / `set_enabled` / `set_muted` / `snooze` /
//!   `delete`).
//! - In-process filter eval for the on-event triggers
//!   (`new_issue`, `regression`).
//! - Atomic throttle claim — UPDATE that only succeeds
//!   when the throttle window has elapsed.
//!
//! K14 does NOT own:
//! - The actual count / crash-free aggregate query that
//!   `event_count` and `crash_free_drop` rules need. That
//!   query lives in the caller (saas/server) and uses K4
//!   events / K9 metrics directly — wiring those into K14
//!   would couple this crate to every business event
//!   pipeline and violate the "不绑死业务流" 钢筋 stance.
//! - Notification dispatch — K11 `sentori_notifier`
//!   territory. K14 hands the caller a [`MatchedRule`] with
//!   the raw channels JSONB; caller shapes it into K11
//!   `Notification`s.
//!
//! ## Trigger kinds
//!
//! See [`TriggerKind`]. Four shipped in v0.1:
//! 1. `NewIssue` — first event of a fingerprint.
//! 2. `Regression` — `resolved → regressed` flip.
//! 3. `EventCount` — ≥N events match filter in window.
//! 4. `CrashFreeDrop` — crash-free session rate dips below
//!    threshold in window.
//!
//! Trigger config + filter config + channels are JSONB so
//! adding a new trigger shape is code-only.
//!
//! ## Throttle: atomic claim
//!
//! [`AlertRuleService::try_claim`] runs:
//!
//! ```sql
//! UPDATE alert_rules
//! SET last_fired_at = now()
//! WHERE id = $1
//!   AND (last_fired_at IS NULL
//!        OR last_fired_at < now() - interval '<N> minutes')
//! RETURNING id
//! ```
//!
//! The WHERE clause + RETURNING in the same statement means
//! two evaluators racing the same rule can't both win the
//! claim — Postgres serialises the UPDATE row lock.
//!
//! ## Use from the on-event path
//!
//! ```no_run
//! use sentori_alert_rule::{AlertRuleService, EventContext};
//! use sentori_workspace_identity::ProjectId;
//! use uuid::Uuid;
//!
//! # async fn demo(pool: sqlx::PgPool, project_id: ProjectId, issue_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
//! let svc = AlertRuleService::new(pool);
//! let ctx = EventContext {
//!     project_id,
//!     issue_id,
//!     error_type: "TypeError".into(),
//!     environment: "production".into(),
//!     release: "app@1.0.0".into(),
//!     is_regression: false,
//! };
//! for matched in svc.try_fire_for_event(&ctx).await? {
//!     // caller dispatches matched.channels via K11 NotifierService
//!     tracing::info!(rule = %matched.rule.id, "alert claimed");
//! }
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
mod filter;
mod model;
mod service;

pub use error::AlertRuleError;
pub use filter::matches_filter;
pub use model::{
    AlertRule, AlertRuleDraft, AlertRulePatch, EventContext, MatchedRule, TriggerKind,
    TriggerKindParseError,
};
pub use service::AlertRuleService;
