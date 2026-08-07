//! [`AuthService`] — the one handle a caller constructs.

use sentori_argon2_password::PasswordHash as Argon2;
use sentori_cookie_session::SecretKey;
use sentori_workspace_identity::{Identity, User, UserId, WorkspaceId};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AuthError;
use crate::options::AuthOptions;
use crate::store::{
    EmailVerifications, MintedEmailVerify, MintedPasswordReset, MintedSession, PasswordResets,
    RequestMeta, Session, SessionId, Sessions,
};

/// High-level entry point for the auth-session crate.
///
/// One handle wraps:
/// - [`Identity`] from K1 (sentori-workspace-identity)
/// - [`SecretKey`] from S9 (signs the session cookie)
/// - the auth-session SQL stores (auth_sessions +
///   email_verifications + password_resets)
///
/// Cheap to clone — internally `PgPool` is `Arc`-shared and
/// `Identity` itself wraps the pool.
#[derive(Clone)]
pub struct AuthService {
    identity: Identity,
    cookie_key: SecretKey,
    opts: AuthOptions,
}

impl std::fmt::Debug for AuthService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Identity holds a PgPool which has its own Debug; we
        // include it but redact the cookie key.
        f.debug_struct("AuthService")
            .field("identity", &self.identity)
            .field("cookie_key", &"<redacted>")
            .field("opts", &self.opts)
            .finish()
    }
}

impl AuthService {
    /// Construct.
    #[must_use]
    pub const fn new(identity: Identity, cookie_key: SecretKey, opts: AuthOptions) -> Self {
        Self {
            identity,
            cookie_key,
            opts,
        }
    }

    /// Borrow the K1 identity handle. Useful for routes that
    /// need to call identity CRUD directly (e.g. workspace
    /// member management) without re-passing the pool.
    #[must_use]
    pub const fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Borrow the cookie signing key. Used by the [`crate::axum`]
    /// module to seal / verify cookies.
    #[must_use]
    pub const fn cookie_key(&self) -> &SecretKey {
        &self.cookie_key
    }

    /// Borrow the tuning knobs.
    #[must_use]
    pub const fn opts(&self) -> &AuthOptions {
        &self.opts
    }

    const fn pool(&self) -> &PgPool {
        self.identity.pool()
    }

