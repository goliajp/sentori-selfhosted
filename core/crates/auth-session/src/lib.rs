//! # `sentori-auth-session` — session lifecycle, email verify, password reset
//!
//! Steel-tier (钢筋) crate #2. Composes:
//!
//! - K1 [`sentori_workspace_identity`] for users + members CRUD.
//! - S9 [`sentori_cookie_session`] for the HMAC-signed cookie
//!   wrapping the `session_id`.
//! - S13 [`sentori_argon2_password`] for password hashing.
//!
//! Owns three tables (migration `core/migrations/0002_auth_session.sql`):
//! `auth_sessions`, `email_verifications`, `password_resets`.
//!
//! ## One handle, fan-out sub-handles + high-level methods
//!
//! ```text
//! AuthService::new(identity, cookie_key, opts)
//!   ├── .sessions()              — auth_sessions CRUD
//!   ├── .email_verifications()   — email-verify token CRUD
//!   ├── .password_resets()       — reset token CRUD
//!   │
//!   ├── .register(email, pwd)            → high-level: create user + mint verify token
//!   ├── .verify_email(token_wire)        → high-level: consume verify token, mark verified
//!   ├── .login(email, pwd, meta)         → high-level: verify pwd + mint session
//!   ├── .lookup_session(cookie_value)    → high-level: parse cookie + look up session + user
//!   ├── .logout(session_id)              → high-level: delete a single session row
//!   ├── .sign_out_everywhere(uid, keep)  → high-level: delete every session except keep
//!   ├── .forgot_password(email)          → high-level: mint reset token (or None if no user)
//!   ├── .reset_password(token, new_pwd)  → high-level: verify token + rehash + drop all sessions
//!   └── .change_password(uid, cur, new)  → high-level: verify current pwd + rehash + drop other sessions
//! ```
//!
//! High-level methods are NOT redundant wrappers — they
//! coordinate writes across multiple tables in single
//! transactions where it matters (e.g. `reset_password` ties
//! token consumption + password rotation + session purge into
//! one transaction so a partial-failure can't leave an
//! invalidated token paired with an unrotated password).
//!
//! ## Token shape (matches K1 invite token)
//!
//! Three single-use email-delivered tokens:
//!
//! - [`EmailVerifyToken`] — 24h default expiry
//! - [`PasswordResetToken`] — 2h default expiry
//! - (K1's [`sentori_workspace_identity::InviteToken`] — 7d default)
//!
//! All three use the same on-the-wire form: 32 random bytes
//! base64url-no-pad (43 ASCII chars), DB stores SHA-256
//! only, plaintext leaves the server exactly once at mint
//! time and is Zeroize-on-Drop in process memory.
//!
//! Rule-of-three for a shared stone hasn't fired yet — K2's
//! two tokens use a private generic `SingleUseToken<M>` (in
//! `src/token.rs`) shared at file level inside this crate;
//! K1's `InviteToken` stays as-is. If a fourth single-use
//! token type appears, extract to a stone.
//!
//! ## Session strategy: stateful + `SignedCookie` wrapper
//!
//! - **`session_id`** = 32 random bytes base64url-no-pad.
//! - **Cookie payload** = the bytes of `session_id`, sealed in
//!   an [`sentori_cookie_session::SignedCookie`] (HMAC-SHA256).
//!   A tampered cookie fails verification at the cookie layer
//!   before we ever hit the DB. The plaintext `session_id` is
//!   still readable by the client — that's fine; it's an opaque
//!   token, not a credential.
//! - **DB row** stored at SHA-256(`session_id`). A leaked DB row
//!   cannot be replayed because we'd need to reverse the
//!   SHA-256 to construct the cookie.
//! - **Per-session invalidate** = `sessions().revoke(id)`.
//! - **Sign-out-everywhere** = `sessions().revoke_all_for_user(uid, keep)`.
//!
//! ## axum module
//!
//! The [`axum`] module ships:
//!
//! - [`axum::CurrentUser`] — typed extension inserted by middleware.
//! - [`axum::require_user`] — middleware function that 401s
//!   without a valid session or attaches `CurrentUser` and
//!   forwards.
//! - [`axum::build_session_cookie`] / [`axum::clear_session_cookie`]
//!   — helpers producing the `Set-Cookie` header value.
//!
//! Wiring (in your server crate):
//!
//! ```ignore
//! let auth = AuthService::new(identity, cookie_key, AuthOptions::default());
//! let router = Router::new()
//!     .route("/me", get(me_handler))
//!     .route_layer(middleware::from_fn_with_state(auth.clone(), require_user))
//!     .with_state(auth);
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
// Doc backticks: rustdoc complains about every snake_case
// technical term (session_id, axum, RustCrypto, …). The prose
// reads cleaner without backticking every identifier; we lean
// on `[`...`]` links for the names that actually need linking.
#![allow(clippy::doc_markdown)]

pub mod axum;
mod error;
mod options;
mod service;
mod store;
mod token;

pub use error::AuthError;
pub use options::AuthOptions;
pub use service::AuthService;
pub use store::{
    EmailVerification, EmailVerifications, MintedEmailVerify, MintedPasswordReset, MintedSession,
    PasswordReset, PasswordResets, RequestMeta, SESSION_ID_BYTES, Session, SessionId, Sessions,
};
pub use token::{
    EmailVerifyMarker, EmailVerifyToken, PasswordResetMarker, PasswordResetToken, SingleUseToken,
    SingleUseTokenHash, TOKEN_BYTES,
};
