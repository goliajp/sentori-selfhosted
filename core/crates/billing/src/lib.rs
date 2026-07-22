//! # `sentori-billing` — plan + per-project quota
//!
//! Steel-tier (钢筋) crate #17. Final 钢筋 in Phase 2.
//!
//! Composes [`Plan`] + [`Limits`] with a workspace-level
//! `workspace_billing` row + per-project `usage_counters`
//! to answer "may this project record one more
//! event/span/replay this month?".
//!
//! ## Scope (v0.1)
//!
//! - **Workspace-level plan** — Sentori v0.1 K1 is
//!   single-workspace; one `workspace_billing` row at
//!   most (DB-enforced singleton).
//! - **Per-project counters** — usage rolls up at the
//!   project granularity for dashboard "events per
//!   project this month" panels.
//! - **3 counter kinds** — [`CounterKind::Events`] /
//!   [`CounterKind::Spans`] / [`CounterKind::Replays`].
//! - **Plan constants in code** — [`Plan::limits`]
//!   returns the [`Limits`] for the plan. Adding a plan
//!   is a code-only change (DB stores the wire tag).
//! - **Atomic UPSERT increments** — no Valkey hot path;
//!   PG handles the contention. v0.1 traffic is fine;
//!   K17.1 may add Valkey if observed.
//! - **Stripe webhook ingest deferred to K17.1** — K17
//!   stores `stripe_customer_id` slot so wiring later is
//!   just adding the handler that calls
//!   [`BillingService::set_plan`].
//!
//! ## Quota check shape
//!
//! [`BillingService::check_and_record`] is the load-bearing
//! call:
//!
//! 1. Read the current plan + month.
//! 2. Atomically UPSERT `usage_counters` (`count = count + n`).
//! 3. Compare new total vs `limits.for_kind(kind)`.
//! 4. Return [`Decision::{Allow, AtLimit, OverLimit}`].
//!
//! Caller drops the event when `OverLimit`, optionally
//! records the drop via
//! [`BillingService::record_drop`] (separate counter
//! `dropped_count` on the same row).
//!
//! ## Quick start
//!
//! ```no_run
//! use sentori_billing::{BillingService, CounterKind, Decision, Plan};
//! use sentori_workspace_identity::ProjectId;
//!
//! # async fn demo(pool: sqlx::PgPool, project: ProjectId) -> Result<(), Box<dyn std::error::Error>> {
//! let svc = BillingService::new(pool);
//! // Boot — make sure billing row exists at Free plan.
//! svc.ensure_default().await?;
//!
//! // Ingest path — per event:
//! let now = time::OffsetDateTime::now_utc();
//! let decision = svc.check_and_record(project, CounterKind::Events, 1, now).await?;
//! match decision {
//!     Decision::Allow { .. } => { /* accept */ }
//!     Decision::AtLimit { .. } => { /* accept, warn UI */ }
//!     Decision::OverLimit { .. } => {
//!         svc.record_drop(project, CounterKind::Events, 1, now).await?;
//!         /* return 429 */
//!     }
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
    clippy::single_match,
    clippy::too_long_first_doc_paragraph,
    clippy::module_name_repetitions
)]

mod error;
mod model;
mod period;
mod service;

pub use error::BillingError;
pub use model::{
    CounterKind, CounterKindParseError, Decision, Limits, Plan, PlanParseError, PlanStatus,
    PlanStatusParseError, UsageRow, WorkspaceBilling, effective_plan,
};
pub use period::{next_period_start, period_key};
pub use service::BillingService;