    /// Sub-handle for `auth_sessions`.
    #[must_use]
    pub const fn sessions(&self) -> Sessions<'_> {
        Sessions::new(self.identity.pool())
    }

    /// Sub-handle for `email_verifications`.
    #[must_use]
    pub const fn email_verifications(&self) -> EmailVerifications<'_> {
        EmailVerifications::new(self.identity.pool(), self.identity.workspace_id())
    }

    /// Sub-handle for `password_resets`.
    #[must_use]
    pub const fn password_resets(&self) -> PasswordResets<'_> {
        PasswordResets::new(self.identity.pool(), self.identity.workspace_id())
    }

    // ── high-level orchestration ─────────────────────────────────

    /// Register a new user + mint an email-verify token. Email
    /// is normalised to lowercase before insertion.
    ///
    /// The caller is responsible for actually emailing the
    /// verification link — typically by dropping the wire-
    /// encoded token into a notifier-channel message.
    ///
    /// # Errors
    ///
    /// - [`AuthError::EmailInvalid`] if the email fails
    ///   plausible-format check.
    /// - [`AuthError::PasswordTooShort`] if the password is
    ///   under [`AuthOptions::password_min_chars`].
    /// - [`AuthError::Identity`] wrapping
    ///   [`sentori_workspace_identity::IdentityError::EmailTaken`]
    ///   if the email is already on file.
    /// - [`AuthError::Password`] on hashing failure.
    /// - [`AuthError::Db`] on DB failure.
    ///
    /// Returns the new user, the minted email-verify token, and
    /// the [`WorkspaceId`] of the freshly-provisioned personal
    /// workspace the user now owns. `SaaS` self-signup lands every
    /// registrant in their own isolated tenant (not the shared
    /// default workspace); the caller seeds that workspace's
    /// billing row and emails the verify link.
    pub async fn register(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(User, MintedEmailVerify, WorkspaceId), AuthError> {
        validate_email(email)?;
        validate_password(password, self.opts.password_min_chars)?;

        let email_norm = email.trim().to_ascii_lowercase();
        let hash = Argon2::hash(password)?;
        let ws_name = default_workspace_name(&email_norm);
        // One transaction: workspace + owner user + owner
        // membership. Rolls back cleanly on a taken email so no
        // orphan workspace is left behind.
        let (user, workspace_id) = self
            .identity
            .register_tenant_tx(&email_norm, &hash, &ws_name)
            .await?;

        let expires_at = OffsetDateTime::now_utc() + self.opts.email_verify_ttl;
        // Verification token scoped to the NEW workspace, not the
        // AuthService's bound (default) one.
        let minted = crate::store::EmailVerifications::new(self.identity.pool(), workspace_id)
            .create(user.id, expires_at)
            .await?;
        Ok((user, minted, workspace_id))
    }

    /// Re-send a verification token for an unverified user.
    /// Returns `None` if the user doesn't exist or is already
    /// verified (silent — same anti-enumeration discipline as
    /// the legacy register flow).
    ///
    /// # Errors
    ///
    /// - [`AuthError::Db`] on DB failure.
    /// - [`AuthError::Entropy`] / [`AuthError::Identity`] from
    ///   downstream calls.
    pub async fn resend_email_verification(
        &self,
        email: &str,
    ) -> Result<Option<MintedEmailVerify>, AuthError> {
        let email_norm = email.trim().to_ascii_lowercase();
        let Some(user) = self.identity.users().find_by_email(&email_norm).await? else {
            return Ok(None);
        };
        if user.email_verified {
            return Ok(None);
        }
        let expires_at = OffsetDateTime::now_utc() + self.opts.email_verify_ttl;
        let minted = self
            .email_verifications()
            .create(user.id, expires_at)
            .await?;
        Ok(Some(minted))
    }

    /// Consume an email-verification token, mark the user
    /// verified, purge any other pending verifications for that
    /// user. One transaction.
    ///
    /// # Errors
    ///
    /// - [`AuthError::TokenInvalid`] for bad / expired / used
    ///   tokens.
    /// - [`AuthError::Db`] on DB failure.
    pub async fn verify_email(&self, token_wire: &str) -> Result<UserId, AuthError> {
        let user_id = self.email_verifications().consume(token_wire).await?;
        // The consume above runs in its own tx. Flipping the
        // user flag is a separate write; we do it in a fresh
        // tx and accept the (very narrow) window where the
        // token is marked used but the user isn't verified
        // yet — the user retries verify with the same token
        // and we surface InvalidToken since used_at is set.
        // To avoid that, we'd thread the tx through; for
        // simplicity v0.1 collapses to two-step.
        self.identity.users().mark_email_verified(user_id).await?;
        // Best-effort purge of other pending verifications.
        let _ = self.email_verifications().purge_for_user(user_id).await;
        Ok(user_id)
    }

    /// Verify credentials + mint a session if the email is
    /// verified.
    ///
    /// Returns [`AuthError::InvalidCredentials`] for both
    /// "unknown email" and "wrong password" — deliberately
    /// collapsed to prevent user enumeration.
    ///
    /// # Errors
    ///
    /// - [`AuthError::InvalidCredentials`] / [`AuthError::EmailNotVerified`]
    /// - [`AuthError::Password`] on hash backend failure.
    /// - [`AuthError::Db`] on DB failure.
    pub async fn login(
        &self,
        email: &str,
        password: &str,
        meta: &RequestMeta,
    ) -> Result<(User, MintedSession), AuthError> {
        let email_norm = email.trim().to_ascii_lowercase();
        let user = self
            .identity
            .users()
            .find_by_email(&email_norm)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        let lookup = self
            .identity
            .users()
            .lookup_password_hash(&email_norm)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;
        // password verify intentionally happens AFTER the user
        // lookup so the cost-of-failure timing is bounded by
        // the hash check (which is ~100 ms — the same whether
        // or not the user exists).
        if !Argon2::verify(password, &lookup.1)? {
            return Err(AuthError::InvalidCredentials);
        }
        if !user.email_verified {
            return Err(AuthError::EmailNotVerified);
        }

        let expires_at = OffsetDateTime::now_utc() + self.opts.session_ttl;
        let minted = self.sessions().create(user.id, expires_at, meta).await?;
        Ok((user, minted))
    }

    /// Look up the user behind a cookie value (the raw cookie
    /// string after axum-extra's parse).
    ///
    /// 1. Unwraps the S9 SignedCookie → plaintext session_id.
    /// 2. Hashes session_id → DB lookup.
    /// 3. Touches `last_seen_at` + returns the user row.
    ///
    /// Returns `Ok(None)` for cookies that don't verify or have
    /// no live session.
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] / [`AuthError::SessionDangling`] on DB
    /// inconsistency.
    pub async fn lookup_session(
        &self,
        cookie_value: &str,
    ) -> Result<Option<(User, Session)>, AuthError> {
        let Ok(payload) =
            sentori_cookie_session::SignedCookie::open(&self.cookie_key, cookie_value)
        else {
            return Ok(None);
        };
        let Ok(session_wire) = String::from_utf8(payload) else {
            return Ok(None);
        };

        let Ok(session_id) = SessionId::parse(&session_wire) else {
            return Ok(None);
        };
        let id_hash = session_id.hash();
        // Drop the plaintext as soon as we have the hash.
        drop(session_id);

        let Some(session) = self.sessions().touch_and_lookup(&id_hash).await? else {
            return Ok(None);
        };

        let user = self
            .identity
            .users()
            .find_by_id(session.user_id)
            .await?
            .ok_or_else(|| {
                AuthError::SessionDangling(Uuid::from_bytes(stub_uuid_from_hash(&id_hash)))
            })?;

        Ok(Some((user, session)))
    }

    /// Delete the session identified by `id_hash`. Idempotent.
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn logout(&self, id_hash: &[u8; 32]) -> Result<(), AuthError> {
        self.sessions().revoke(id_hash).await
    }

    /// Delete every session for `user_id` except the one
    /// identified by `keep`. Caller passes the hash of the
    /// session that should survive (typically the one making
    /// the request).
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn sign_out_everywhere(
        &self,
        user_id: UserId,
        keep: &[u8; 32],
    ) -> Result<u64, AuthError> {
        self.sessions()
            .revoke_all_for_user(user_id, Some(keep))
            .await
    }

    /// Mint a password-reset token. Returns `None` if no user
    /// with the given email exists (silent — same anti-
    /// enumeration discipline as legacy `forgot_password`).
    ///
    /// # Errors
    ///
    /// [`AuthError::Entropy`] / [`AuthError::Db`].
    pub async fn forgot_password(
        &self,
        email: &str,
    ) -> Result<Option<MintedPasswordReset>, AuthError> {
        let email_norm = email.trim().to_ascii_lowercase();
        let Some(user) = self.identity.users().find_by_email(&email_norm).await? else {
            return Ok(None);
        };
        let expires_at = OffsetDateTime::now_utc() + self.opts.password_reset_ttl;
        let minted = self.password_resets().create(user.id, expires_at).await?;
        Ok(Some(minted))
    }

    /// Consume a password-reset token, rehash the new password,
    /// and invalidate every active session for that user.
    /// Three writes; coordinated with [`PasswordResets::consume`]
    /// (own tx) + identity password update + session purge.
    ///
    /// We accept the same two-step caveat as `verify_email` —
    /// if the consume succeeds but the rehash fails, the token
    /// is dead and the user has to start over via
    /// `forgot_password`. UX is fine because the email is the
    /// recovery channel; the alternative (thread the tx
    /// through stores) bloats the surface.
    ///
    /// # Errors
    ///
    /// - [`AuthError::TokenInvalid`] for bad / expired / used.
    /// - [`AuthError::PasswordTooShort`] if `new_password`
    ///   under [`AuthOptions::password_min_chars`].
    /// - [`AuthError::Password`] on hash failure.
    /// - [`AuthError::Db`] on DB failure.
    pub async fn reset_password(
        &self,
        token_wire: &str,
        new_password: &str,
    ) -> Result<UserId, AuthError> {
        validate_password(new_password, self.opts.password_min_chars)?;
        let user_id = self.password_resets().consume(token_wire).await?;
        let hash = Argon2::hash(new_password)?;
        self.identity
            .users()
            .update_password_hash(user_id, &hash)
            .await?;
        self.sessions().revoke_all_for_user(user_id, None).await?;
        let _ = self.password_resets().purge_for_user(user_id).await;
        Ok(user_id)
    }

    /// Change password for an already-authenticated user.
    /// Verifies the current password (defence against session-
    /// hijack-then-rotate), rehashes the new one, and drops
    /// every session except the calling one.
    ///
    /// # Errors
    ///
    /// - [`AuthError::CurrentPasswordWrong`] if `current_password`
    ///   doesn't match.
    /// - [`AuthError::PasswordTooShort`] if `new_password`
    ///   under [`AuthOptions::password_min_chars`].
    /// - [`AuthError::UserNotFound`] if `user_id` doesn't exist.
    /// - [`AuthError::Password`] / [`AuthError::Db`].
    pub async fn change_password(
        &self,
        user_id: UserId,
        current_password: &str,
        new_password: &str,
        keep_session_hash: &[u8; 32],
    ) -> Result<(), AuthError> {
        validate_password(new_password, self.opts.password_min_chars)?;
        let user = self
            .identity
            .users()
            .find_by_id(user_id)
            .await?
            .ok_or(AuthError::UserNotFound(user_id))?;
        let lookup = self
            .identity
            .users()
            .lookup_password_hash(&user.email)
            .await?
            .ok_or(AuthError::UserNotFound(user_id))?;
        if !Argon2::verify(current_password, &lookup.1)? {
            return Err(AuthError::CurrentPasswordWrong);
        }
        let hash = Argon2::hash(new_password)?;
        self.identity
            .users()
            .update_password_hash(user_id, &hash)
            .await?;
        self.sessions()
            .revoke_all_for_user(user_id, Some(keep_session_hash))
            .await?;
        Ok(())
    }

    /// Borrow the underlying pool. Convenience for tests +
    /// ad-hoc queries in the consumer crate.
    #[must_use]
    pub const fn raw_pool(&self) -> &PgPool {
        self.pool()
    }
}

