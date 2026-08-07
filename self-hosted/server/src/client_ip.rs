//! Extract the originating client IP from a request behind Caddy.
//!
//! This lived in triplicate — `notify.rs`, the OAuth callback, and the
//! session store all wrote the same walk over `X-Forwarded-For` and
//! `X-Real-IP`. Three copies of the same header parse invited three
//! subtly different answers, and made adding a fourth — the login
//! limiter — feel like it had to reproduce the pattern once more.
//!
//! `X-Forwarded-For` may be a comma-separated chain when a request
//! passes through more than one proxy. We take the leftmost entry,
//! which is the client the outermost proxy saw. `X-Real-IP` is the
//! fallback for edges that set only that.
//!
//! **The header can be spoofed unless something in front is trusted to
//! rewrite it.** In our topology Caddy fronts every request and sets
//! `X-Forwarded-For` itself, overwriting whatever the client sent, so
//! the value here is the connection Caddy saw. A deployment that puts
//! the server on the open internet without a proxy has to change that.

use axum::http::HeaderMap;

/// Return the caller's IP as a string, or `None` if no header carries
/// one. `None` is normal in tests and unusual in production.
#[must_use]
pub fn client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    /// Static in both fields, so both `from_static` calls are
    /// infallible. Non-static test input would need `from_str` and
    /// this crate lints hard against the fallible-then-unwrap shape.
    fn hm(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                axum::http::HeaderName::from_static(k),
                HeaderValue::from_static(v),
            );
        }
        h
    }

    #[test]
    fn xff_wins_and_takes_the_leftmost_hop() {
        assert_eq!(
            client_ip(&hm(&[("x-forwarded-for", "1.2.3.4, 10.0.0.1")])).as_deref(),
            Some("1.2.3.4"),
        );
    }

    #[test]
    fn real_ip_is_the_fallback() {
        assert_eq!(
            client_ip(&hm(&[("x-real-ip", "5.6.7.8")])).as_deref(),
            Some("5.6.7.8"),
        );
    }

    #[test]
    fn xff_beats_real_ip_when_both_are_present() {
        assert_eq!(
            client_ip(&hm(&[
                ("x-forwarded-for", "1.2.3.4"),
                ("x-real-ip", "9.9.9.9"),
            ]))
            .as_deref(),
            Some("1.2.3.4"),
        );
    }

    #[test]
    fn none_when_no_header_carries_an_ip() {
        assert!(client_ip(&HeaderMap::new()).is_none());
    }

    /// A header may exist but be empty in some proxy configurations.
    /// `Some("")` would key the rate limiter under a shared bucket for
    /// every one of them, which is a self-inflicted denial of service.
    #[test]
    fn empty_header_is_treated_as_missing() {
        assert!(client_ip(&hm(&[("x-forwarded-for", "")])).is_none());
        assert!(client_ip(&hm(&[("x-forwarded-for", "   ")])).is_none());
    }
}
