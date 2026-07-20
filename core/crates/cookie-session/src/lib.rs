//! # `sentori-cookie-session` — session-cookie primitives
//!
//! Stone-tier crate (per cement-stone methodology) for the
//! cookie-side of the v0.1 dashboard's session story. HTTP-
//! agnostic: this stone returns cookie *values* (strings safe to
//! drop into a `Set-Cookie` header) and verifies values back into
//! payloads. Set-Cookie / Cookie header construction, axum
//! middleware, and the cookie-to-session-id binding all live in
//! the 钢筋 `auth-session` crate above.
//!
//! ## What this crate provides
//!
//! Five independently useful primitives:
//!
//! 1. [`SecretKey`] — typed 32-byte symmetric key with
//!    constant-time equality and zero-on-drop. Every signing /
//!    encryption surface in the crate takes `&SecretKey` (never
//!    `&[u8]`), so call-site mistakes are caught at compile time.
//! 2. [`SignedCookie`] — HMAC-SHA256-sealed cookie value. Payload
//!    stays plaintext (visible to the client) but is tamper-
//!    evident. Wire format: `base64url_nopad(payload || tag)`.
//! 3. [`EncryptedCookie`] — AES-256-GCM-sealed cookie value (AEAD).
//!    Both confidential and tamper-evident. Wire format:
//!    `base64url_nopad(nonce || ciphertext || tag)` with a
//!    per-call random 96-bit nonce.
//! 4. [`PasswordHash`] — bcrypt(5) wrapper. Cost-tunable, refuses
//!    bcrypt's silent-truncation footgun (passwords > 72 bytes are
//!    rejected explicitly rather than silently truncated).
//! 5. [`CsrfToken`] — 32-byte CSPRNG-generated token with
//!    constant-time `ct_eq`. Caller binds it to a session out of
//!    band; the stone deliberately stays session-unaware.
//!
//! ## What this crate deliberately does NOT do
//!
//! - **No HTTP coupling.** No axum / hyper / reqwest types appear
//!   anywhere in the public surface. Cookies live as strings;
//!   passwords live as `&str`. Wiring to HTTP headers is for the
//!   钢筋 layer.
//! - **No session storage.** This crate does not know that
//!   sessions exist. A "session" emerges when the 钢筋 layer
//!   combines `SignedCookie` (carrying a session id) with a
//!   session store (Postgres, valkey, ...).
//! - **No key rotation policy.** Both `SignedCookie::open` and
//!   `EncryptedCookie::open` take a single key. Callers
//!   implement rotation by trying the current key first, then
//!   the previous key on `BadSignature` / `Decrypt`.
//! - **No password-policy enforcement** (length / strength /
//!   pwned-list lookup). Callers compose those before calling
//!   `PasswordHash::hash`.
//!
//! ## Wire-format invariants (locked)
//!
//! All cookie values are **base64-url-no-pad** — URL-safe and
//! Set-Cookie-safe with no further escaping. The 32-byte HMAC tag
//! on `SignedCookie` is a *suffix*, not a prefix, so verification
//! does a single fixed-position split. The 12-byte AES-GCM nonce
//! on `EncryptedCookie` is a *prefix*, matching the AEAD
//! convention.
//!
//! ## Quick start
//!
//! ```rust
//! use sentori_cookie_session::{SecretKey, SignedCookie, CsrfToken, PasswordHash};
//!
//! # fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! // Boot-time: load (or generate) the cookie key. In production
//! // you'd load 32 bytes from a secrets manager; we generate here
//! // for the doctest.
//! let key = SecretKey::generate().expect("OS RNG available");
//!
//! // On login: hash the password before storing.
//! let stored = PasswordHash::hash("hunter2")?;
//!
//! // On subsequent login: verify the candidate.
//! assert!(PasswordHash::verify("hunter2", &stored)?);
//!
//! // Mint a session cookie carrying an opaque session id.
//! let cookie = SignedCookie::seal(&key, b"session-id-abc123");
//!
//! // Verify on a later request — payload is the original bytes.
//! let recovered = SignedCookie::open(&key, &cookie)?;
//! assert_eq!(recovered, b"session-id-abc123");
//!
//! // Mint a CSRF token, encode for the wire, parse on echo.
//! let csrf = CsrfToken::generate().expect("OS RNG available");
//! let wire = csrf.encode();
//! let echoed = CsrfToken::parse(&wire)?;
//! assert!(csrf.ct_eq(&echoed));
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Crate docs interleave narrative with tech identifiers ("HMAC",
// "AES-GCM", "URL") the doc_markdown heuristic mis-flags as un-
// codified. Prose readability over satisfying the lint.
#![allow(clippy::doc_markdown)]
#![allow(clippy::multiple_crate_versions)]
// `aes-gcm 0.10`'s public surface still re-exports the old
// `GenericArray`-based `Nonce::from_slice` / `Key::from_slice`
// helpers as deprecated. Their replacement (`generic-array 1.x`)
// is on the upstream's 0.11 milestone, not yet released. Until
// then the deprecation is unavoidable noise — pinning the
// existing API keeps the bytes layout identical and the migration
// trivial when 0.11 ships.
#![allow(deprecated)]

mod csrf;
mod encrypted;
mod error;
mod key;
mod password;
mod signed;

pub use csrf::{CsrfToken, TOKEN_LEN as CSRF_TOKEN_LEN};
pub use encrypted::{EncryptedCookie, NONCE_LEN, TAG_LEN as ENCRYPTED_TAG_LEN};
pub use error::{
    CsrfError, CsrfResult, EncryptedCookieError, EncryptedCookieResult, PasswordError,
    PasswordResult, SignedCookieError, SignedCookieResult,
};
pub use key::{KEY_LEN, SecretKey, WrongLength};
pub use password::PasswordHash;
pub use signed::{SignedCookie, TAG_LEN as SIGNED_TAG_LEN};
