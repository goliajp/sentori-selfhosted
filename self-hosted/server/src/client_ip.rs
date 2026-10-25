//! Canonical source IP established by the trusted-proxy request middleware.
use axum::http::HeaderMap;

/// Background work and tests without a socket peer have no source IP.
#[must_use]
pub fn client_ip(_headers: &HeaderMap) -> Option<String> {
    sentori_request_origin::current()
}
