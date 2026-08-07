//! `sentorictl` subcommand impls.

pub mod dump;
pub mod export;
pub mod import;
pub mod restore;
pub mod status;

/// Tables snapshotted by `dump` / scanned by `status`.
/// Order matters for restore (parents before children).
pub const TABLES: &[&str] = &[
    // K1 identity
    "users",
    "privacy_salts",
    "projects",
    "workspace_members",
    "project_user_visibility",
    "workspace_invites",
    "app_user_identities",
    "audit_logs",
    // K2
    "sessions",
    "email_verifications",
    "password_reset_tokens",
    // K4 / K5
    "issues",
    "events",
    "identity_fingerprints",
    "project_dropped",
    // K6
    "spans",
    "trace_session_rollup",
    // K7
    "push_tokens",
    "push_token_quarantine",
    // K8
    "replay_sessions",
    // K9
    "runtime_metrics_raw",
    "runtime_metrics_1m",
    "runtime_metrics_1h",
    "runtime_metrics_1d",
    "runtime_metrics_dropped",
    // K10
    "cert_watch_domains",
    "cert_observations",
    // K11
    "delivery_log",
    // K12
    "integrations",
    "issue_integration_links",
    // K14
    "alert_rules",
    // K15
    "saved_views",
    // K17
    "workspace_billing",
    "usage_counters",
];
