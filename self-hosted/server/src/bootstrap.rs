//! Env-declared owner bootstrap.
//!
//! The owner (superadmin) is configuration, not registration
//! (design.md §9-10): every boot reconciles the `users` table
//! against `SENTORI_OWNER_EMAIL` / `SENTORI_OWNER_PASSWORD`.
//!
//! Reconciliation rules:
//! - No superadmin exists → create one from env. If the password
//!   env is absent, generate a random one and print it to the log
//!   once (the Grafana move) — `docker compose up` stays one-shot.
//! - A superadmin exists with a different email → update the email
//!   in place. Declarative: the env var is the source of truth.
//! - The password env NEVER overwrites an existing hash. Otherwise
//!   every restart would silently reset the owner's password to
//!   whatever stale value sits in the compose file.
//!
//! There is no self-signup in this product; admins are created by
//! the owner through the dashboard. This file is the only account
//! creation path that exists outside it.

use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

pub async fn ensure_owner(pool: &PgPool) -> anyhow::Result<()> {
    let email = read_env("SENTORI_OWNER_EMAIL");

    let existing: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, email FROM users WHERE role = 'superadmin' LIMIT 1")
            .fetch_optional(pool)
            .await?;

    match (existing, email) {
        (Some((id, current)), Some(wanted)) => {
            if !current.eq_ignore_ascii_case(&wanted) {
                sqlx::query("UPDATE users SET email = $1 WHERE id = $2")
                    .bind(&wanted)
                    .bind(id)
                    .execute(pool)
                    .await?;
                info!(from = %current, to = %wanted, "owner email reconciled from env");
            }
            Ok(())
        }
        (Some(_), None) => Ok(()),
        (None, None) => {
            warn!(
                "no superadmin exists and SENTORI_OWNER_EMAIL is unset; \
                 the dashboard has no account until it is provided"
            );
            Ok(())
        }
        (None, Some(wanted)) => {
            let password = crate::env_config::env_or_file("SENTORI_OWNER_PASSWORD").unwrap_or_else(|| {
                let generated = random_password();
                // Printed exactly once, at first boot, to the
                // container log — the operator copies it and can
                // change it in the dashboard.
                info!(email = %wanted, password = %generated, "owner created with generated password (change it after first login)");
                generated
            });
            let phc = sentori_argon2_password::PasswordHash::hash(&password)
                .map_err(|e| anyhow::anyhow!("argon2 hash failed: {e}"))?;
            sqlx::query(
                "INSERT INTO users (id, email, password_hash, role, display_name) \
                 VALUES ($1, $2, $3, 'superadmin', 'Owner')",
            )
            .bind(Uuid::now_v7())
            .bind(&wanted)
            .bind(&phc)
            .execute(pool)
            .await?;
            info!(email = %wanted, "owner created from env");
            Ok(())
        }
    }
}

/// 24 random alphanumerics from the OS RNG — long enough that
/// printing it to a private container log is the weakest link.
fn random_password() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789";
    let mut rng = rand::rng();
    (0..24)
        .map(|_| {
            let i = rng.random_range(0..CHARS.len());
            CHARS[i] as char
        })
        .collect()
}

fn read_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `sentori-server reset-password <email>` — set a fresh random
/// password for an existing account and print it to stdout. The
/// SMTP-free recovery path for a locked-out operator.
pub async fn reset_password(pool: &PgPool, email: &str) -> anyhow::Result<()> {
    let user: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await?;
    let Some(id) = user else {
        anyhow::bail!("no account with email {email}");
    };
    let password = random_password();
    let phc = sentori_argon2_password::PasswordHash::hash(&password)
        .map_err(|e| anyhow::anyhow!("argon2 hash failed: {e}"))?;
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&phc)
        .bind(id)
        .execute(pool)
        .await?;
    // Invalidate every live session for the account — a reset that
    // leaves stolen sessions alive isn't a reset.
    sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    println!("password reset for {email}\nnew password: {password}\nchange it after signing in");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_passwords_are_long_and_unambiguous() {
        let p = random_password();
        assert_eq!(p.len(), 24);
        // The alphabet excludes 0/O/1/l/I/o on purpose — an operator
        // reads this out of a terminal log.
        for banned in ['0', 'O', '1', 'l', 'I', 'o'] {
            assert!(!p.contains(banned), "ambiguous char {banned} in {p}");
        }
    }

    #[test]
    fn two_generated_passwords_differ() {
        assert_ne!(random_password(), random_password());
    }
}
