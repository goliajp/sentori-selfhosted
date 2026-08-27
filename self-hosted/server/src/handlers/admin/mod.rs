//! Admin / dashboard endpoints (`/admin/api/*` + scoped reads).
//!
//! Phase E sub-module — wires dashboard CRUD for tokens,
//! projects, etc. Unlike `sdk::*` (Bearer st_pk_ gated), these
//! endpoints will be cookie-session gated in Phase E step 2+;
//! for now they're open (single-tenant self-hosted dev).

pub mod projects;
pub mod push_audience;
pub mod push_credentials;
pub mod push_devices;
pub mod push_readiness;
pub mod push_sends;
pub mod releases;
pub mod test_push;
pub mod tokens;
pub mod users;
