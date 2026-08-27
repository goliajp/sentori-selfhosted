//! HTTP handler aggregation.
//!
//! Two route groups:
//! - **SDK ingest** (`/v1/*`): Bearer st_pk_<token> authenticated
//!   via `sentori-ingest-token`'s `bearer_middleware`. Each handler
//!   receives `Extension<IngestContext>` with the resolved
//!   `(workspace_id, project_id, token_kind)`.
//! - **Dashboard / admin** (`/healthz`, `/v1/projects/...`,
//!   `/v1/usage`, ...): unauthenticated for v0.2 step 2; Phase E
//!   will gate with cookie session.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware as axum_middleware;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use sentori_ingest_token::{TokenStore, bearer_middleware};
use serde_json::json;

use crate::session_mw::session_middleware;
use crate::state::AppState;
use tower_http::catch_panic::CatchPanicLayer;

mod admin;
mod api;
pub mod artifacts_upload;
mod attachments;
mod audit;
mod auth;
mod events;
mod health;
mod instruments;
mod issues;
mod metrics_prom;
mod notify_admin;
mod projects;
mod sdk;

/// Refuse an IP that is hammering a credentialed auth endpoint.
///
/// A password login has no bearer token to key on, so the bucket is
/// keyed on the client's address instead. `X-Forwarded-For` is
/// trusted here because Caddy rewrites it — a deployment that puts
/// the server on the open internet has to change that.
///
/// Absent an IP the request is admitted rather than refused. A limiter
/// silently DOS-ing traffic from callers whose IP header did not
/// arrive is worse than the brute-force window it exists to close.
async fn auth_rate_limit_mw(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: axum_middleware::Next,
) -> axum::response::Response {
    let admitted = crate::client_ip::client_ip(req.headers()).is_none_or(|ip| {
        state
            .auth_rate_limit
            .admit(crate::rate_limit::ip_to_key(&ip))
    });

    if admitted {
        return next.run(req).await;
    }
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({
            "error": "rate_limited",
            "hint": "too many attempts from this address; wait and try again",
        })),
    )
        .into_response()
}

/// Refuse a token that is flooding the ingest surface.
///
/// A public token is compiled into the customer's app, so the rate a
/// single credential can drive is not bounded by anything the customer
/// controls. Without this, someone holding a copy of the app can spend
/// that workspace's monthly quota in minutes and leave the customer's
/// monitoring blind — the outage they bought Sentori to see.
///
/// 429 with `retryAfterMs`, which is what the SDKs already back off
/// on. Deliberately not 402: that means the month's quota is spent and
/// the SDKs drop the batch, which is the wrong answer for a burst that
/// will be fine a second from now.
async fn rate_limit_mw(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: axum_middleware::Next,
) -> axum::response::Response {
    let admitted = req
        .extensions()
        .get::<sentori_ingest_token::IngestContext>()
        .is_none_or(|ctx| state.rate_limit.admit(ctx.token_id));

    if admitted {
        return next.run(req).await;
    }
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({
            "error": "rate_limited",
            "retryAfterMs": 1000,
        })),
    )
        .into_response()
}

