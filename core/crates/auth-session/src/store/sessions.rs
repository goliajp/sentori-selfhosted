//! `auth_sessions` CRUD.

use std::fmt;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sentori_workspace_identity::UserId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use zeroize::Zeroize;

use crate::error::AuthError;

/// Size of the raw session_id in bytes (32 = 256 bits).
pub const SESSION_ID_BYTES: usize = 32;

/// Opaque session identifier. Plaintext base64url string lives
/// only in the client's cookie payload + transiently in handler
/// memory. DB stores SHA-256 of these bytes (in
/// `auth_sessions.id_hash`).
///
/// `Drop` zeroes the buffer. Wire form is 43-char base64url-no-
/// pad. The newtype prevents callers from accidentally passing
/// a hash where they meant the plaintext (or vice versa).
pub struct SessionId {
    bytes: [u8; SESSION_ID_BYTES],
}

impl SessionId {
    /// Mint a fresh random session id.
    ///
    /// # Errors
    ///
    /// [`AuthError::Entropy`] on OS CSPRNG failure.
    pub fn generate() -> Result<Self, AuthError> {
        let mut bytes = [0u8; SESSION_ID_BYTES];
        getrandom::getrandom(&mut bytes).map_err(|e| AuthError::Entropy(e.to_string()))?;
        Ok(Self { bytes })
    }

    /// Encode in 43-char base64url-no-pad wire form. This is
    /// the value that goes inside the SignedCookie payload.
    #[must_use]
    pub fn to_wire_string(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.bytes)
    }

    /// Parse a wire-format string back into a session id. Used
    /// after [`sentori_cookie_session::SignedCookie::open`]
    /// unwraps the cookie.
    ///
    /// # Errors
    ///
    /// [`AuthError::CookieInvalid`] for any string not decoding
    /// to exactly [`SESSION_ID_BYTES`] bytes.
    pub fn parse(s: &str) -> Result<Self, AuthError> {
        let mut decoded = URL_SAFE_NO_PAD
            .decode(s.as_bytes())
            .map_err(|_| AuthError::CookieInvalid)?;
        if decoded.len() != SESSION_ID_BYTES {
            decoded.zeroize();
            return Err(AuthError::CookieInvalid);
        }
        let mut bytes = [0u8; SESSION_ID_BYTES];
        bytes.copy_from_slice(&decoded);
        decoded.zeroize();
        Ok(Self { bytes })
    }

    /// SHA-256 of the bytes — what `auth_sessions.id_hash` stores.
    #[must_use]
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.bytes);
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }
}

impl Drop for SessionId {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionId")
            .field("bytes", &"<redacted>")
            .finish()
    }
}

/// Session row pulled from `auth_sessions`. Does NOT carry the
/// plaintext session id (only the hash, as `id_hash`), so it's
/// safe to log / serialize for "active sessions" UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// SHA-256 of the session id, hex-encoded for serde clarity.
    pub id_hash_hex: String,
    /// Owning user.
    pub user_id: UserId,
    /// When the session was minted (login or refresh).
    pub created_at: OffsetDateTime,
    /// Last time a request touched this session (updated on
    /// every authenticated request).
    pub last_seen_at: OffsetDateTime,
    /// Hard expiry — past this, the session is gone whether or
    /// not it's been used recently.
    pub expires_at: OffsetDateTime,
    /// Originating client IP, if the caller could determine one.
    pub ip: Option<String>,
    /// Originating User-Agent, if any.
    pub user_agent: Option<String>,
}

impl Session {
    /// True if this session is past its `expires_at`.
    #[must_use]
    pub fn is_expired(&self, now: OffsetDateTime) -> bool {
        self.expires_at <= now
    }
}

/// Combined return value of [`Sessions::create`]: the persisted
/// row alongside the plaintext id (returned exactly once; the
/// caller wraps it in a SignedCookie and ships it to the client).
#[derive(Debug)]
pub struct MintedSession {
    /// Persisted row (no plaintext).
    pub session: Session,
    /// Plaintext id — ready to seal in a cookie.
    pub session_id: SessionId,
}

/// Per-request metadata recorded into the session row. Caller
/// extracts from the inbound HTTP request.
#[derive(Debug, Clone, Default)]
pub struct RequestMeta {
    /// Originating client IP (typically `X-Forwarded-For` first
    /// hop). `None` if the caller didn't supply one.
    pub ip: Option<String>,
    /// Originating User-Agent header.
    pub user_agent: Option<String>,
}

/// Store sub-handle for `auth_sessions`.
#[derive(Debug, Clone, Copy)]
pub struct Sessions<'a> {
    pool: &'a PgPool,
}