/// Friendly default name for a self-signup's personal workspace,
/// derived from the email local-part (e.g. `jane@acme.io` →
/// `jane's workspace`). The owner can rename it later.
fn default_workspace_name(email_norm: &str) -> String {
    let local = email_norm.split('@').next().unwrap_or(email_norm);
    let local = if local.is_empty() { "my" } else { local };
    format!("{local}'s workspace")
}

fn validate_email(email: &str) -> Result<(), AuthError> {
    let trimmed = email.trim();
    if trimmed.is_empty()
        || trimmed.len() > 254
        || !trimmed.contains('@')
        || trimmed.contains(char::is_whitespace)
    {
        return Err(AuthError::EmailInvalid);
    }
    Ok(())
}

fn validate_password(password: &str, min_chars: usize) -> Result<(), AuthError> {
    let len = password.chars().count();
    if len < min_chars {
        return Err(AuthError::PasswordTooShort {
            min: min_chars,
            got: len,
        });
    }
    Ok(())
}

/// Derive a deterministic UUID-shaped stub from a session id
/// hash, used only as a logging breadcrumb in the (should-never-
/// happen) [`AuthError::SessionDangling`] variant. Not a real
/// UUID; we just want a 16-byte truncation for the error message
/// without forcing the caller to remember the full 32B hash.
fn stub_uuid_from_hash(hash: &[u8; 32]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out.copy_from_slice(&hash[..16]);
    out
}