// A flat route table: length is inherent to enumerating every route
// in one place, and splitting it would only hide the routing surface.
#[allow(clippy::too_many_lines)]
pub fn router(state: Arc<AppState>) -> Router {
    // ── SDK ingest routes — Bearer st_ token, ingest scope ──
    let token_store = TokenStore::new(state.pool.clone());
    let sdk_routes = Router::new()
        .route("/v1/events", post(sdk::events::handle))
        .route("/v1/events:batch", post(sdk::events_batch::handle))
        .route(
            "/v1/events/{event_id}/attachments/{kind}",
            post(sdk::events_attachments::handle),
        )
        .route("/v1/deploys", post(sdk::deploys::handle))
        .route(
            "/v1/releases/{release}/artifacts",
            // Sourcemaps/dSYMs run tens of MB; axum's 2 MB default
            // body limit truncated the multipart stream and every
            // real-app upload died as "Error parsing multipart".
            // 256 MB on the wire; bigger artifacts arrive gzipped
            // (the handler inflates, capped at 512 MB decompressed).
            get(artifacts_upload::list_by_release_name)
                .post(artifacts_upload::upload_by_release_name)
                .layer(axum::extract::DefaultBodyLimit::max(256 * 1024 * 1024)),
        )
        // ── push ──
        //
        // Plural resource nouns, ids in the path, one custom action.
        // The two id levels have two words: `sendId` is one call to
        // `POST /v1/push/sends`, `deliveryId` is one device's row
        // inside it. They used to share the word `send`, and passing
        // one where the other belonged was a 404 with no hint.
        //
        // `devices`, not `tokens`: four things in this system are
        // called a token — the ingest one, the api one, the vendor's,
        // and the spToken — and the thing being registered is a
        // device.
        .route("/v1/push/devices", post(sdk::push::register_token::handle))
        .route(
            "/v1/push/devices/{sp_token}",
            delete(sdk::push::revoke_token::handle),
        )
        .route(
            "/v1/push/devices/{sp_token}/topics",
            post(sdk::push::subscribe_topic::handle),
        )
        .route(
            "/v1/push/devices/{sp_token}/topics/{topic}",
            delete(sdk::push::unsubscribe_topic::handle),
        )
        // Counting is its own path rather than a flag on the send. A
        // mistyped flag sends to forty thousand people; a mistyped
        // path is a 404, and a push cannot be recalled.
        .route("/v1/push/audience/count", post(sdk::push::count::handle))
        .route("/v1/push/sends", post(sdk::push::send::handle))
        .route("/v1/push/sends/{send_id}", get(sdk::push::batch::summary))
        .route(
            "/v1/push/sends/{send_id}/deliveries",
            get(sdk::push::batch::deliveries),
        )
        .route(
            "/v1/push/deliveries/{delivery_id}",
            get(sdk::push::receipt::handle),
        )
        .route(
            "/v1/push/deliveries/{delivery_id}/ack",
            post(sdk::push::ack::handle),
        )
        .route(
            "/v1/push/expo-compat/send",
            post(sdk::push::expo_send::handle),
        )
        .route(
            "/v1/push/expo-compat/receipts/{delivery_id}",
            get(sdk::push::expo_receipt::handle),
        )
        .route(
            "/v1/push/users/{user_key}/preferences",
            get(sdk::push::get_preferences::handle),
        )
        .route(
            "/v1/push/users/{user_key}/preferences/{category}",
            axum::routing::put(sdk::push::put_preference::handle),
        )
        // Order matters: the limiter runs *after* the bearer check, so
        // it has a token to key on and an unauthenticated flood is
        // rejected earlier and more cheaply.
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            rate_limit_mw,
        ))
        .layer(axum_middleware::from_fn_with_state(
            token_store,
            bearer_middleware,
        ))
        .with_state(state.clone());

    // ── Dashboard + admin — cookie session ──
    let admin_routes = Router::new()
        // projects
        .route(
            "/admin/api/projects",
            get(projects::list).post(admin::projects::create),
        )
        .route(
            "/admin/api/projects/{project_id}",
            get(admin::projects::get)
                .patch(admin::projects::update)
                .delete(admin::projects::delete),
        )
        // tokens
        .route(
            "/admin/api/projects/{project_id}/tokens",
            get(admin::tokens::list).post(admin::tokens::create),
        )
        .route(
            "/admin/api/tokens/{token_id}",
            delete(admin::tokens::revoke),
        )
        // users + assignments (owner)
        .route(
            "/admin/api/users",
            get(admin::users::list).post(admin::users::create),
        )
        .route("/admin/api/users/{user_id}", delete(admin::users::delete))
        .route(
            "/admin/api/users/{user_id}/projects/{project_id}",
            axum::routing::put(admin::users::assign).delete(admin::users::unassign),
        )
        // issues — the Inbox and the detail page
        .route("/admin/api/issues", get(issues::list))
        .route("/admin/api/issues/{issue_id}", get(issues::get))
        .route(
            "/admin/api/issues/{issue_id}/resolve",
            post(issues::resolve),
        )
        .route("/admin/api/issues/{issue_id}/ignore", post(issues::ignore))
        .route("/admin/api/issues/{issue_id}/reopen", post(issues::reopen))
        .route("/admin/api/issues/{issue_id}/assign", post(issues::assign))
        .route("/admin/api/issues/{issue_id}/notes", post(issues::add_note))
        .route(
            "/admin/api/issues/{issue_id}/events",
            get(issues::occurrences),
        )
        // instruments — the devices panel
        .route(
            "/admin/api/projects/{project_id}/health",
            get(admin::projects::health),
        )
        .route(
            "/admin/api/projects/{project_id}/environments",
            get(admin::projects::environments),
        )
        .route(
            "/admin/api/projects/{project_id}/context-keys",
            get(admin::projects::context_keys),
        )
        .route(
            "/admin/api/projects/{project_id}/context-values",
            get(admin::projects::context_values),
        )
        .route(
            "/admin/api/projects/{project_id}/instruments",
            get(instruments::get),
        )
        // events + attachments (reached from issues, never browsed)
        .route("/admin/api/events/{event_id}", get(events::get))
        .route("/admin/api/events/{event_id}/context", get(events::context))
        .route("/admin/api/attachments/{ref}", get(attachments::get))
        // releases + artifacts
        .route(
            "/admin/api/projects/{project_id}/releases",
            get(admin::releases::list),
        )
        .route(
            "/admin/api/projects/{project_id}/releases/{release_id}/artifacts",
            get(admin::releases::list_artifacts)
                .post(artifacts_upload::upload)
                .layer(axum::extract::DefaultBodyLimit::max(256 * 1024 * 1024)),
        )
        .route(
            "/admin/api/releases/{release_id}",
            delete(admin::releases::delete),
        )
        // audit (owner)
        .route("/admin/api/audit", get(audit::list))
        // notification channel (email)
        .route("/admin/api/smtp", get(notify_admin::smtp_status))
        .route("/admin/api/smtp/test", post(notify_admin::smtp_test))
        .route(
            "/admin/api/notification-prefs",
            get(notify_admin::prefs_list).put(notify_admin::prefs_put),
        )
        // Push — a first-class capability of the product since v1.5,
        // not a subsystem parked next to it. The dashboard reaches
        // every one of these.
        .route(
            "/admin/api/projects/{project_id}/push/credentials",
            get(admin::push_credentials::list).post(admin::push_credentials::create),
        )
        // By id, not by kind: a project may now hold several of the
        // same kind — the one that sends, and the ones being tried.
        .route(
            "/admin/api/projects/{project_id}/push/credentials/{credential_id}",
            delete(admin::push_credentials::delete),
        )
        // Ask the vendor. Delivers nothing; see push_credential_probe.
        .route(
            "/admin/api/projects/{project_id}/push/credentials/{credential_id}/probe",
            post(admin::push_credentials::probe),
        )
        .route(
            "/admin/api/projects/{project_id}/push/credentials/{credential_id}/activate",
            post(admin::push_credentials::activate),
        )
        .route(
            "/admin/api/projects/{project_id}/push/test",
            post(admin::test_push::handle),
        )
        .route(
            "/admin/api/projects/{project_id}/push/readiness",
            get(admin::push_readiness::handle),
        )
        .route(
            "/admin/api/projects/{project_id}/push/health",
            get(admin::push_sends::health),
        )
        .route(
            "/admin/api/projects/{project_id}/push/devices",
            get(admin::push_devices::list),
        )
        .route(
            "/admin/api/projects/{project_id}/push/audience/preview",
            post(admin::push_audience::preview),
        )
        .route(
            "/admin/api/projects/{project_id}/push/audience/send",
            post(admin::push_audience::send),
        )
        .route(
            "/admin/api/projects/{project_id}/push/devices/{token_id}/revoke",
            post(admin::push_devices::revoke),
        )
        .route(
            "/admin/api/projects/{project_id}/push/sends",
            get(admin::push_sends::list),
        )
        .route(
            "/admin/api/projects/{project_id}/push/sends/{send_id}/retry",
            post(admin::push_sends::retry),
        )
        .route(
            "/admin/api/projects/{project_id}/push/sends/_retry_all_failed",
            post(admin::push_sends::retry_all_failed),
        )
        // session-bound auth endpoints
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/change-password", post(auth::change_password))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            session_middleware,
        ))
        .with_state(state.clone());

    // ── Credentialed auth endpoints with their own per-IP limiter ──
    let auth_bruteforce_routes = Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/forgot-password", post(auth::forgot_password))
        .route("/auth/reset-password", post(auth::reset_password))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_rate_limit_mw,
        ))
        .with_state(state.clone());

    // ── AI closed loop — Bearer api-scope token ──
    let api_token_store = TokenStore::new(state.pool.clone());
    let api_routes = Router::new()
        .route("/api/issues", get(api::list))
        .route("/api/issues/{issue_id}/bundle", get(api::bundle))
        .route("/api/issues/{issue_id}/notes", post(api::add_note))
        .route("/api/issues/{issue_id}/resolve", post(api::resolve))
        .route("/api/probes:sync", post(api::probes_sync))
        .layer(axum_middleware::from_fn_with_state(
            api_token_store,
            bearer_middleware,
        ))
        .with_state(state.clone());

    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .route("/metrics", get(metrics_prom::handle))
        .with_state(state)
        .merge(admin_routes)
        .merge(auth_bruteforce_routes)
        .merge(api_routes)
        .merge(sdk_routes)
        .fallback(spa_or_api_404)
        // Outermost, so it wraps every route above. A panic inside a
        // handler otherwise takes the tokio worker with it and the
        // client sees a dropped socket — no status, no body. That is
        // not a hypothetical: a backend whose Describe under-reported
        // columns turned `Row::get` into a panic on the artifact
        // upload and again on push readiness, and both times the
        // uploader got `curl: (52) Empty reply from server`.
        //
        // The panic is still a bug and still logged. This only decides
        // whether the caller can read what happened.
        .layer(CatchPanicLayer::custom(panic_as_500))
}

