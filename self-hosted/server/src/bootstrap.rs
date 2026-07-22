//! Env-driven first-owner + default-workspace bootstrap.
//!
//! On first boot:
//! 1. Ensure the default `workspaces` row exists (self-hosted is
//!    single-workspace; the row is identified by a constant
//!    `DEFAULT_WORKSPACE_ID` UUID so re-runs are idempotent).
//! 2. Initialise the workspace's billing row at Free plan.
//! 3. If no owner exists yet, read
//!    `SENTORI_BOOTSTRAP_OWNER_EMAIL` + `SENTORI_BOOTSTRAP_OWNER_PASSWORD`
//!    and create the initial Owner.
//!
//! Idempotent — second-and-later boots see the workspace +
//! owner already there and skip.

use sentori_billing::BillingService;
use sentori_workspace_identity::{Identity, Role, WorkspaceId, ensure_workspace};
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

/// The constant workspace_id used by every self-hosted
/// deployment. SaaS deployments mint per-tenant workspace ids;
/// self-hosted always uses this one so dump/restore between
/// self-hosted instances round-trips trivially.
///
/// UUIDv4 "00000000-0000-4000-8000-000000000001" — explicitly
/// versioned so it can never collide with a UUIDv7 mint.
pub const DEFAULT_WORKSPACE_ID: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_000000000001);

/// Return the default workspace id as a typed [`WorkspaceId`].
#[must_use]
pub const fn default_workspace_id() -> WorkspaceId {
    WorkspaceId::from_uuid(DEFAULT_WORKSPACE_ID)
}

/// Run once at boot. Ensures workspace + billing + first owner
/// exist; safe to call on every boot.
pub async fn ensure_first_owner(pool: &PgPool) -> anyhow::Result<()> {
    let workspace_id = default_workspace_id();

    // 1. Ensure the workspaces row exists (idempotent).
    ensure_workspace(pool, workspace_id, "default")
        .await
        .map_err(|e| anyhow::anyhow!("ensure workspaces row: {e}"))?;

    // 2. Ensure billing row exists.
    let billing = BillingService::new(pool.clone(), workspace_id);
    if billing.ensure_default().await? {
        info!("billing row initialised (Free plan) for default workspace");
    }

    // 3. Owner bootstrap.
    let identity = Identity::new(pool.clone(), workspace_id);
    let owner_exists: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM workspace_members \
         WHERE workspace_id = $1 AND role = 'owner' LIMIT 1",
    )
    .bind(workspace_id.into_uuid())
    .fetch_optional(pool)
    .await?;
    if owner_exists.is_some() {
        return Ok(());
    }

    let Some(email) = read_env("SENTORI_BOOTSTRAP_OWNER_EMAIL") else {
        warn!(
            "no SENTORI_BOOTSTRAP_OWNER_EMAIL set; skipping first-owner bootstrap — dashboard /signup must be reachable"
        );
        return Ok(());
    };
    let Some(password) = read_env("SENTORI_BOOTSTRAP_OWNER_PASSWORD") else {
        warn!(
            "SENTORI_BOOTSTRAP_OWNER_EMAIL set but SENTORI_BOOTSTRAP_OWNER_PASSWORD missing; skipping"
        );
        return Ok(());
    };

    let phc = sentori_argon2_password::PasswordHash::hash(&password)
        .map_err(|e| anyhow::anyhow!("argon2 hash failed: {e}"))?;

    let user = identity
        .users()
        .create(&email, &phc)
        .await
        .map_err(|e| anyhow::anyhow!("create owner user: {e}"))?;
    identity
        .members()
        .add(user.id, Role::Owner, None)
        .await
        .map_err(|e| anyhow::anyhow!("add owner member: {e}"))?;
    // The env-bootstrapped owner is trusted (operator who set
    // SENTORI_BOOTSTRAP_OWNER_PASSWORD); skip the verification step
    // that would otherwise require a mailer + click-through.
    let _ = sqlx::query("UPDATE users SET email_verified = TRUE WHERE id = $1")
        .bind(user.id.into_uuid())
        .execute(pool)
        .await;
    info!(%email, "first owner created (env-bootstrapped + auto-verified)");
    Ok(())
}

fn read_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
