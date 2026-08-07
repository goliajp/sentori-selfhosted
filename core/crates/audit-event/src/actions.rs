//! K-tier-stable action constants.
//!
//! Other K crates emit these via [`crate::AuditService::record`].
//! Consumer crates (saas/server, self-hosted/server) define
//! their own `pub mod actions` for vendor-specific events.
//!
//! Convention: snake_case, dotted-domain prefix. Keep them
//! short — they are stored verbatim and any rename rewrites
//! history. Add new constants append-only.

// ── workspace identity (K1) ─────────────────────────────────

/// A new user joined the workspace (via invite acceptance or
/// first-user bootstrap).
pub const WORKSPACE_MEMBER_ADDED: &str = "workspace.member_added";

/// Member role changed (owner / admin / user transitions).
pub const WORKSPACE_MEMBER_ROLE_CHANGED: &str = "workspace.member_role_changed";

/// Member removed by an admin.
pub const WORKSPACE_MEMBER_REMOVED: &str = "workspace.member_removed";

/// Owner transferred to another user.
pub const WORKSPACE_OWNER_TRANSFERRED: &str = "workspace.owner_transferred";

// ── projects (K1) ───────────────────────────────────────────

/// New project created.
pub const PROJECT_CREATED: &str = "project.created";

/// Project name / settings updated.
pub const PROJECT_UPDATED: &str = "project.updated";

/// Project deleted (cascades through K3-K12 FKs).
pub const PROJECT_DELETED: &str = "project.deleted";

// ── auth-session (K2) ───────────────────────────────────────

/// Session token issued (login).
pub const SESSION_LOGIN: &str = "session.login";

/// Session token revoked (logout / admin force-revoke).
pub const SESSION_LOGOUT: &str = "session.logout";

/// Password reset link consumed → password changed.
pub const PASSWORD_RESET: &str = "password.reset";

/// Email verification link consumed.
pub const EMAIL_VERIFIED: &str = "email.verified";

// ── issues (K4 + K5) ────────────────────────────────────────

/// Operator marked issue resolved.
pub const ISSUE_RESOLVED: &str = "issue.resolved";

/// Operator (or velocity cron) flipped issue back to
/// regressed.
pub const ISSUE_REGRESSED: &str = "issue.regressed";

/// Operator merged two duplicate issues.
pub const ISSUE_MERGED: &str = "issue.merged";

/// Operator silenced an issue (status = ignored).
pub const ISSUE_IGNORED: &str = "issue.ignored";

// ── integrations (K12) ──────────────────────────────────────

/// Adapter config stored for `(project, kind)`.
pub const INTEGRATION_CONNECTED: &str = "integration.connected";

/// Adapter row deactivated (config retained).
pub const INTEGRATION_DEACTIVATED: &str = "integration.deactivated";

/// Adapter row removed entirely.
pub const INTEGRATION_DISCONNECTED: &str = "integration.disconnected";

// ── tokens / API keys ───────────────────────────────────────

/// Project API token minted.
pub const TOKEN_CREATED: &str = "token.created";

/// Project API token revoked.
pub const TOKEN_REVOKED: &str = "token.revoked";

// ── compliance / identity ───────────────────────────────────

/// GDPR-aligned subject erasure executed (PII cleared on
/// matching events + identity fingerprints dropped).
pub const IDENTITY_ERASED: &str = "identity.erased";

/// Dry-run of identity erasure (no mutation).
pub const IDENTITY_ERASE_DRY_RUN: &str = "identity.erase.dry_run";

/// Two identity fingerprints merged into one.
pub const IDENTITY_MERGED: &str = "identity.merged";

/// Identity merge undone (within the 7-day undo window).
pub const IDENTITY_MERGE_UNDONE: &str = "identity.merge_undone";