/// Turn a caught panic into a 500 the caller can branch on, and a log
/// line whoever runs this can find.
#[allow(clippy::needless_pass_by_value)] // the layer's callback signature
fn panic_as_500(err: Box<dyn std::any::Any + Send + 'static>) -> axum::response::Response {
    let detail = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap_or("panic");
    tracing::error!(panic = detail, "handler panicked — returning 500");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "internal" })),
    )
        .into_response()
}

/// Path prefixes that belong to the HTTP API, not to the SPA.
///
/// Anything under these is machine-facing: shipped SDKs hit `/v1/*`,
/// the dashboard hits `/admin/api/*` and `/auth/*`, and `/api/*` is
/// the legacy prefix Caddy still forwards from old clients.
const API_PREFIXES: [&str; 4] = ["/v1/", "/admin/api/", "/auth/", "/api/"];

/// True when `path` belongs to the HTTP API rather than the SPA.
fn is_api_path(path: &str) -> bool {
    API_PREFIXES.iter().any(|p| path.starts_with(p))
}

/// Where Vite emits its content-hashed bundles. Nothing under here is
/// an SPA route, so a miss is a genuinely absent file.
const ASSET_PREFIX: &str = "/assets/";

/// True when `path` addresses a build artifact rather than an SPA
/// route. Matched by prefix rather than by file extension on purpose:
/// an extension heuristic would misfire on route segments that
/// legitimately contain dots (release names like `app@5.4.2+361`).
fn is_asset_path(path: &str) -> bool {
    path.starts_with(ASSET_PREFIX)
}

