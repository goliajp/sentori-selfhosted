//! Baseline HTTP response security headers.
//!
//! Every response leaves this server without HSTS, without MIME-sniff
//! protection, without a frame policy, and without a referrer policy.
//! Caddy in front adds none of these by default and we did not set
//! them ourselves — the browser fell back to defaults that let
//! downgrade attacks, MIME confusion attacks and clickjacking each
//! matter a little more than they had to.
//!
//! This middleware sets the four cheap ones. They cost nothing to
//! enable and each closes a small door on its own.
//!
//! Deliberately not set here:
//!
//! - **Content-Security-Policy** — a strong CSP is worth doing but
//!   needs to know every script origin the SPA loads (React, GDS,
//!   inline module scripts, Vite dev shims, image hashes). Shipping a
//!   guess breaks the dashboard in one browser and not another, and a
//!   permissive policy is worse than no header at all. Left for a
//!   pass that runs it against the built bundle rather than reasoned
//!   about here.

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;

/// Set headers if the handler did not already. `insert` would blow
/// away a deliberate per-endpoint choice (an embed iframe permission
/// on a public asset, say); `entry(...).or_insert(...)` respects it.
pub async fn add_baseline_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();

    // One year, subdomains included, no preload — preload is a
    // one-way switch and needs to be opted into deliberately at the
    // apex domain, not casually per-service.
    h.entry("strict-transport-security")
        .or_insert(HeaderValue::from_static(
            "max-age=31536000; includeSubDomains",
        ));

    h.entry("x-content-type-options")
        .or_insert(HeaderValue::from_static("nosniff"));

    // Nothing in this product embeds the dashboard as an iframe. Same
    // effect could be had with `Content-Security-Policy: frame-
    // ancestors 'none'`; until we ship a CSP the X-Frame-Options
    // header is what old browsers actually read.
    h.entry("x-frame-options")
        .or_insert(HeaderValue::from_static("DENY"));

    // Send the origin on cross-site requests, the full URL on same-
    // origin. Session tokens live in cookies rather than URLs so full
    // referrer would not leak them here, but a session-scoped page
    // path can still be sensitive.
    h.entry("referrer-policy")
        .or_insert(HeaderValue::from_static("strict-origin-when-cross-origin"));

    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request as HttpRequest, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::{Router, middleware};
    use tower::ServiceExt;

    async fn plain() -> impl IntoResponse {
        (StatusCode::OK, "ok")
    }

    fn app() -> Router {
        Router::new()
            .route("/", get(plain))
            .layer(middleware::from_fn(add_baseline_headers))
    }

    fn get_header(h: &axum::http::HeaderMap, name: &str) -> String {
        h.get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
            .unwrap_or_default()
    }

    async fn hit(router: Router, path: &str) -> Response {
        // `builder()` and `oneshot(...)` both return `Result`. In a
        // test both should always succeed; matching lets us fall into
        // a `panic!` the crate's lints allow only under `#[cfg(test)]`.
        let built = match HttpRequest::builder()
            .method(Method::GET)
            .uri(path)
            .body(Body::empty())
        {
            Ok(r) => r,
            Err(e) => unreachable!("test constructed a malformed request: {e}"),
        };
        match router.oneshot(built).await {
            Ok(r) => r,
            Err(e) => unreachable!("test router call failed: {e}"),
        }
    }

    #[tokio::test]
    async fn every_baseline_header_is_set() {
        let resp = hit(app(), "/").await;
        let h = resp.headers();
        assert!(
            get_header(h, "strict-transport-security").contains("max-age=31536000"),
            "HSTS missing or wrong",
        );
        assert_eq!(get_header(h, "x-content-type-options"), "nosniff");
        assert_eq!(get_header(h, "x-frame-options"), "DENY");
        assert!(get_header(h, "referrer-policy").contains("strict-origin"));
    }

    /// If a handler has a reason to say something different — a public
    /// asset that wants to be iframed, an endpoint that opts out of
    /// HSTS during a rollback — this middleware must respect it. The
    /// header goes on with `entry().or_insert()` for exactly that.
    #[tokio::test]
    async fn per_handler_override_is_respected() {
        async fn framed() -> impl IntoResponse {
            (
                StatusCode::OK,
                [("x-frame-options", "SAMEORIGIN")],
                "iframed",
            )
        }
        let router = Router::new()
            .route("/framed", get(framed))
            .layer(middleware::from_fn(add_baseline_headers));

        let resp = hit(router, "/framed").await;
        assert_eq!(get_header(resp.headers(), "x-frame-options"), "SAMEORIGIN");
    }
}