impl<'a> Sessions<'a> {
    /// Construct over a borrowed pool.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Mint a session for `user_id`, valid until `expires_at`.
    /// Returns both the row (without plaintext) and the freshly
    /// minted `SessionId` (returned exactly once).
    ///
    /// # Errors
    ///
    /// - [`AuthError::Entropy`] on CSPRNG failure.
    /// - [`AuthError::Db`] on DB failure (incl. FK to users).
    pub async fn create(
        &self,
        user_id: UserId,
        expires_at: OffsetDateTime,
        meta: &RequestMeta,
    ) -> Result<MintedSession, AuthError> {
        let session_id = SessionId::generate()?;
        let id_hash = session_id.hash();

        // v0.2 schema requires workspace_id on auth_sessions; derive
        // it from the user row in the same INSERT to avoid a second
        // round-trip.
        let row = sqlx::query(
            "INSERT INTO auth_sessions \
             (id_hash, workspace_id, user_id, expires_at, ip, user_agent) \
             SELECT $1, u.workspace_id, $2, $3, $4, $5 \
             FROM users u WHERE u.id = $2 \
             RETURNING id_hash, user_id, created_at, last_seen_at, expires_at, ip, user_agent",
        )
        .bind(id_hash.as_slice())
        .bind(user_id.into_uuid())
        .bind(expires_at)
        .bind(meta.ip.as_deref())
        .bind(meta.user_agent.as_deref())
        .fetch_one(self.pool)
        .await?;

        let session = row_to_session(&row);
        Ok(MintedSession {
            session,
            session_id,
        })
    }

    /// Look up a session by id hash. Returns `None` if no
    /// matching row OR if the row is past its `expires_at`. We
    /// fold the two conditions so callers don't have to remember
    /// the expiry check.
    ///
    /// On success, updates `last_seen_at` so the "active
    /// sessions" UI shows realistic recency. Done in a single
    /// statement (`UPDATE ... RETURNING`) to avoid a read-then-
    /// write race.
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn touch_and_lookup(&self, id_hash: &[u8; 32]) -> Result<Option<Session>, AuthError> {
        let row = sqlx::query(
            "UPDATE auth_sessions SET last_seen_at = now() \
             WHERE id_hash = $1 AND expires_at > now() \
             RETURNING id_hash, user_id, created_at, last_seen_at, expires_at, ip, user_agent",
        )
        .bind(id_hash.as_slice())
        .fetch_optional(self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_session))
    }

    /// Same as [`Self::touch_and_lookup`] but does NOT bump
    /// `last_seen_at`. Used by read-only handlers (audit log
    /// fetches, etc).
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn lookup(&self, id_hash: &[u8; 32]) -> Result<Option<Session>, AuthError> {
        let row = sqlx::query(
            "SELECT id_hash, user_id, created_at, last_seen_at, expires_at, ip, user_agent \
             FROM auth_sessions \
             WHERE id_hash = $1 AND expires_at > now()",
        )
        .bind(id_hash.as_slice())
        .fetch_optional(self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_session))
    }

    /// List every session belonging to `user_id`, expired or
    /// not. UI uses this for the "active sessions" page;
    /// callers filter `is_expired()` as needed.
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn list_for_user(&self, user_id: UserId) -> Result<Vec<Session>, AuthError> {
        let rows = sqlx::query(
            "SELECT id_hash, user_id, created_at, last_seen_at, expires_at, ip, user_agent \
             FROM auth_sessions WHERE user_id = $1 \
             ORDER BY last_seen_at DESC",
        )
        .bind(user_id.into_uuid())
        .fetch_all(self.pool)
        .await?;

        Ok(rows.iter().map(row_to_session).collect())
    }

    /// Delete a single session by its id hash. Idempotent.
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn revoke(&self, id_hash: &[u8; 32]) -> Result<(), AuthError> {
        sqlx::query("DELETE FROM auth_sessions WHERE id_hash = $1")
            .bind(id_hash.as_slice())
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Delete every session for `user_id` except the one
    /// matching `keep_hash`. Powers `sign_out_everywhere` and
    /// the auto-purge that follows `reset_password` /
    /// `change_password`.
    ///
    /// Pass `None` for `keep_hash` to delete every session
    /// (used by `reset_password`).
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn revoke_all_for_user(
        &self,
        user_id: UserId,
        keep_hash: Option<&[u8; 32]>,
    ) -> Result<u64, AuthError> {
        let result = if let Some(keep) = keep_hash {
            sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1 AND id_hash <> $2")
                .bind(user_id.into_uuid())
                .bind(keep.as_slice())
                .execute(self.pool)
                .await?
        } else {
            sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
                .bind(user_id.into_uuid())
                .execute(self.pool)
                .await?
        };
        Ok(result.rows_affected())
    }

    /// Bulk delete expired rows — for a janitor cron. Returns
    /// the count of deleted rows.
    ///
    /// # Errors
    ///
    /// [`AuthError::Db`] on DB failure.
    pub async fn prune_expired(&self) -> Result<u64, AuthError> {
        let result = sqlx::query("DELETE FROM auth_sessions WHERE expires_at < now()")
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

fn row_to_session(row: &sqlx::postgres::PgRow) -> Session {
    let id_hash: Vec<u8> = row.get("id_hash");
    Session {
        id_hash_hex: bytes_to_hex(&id_hash),
        user_id: UserId::from_uuid(row.get("user_id")),
        created_at: row.get::<OffsetDateTime, _>("created_at"),
        last_seen_at: row.get::<OffsetDateTime, _>("last_seen_at"),
        expires_at: row.get::<OffsetDateTime, _>("expires_at"),
        ip: row.get::<Option<String>, _>("ip"),
        user_agent: row.get::<Option<String>, _>("user_agent"),
    }
}

fn bytes_to_hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        use std::fmt::Write as _;
        let _ = write!(s, "{byte:02x}");
    }
    s
}
