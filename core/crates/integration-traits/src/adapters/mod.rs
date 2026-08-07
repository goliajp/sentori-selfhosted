//! Built-in [`crate::IntegrationAdapter`] impls.
//!
//! K12 ships two:
//! - [`slack::SlackAdapter`] — reference impl,
//!   [`crate::ConnectMode::Manual`] (incoming webhook).
//! - [`mock::MockAdapter`] — deterministic test double.
//!
//! K12.1-K12.4 follow-ups add `linear`, `jira`, `github`,
//! `gitlab` here.

pub mod mock;
pub mod slack;