/// Fallback for everything the router didn't match.
///
/// Unmatched **API** paths must answer with a JSON 404. Serving the
/// SPA shell there — which is what a bare `fallback_service` does,
/// and what production did until 2026-07-20 — hands an SDK
/// `200 <!doctype html>`: it reads as success, and any JSON parse of
/// the body fails somewhere far from the cause.
///
/// A missing **asset** must 404 too. Returning the shell for
/// `/assets/index-OLD.js` — which happens to every browser holding a
/// cached index.html across a redeploy — makes the browser parse HTML
/// as JavaScript and fail with a syntax error instead of a clean 404
/// it can recover from.
///
/// Everything else is an SPA deep link (`/projects/x/issues`) and
/// still resolves to `index.html` with 200 so React Router can take
/// over.
async fn spa_or_api_404(req: axum::extract::Request) -> axum::response::Response {
    use axum::response::IntoResponse;

    let path = req.uri().path();
    if is_api_path(path) {
        let detail = format!("no route for {} {path}", req.method());
        return (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "not_found",
                "detail": detail,
            })),
        )
            .into_response();
    }

    // Assets serve from a ServeDir with no index fallback, so a miss
    // stays a 404 instead of becoming the shell. The two ServeDirs
    // differ in their fallback type parameter, hence the two arms.
    let path_owned = path.to_string();
    let served = if is_asset_path(path) {
        tower::ServiceExt::oneshot(webapp_assets(), req)
            .await
            .map(IntoResponse::into_response)
    } else {
        tower::ServiceExt::oneshot(webapp_dir(), req)
            .await
            .map(IntoResponse::into_response)
    };

    match served {
        Ok(mut res) => {
            // Cache posture: the shell must revalidate every load —
            // a heuristically-cached index.html kept users on stale
            // bundles across deploys until a hard refresh. Assets
            // are content-hashed and can live forever.
            let cache = if is_asset_path(path_owned.as_str()) {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            if let Ok(v) = axum::http::HeaderValue::from_str(cache) {
                res.headers_mut()
                    .insert(axum::http::header::CACHE_CONTROL, v);
            }
            res
        }
        Err(e) => {
            tracing::error!(%e, "webapp static serve failed");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Static-file service for the bundled webapp. Resolves to the
/// path in `SENTORI_WEBAPP_DIST` env-var, defaulting to
/// `/app/webapp` inside the container.
///
/// Unknown paths resolve to `index.html` with 200 so React Router
/// can handle SPA deep links. `spa_or_api_404` gates which requests
/// reach here — API prefixes never do.
/// Same root as [`webapp_dir`] but with **no** index fallback: a
/// request for a build artifact that isn't on disk gets ServeDir's
/// native 404. Used for `/assets/*` — see [`is_asset_path`].
fn webapp_assets() -> tower_http::services::ServeDir {
    tower_http::services::ServeDir::new(webapp_root())
}

/// Root directory holding the compiled SPA.
fn webapp_root() -> String {
    std::env::var("SENTORI_WEBAPP_DIST").unwrap_or_else(|_| "/app/webapp".to_string())
}

fn webapp_dir() -> tower_http::services::ServeDir<tower_http::services::ServeFile> {
    use tower_http::services::{ServeDir, ServeFile};
    let root = webapp_root();
    let index = format!("{root}/index.html");
    // `fallback` (not `not_found_service`) — the latter wraps the
    // fallback in SetStatus(404), which is for custom 404 pages;
    // SPA deep links must serve index.html with 200.
    ServeDir::new(&root).fallback(ServeFile::new(index))
}

#[cfg(test)]
mod fallback_tests {
    use super::{is_api_path, is_asset_path};

    #[test]
    fn api_prefixes_are_machine_facing() {
        for p in [
            "/v1/events",
            "/v1/does-not-exist",
            "/v1/projects/abc/issues",
            "/admin/api/members",
            "/admin/api/nope",
            "/auth/me",
            "/auth/nope",
            "/api/legacy-thing",
        ] {
            assert!(is_api_path(p), "{p} must answer with JSON, not the SPA");
        }
    }

    #[test]
    fn spa_routes_are_not_api() {
        // Every one of these is a React Router path; sending JSON 404
        // here would break deep links.
        for p in [
            "/",
            "/login",
            "/register",
            "/verify",
            "/reset-password",
            "/projects/abc/issues",
            "/assets/index-abc123.js",
            "/some/deep/link",
        ] {
            assert!(!is_api_path(p), "{p} must fall through to the SPA");
        }
    }

    #[test]
    fn assets_are_files_not_spa_routes() {
        // A miss here must stay a 404: a browser holding a cached
        // index.html across a redeploy asks for the old hashed bundle,
        // and answering with the shell makes it parse HTML as JS.
        for p in [
            "/assets/index-BTIykhei.js",
            "/assets/index-BwO1nBzl.css",
            "/assets/index-OLD.js",
        ] {
            assert!(is_asset_path(p), "{p} must 404 when absent");
        }
        // Route segments may legitimately contain dots (release names
        // like `app@5.4.2+361`); only the /assets/ prefix decides.
        for p in ["/", "/login", "/projects/abc/releases", "/releases/1.2.3"] {
            assert!(!is_asset_path(p), "{p} must fall through to the SPA");
        }
    }

    #[test]
    fn prefix_match_does_not_leak_to_sibling_paths() {
        // `/v1` and `/apidocs` share a prefix with an API root but are
        // not under it — the trailing slash in API_PREFIXES is what
        // keeps them on the SPA side.
        for p in ["/v1", "/apidocs", "/authors", "/administration"] {
            assert!(!is_api_path(p), "{p} must not be treated as API");
        }
    }
}

#[cfg(test)]
mod router_tests {
    /// The layer that turns a handler panic into an answerable 500.
    ///
    /// Without it a panic kills the tokio worker and the client gets a
    /// dropped socket — no status, no body. That happened twice in one
    /// week, on two endpoints, from the same cause. A layer nobody
    /// notices is removed by the next person tidying the stack, so it
    /// is pinned here rather than trusted to survive review.
    /// The layer only works if panics unwind.
    ///
    /// `[profile.release] panic = "abort"` makes `CatchPanicLayer`
    /// inert: the process dies before any handler sees the unwind. It
    /// was set, and the layer shipped in 3.3.0 doing nothing in the
    /// only build anyone runs — the test above passed the whole time,
    /// because the layer was present and correctly placed. Presence is
    /// not the property; catching is.
    ///
    /// This test reads a manifest, which is the weaker half. The real
    /// check needs a release binary and was run by hand on 2026-08-19:
    /// add a route that panics, `cargo build --release`, request it.
    /// The answer must be `500` with the process still serving
    /// afterwards; under `abort` it is a dropped socket and a restart.
    /// The probe route is not committed — a handler that panics on
    /// request has no business in a shipped binary — so re-run it the
    /// same way if this ever needs proving again.
    ///
    /// Unwinding costs 1.88 MB of binary (8.98 → 10.86), measured the
    /// same day.
    #[test]
    fn panics_unwind_so_the_layer_can_catch_them() {
        let manifest = include_str!("../../Cargo.toml");
        let release = manifest
            .split("[profile.release]")
            .nth(1)
            .unwrap_or("")
            // Stop at the next section, or `panic` set in a later
            // profile would read as this one's.
            .split("\n[")
            .next()
            .unwrap_or("");
        // Comments stripped first: the note explaining why the line is
        // absent contains the line, and the first version of this test
        // failed on its own explanation.
        let effective: String = release
            .lines()
            .map(|l| l.split('#').next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !effective.contains("panic = \"abort\""),
            "release builds abort on panic — CatchPanicLayer cannot catch an abort, \
             and one bad request takes the whole instance down"
        );
    }

    #[test]
    fn every_route_is_wrapped_against_a_handler_panic() {
        let src = include_str!("mod.rs");
        assert!(
            src.contains("CatchPanicLayer"),
            "the panic-catching layer is gone — a handler panic is a dropped socket again"
        );
        // Outermost, or the routes merged after it are uncovered.
        assert!(
            matches!(
                (
                    src.rfind("CatchPanicLayer::custom"),
                    src.find(".fallback(spa_or_api_404)"),
                ),
                (Some(at), Some(fallback)) if at > fallback
            ),
            "the layer must sit outside every merge, or the routes below it are unwrapped"
        );
    }
}
