//! SDK ingest token store + Bearer middleware.
//!
//! ## Wire format
//!
//! Tokens have the form `st_pk_<26 base32 chars>` (132 bits entropy).
//! `st_pk_` prefix covers both `public` (SDK ingest) and `admin`
//! (server-side admin) token kinds — kind is enforced server-side
//! after lookup, not by prefix. This matches the legacy SDK
//! contract for backwards-compatibility (≥5 year commitment).
//!
//! ## Storage
//!
//! Tokens are stored as `sha256(token_bytes)` in the `tokens`
//! table (see migration `0016_tokens.sql`). A leaked DB row
//! cannot be replayed as a token — only the original plaintext
//! works.
//!
//! ## Auth flow
//!
//! 1. SDK sends `Authorization: Bearer st_pk_...`
//! 2. Middleware extracts the bearer string
//! 3. SHA-256 hash + DB lookup in `tokens.token_hash`
//! 4. If found and not revoked: inject `(ProjectId, WorkspaceId)`
//!    into request extensions
//! 5. If missing / revoked / wrong prefix: 401 with `hint` field

mod error;
mod middleware;
mod model;
mod parse;
mod store;

pub use error::TokenError;
pub use middleware::{IngestContext, bearer_middleware};
pub use model::{Token, TokenKind};
pub use parse::{TOKEN_PREFIX, TOKEN_VALUE_LEN, hash_token};
pub use store::TokenStore;
