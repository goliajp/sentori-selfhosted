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
use axum::routing::{delete, get, patch, post};
use sentori_ingest_token::{TokenStore, bearer_middleware};
use serde_json::json;

use crate::saasadmin_mw::saasadmin_only;
use crate::session_mw::session_middleware;
use crate::state::AppState;

mod activity_log;
mod admin;
mod alerts;
mod alerts_fire;
mod api_describe;
mod artifacts_upload;
mod attachments;
mod audit;
mod auth;
mod billing;
mod cert;
mod events;
mod events_live;
mod health;
mod ingest;
mod issue_comments;
mod issue_watchers;
mod issues;
mod metrics;
mod metrics_prom;
mod notifications;
mod oauth;
mod projects;
mod replays;
mod runtime_metrics_query;
mod saved_views;
mod sdk;
mod search;
mod self_test;
mod sessions_admin;
mod spans;
mod stats;
mod stripe_webhook;
pub mod tenant;
mod track_query;
mod usage;
mod user_reports_query;
mod workspaces;

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
    // SDK ingest routes — Bearer st_pk_ gated.
    let token_store = TokenStore::new(state.pool.clone());
    let sdk_routes = Router::new()
        // ── events ──
        .route("/v1/events", post(sdk::events::handle))
        .route("/v1/events:batch", post(sdk::events_batch::handle))
        .route(
            "/v1/events/{event_id}/attachments/{kind}",
            post(sdk::events_attachments::handle),
        )
        .route("/v1/events/_recent", get(sdk::events_recent::handle))
        // ── tracing ──
        .route("/v1/spans", post(sdk::spans::handle))
        .route("/v1/spans:batch", post(sdk::spans_batch::handle))
        // ── lifecycle ──
        .route("/v1/heartbeat", post(sdk::heartbeat::handle))
        .route("/v1/sessions", post(sdk::sessions::handle))
        .route("/v1/deploys", post(sdk::deploys::handle))
        .route(
            "/v1/releases/{release}/artifacts",
            post(artifacts_upload::upload_by_release_name),
        )
        // ── metrics ──
        .route("/v1/metrics:batch", post(sdk::metrics::handle))
        .route(
            "/v1/runtime-metrics:batch",
            post(sdk::runtime_metrics::handle),
        )
        // ── analytics ──
        .route("/v1/track:batch", post(sdk::track::handle))
        // ── security ──
        .route("/v1/security:report", post(sdk::security_report::handle))
        .route("/v1/security/link", post(sdk::security_link::handle))
        .route("/v1/security/score", get(sdk::security_score::handle))
        // ── control ──
        .route("/v1/control/poll", get(sdk::control::handle))
        // ── feedback ──
        .route("/v1/user-reports", post(sdk::user_reports::handle))
        // ── push (11 endpoints) ──
        .route("/v1/push/tokens", post(sdk::push::register_token::handle))
        .route(
            "/v1/push/tokens/{handle}",
            delete(sdk::push::revoke_token::handle),
        )
        .route(
            "/v1/push/tokens/{handle}/topics",
            post(sdk::push::subscribe_topic::handle),
        )
        .route(
            "/v1/push/tokens/{handle}/topics/{topic}",
            delete(sdk::push::unsubscribe_topic::handle),
        )
        .route("/v1/push/send", post(sdk::push::send::handle))
        .route(
            "/v1/push/receipts/{send_id}",
            get(sdk::push::receipt::handle),
        )
        .route("/v1/push/sends/{send_id}/ack", post(sdk::push::ack::handle))
        .route(
            "/v1/push/expo-compat/send",
            post(sdk::push::expo_send::handle),
        )
        .route(
            "/v1/push/expo-compat/receipts/{send_id}",
            get(sdk::push::expo_receipt::handle),
        )
        .route(
            "/v1/push/users/{fp_hex}/preferences",
            get(sdk::push::get_preferences::handle),
        )
        .route(
            "/v1/push/users/{fp_hex}/preferences/{category}",
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

    // Admin routes — session-gated (cookie or Bearer session_token).
    let admin_routes = Router::new()
        // Workspace switcher (multi-workspace 1:N): list the caller's
        // memberships + repoint the current session.
        .route("/admin/api/workspaces", get(workspaces::list))
        .route("/admin/api/workspaces/switch", post(workspaces::switch))
        .route(
            "/admin/api/projects/{project_id}/tokens",
            get(admin::tokens::list).post(admin::tokens::create),
        )
        .route(
            "/admin/api/tokens/{token_id}",
            delete(admin::tokens::revoke),
        )
        .route("/admin/api/projects", post(admin::projects::create))
        .route(
            "/admin/api/projects/{project_id}",
            get(admin::projects::get)
                .patch(admin::projects::update)
                .delete(admin::projects::delete),
        )
        .route(
            "/admin/api/projects/{project_id}/push/credentials",
            get(admin::push_credentials::list).post(admin::push_credentials::upsert),
        )
        .route(
            "/admin/api/projects/{project_id}/push/credentials/{kind}",
            delete(admin::push_credentials::delete),
        )
        // ── admin: test push send ──────────────────────────
        .route(
            "/admin/api/projects/{project_id}/push/test",
            post(admin::test_push::handle),
        )
        // ── admin: push sends list (DLQ / triage) ─────────
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
        .route(
            "/admin/api/webhooks/test",
            post(admin::test_webhook::handle),
        )
        // ── self-serve billing (caller's own workspace) ──────
        .route("/admin/api/billing", get(billing::get))
        .route("/admin/api/billing/checkout", post(billing::checkout))
        .route("/admin/api/billing/portal", post(billing::portal))
        .route("/admin/api/members", get(admin::members::list))
        .route(
            "/admin/api/members/{user_id}",
            patch(admin::members::update_role).delete(admin::members::remove),
        )
        .route(
            "/admin/api/invites",
            get(admin::invites::list).post(admin::invites::create),
        )
        // Accept lives before `{id}` conceptually but is a distinct
        // path; the logged-in caller joins the token's workspace.
        .route("/admin/api/invites/accept", post(admin::invites::accept))
        .route("/admin/api/invites/{id}", delete(admin::invites::revoke))
        .route(
            "/admin/api/projects/{project_id}/cert/watches",
            post(admin::cert_watch::add),
        )
        .route(
            "/admin/api/projects/{project_id}/cert/watches/{domain}",
            delete(admin::cert_watch::remove),
        )
        .route(
            "/admin/api/projects/{project_id}/integrations",
            get(admin::integrations::list).post(admin::integrations::upsert),
        )
        .route(
            "/admin/api/projects/{project_id}/integrations/{kind}",
            delete(admin::integrations::delete),
        )
        .route(
            "/admin/api/projects/{project_id}/integrations/{kind}/active",
            patch(admin::integrations::set_active),
        )
        // ── admin: issue watchers (session-scoped current user) ──
        .route(
            "/admin/api/issues/{issue_id}/watchers",
            post(issue_watchers::join).delete(issue_watchers::leave),
        )
        // ── admin: issue comments (session-scoped author) ──
        .route(
            "/admin/api/issues/{issue_id}/comments",
            post(issue_comments::create),
        )
        .route(
            "/admin/api/issues/{issue_id}/comments/{comment_id}",
            delete(issue_comments::delete),
        )
        // ── admin: endpoint probes (synthetic monitor) ──
        .route(
            "/admin/api/projects/{project_id}/endpoint-probes",
            get(admin::endpoint_probes::list).post(admin::endpoint_probes::create),
        )
        .route(
            "/admin/api/endpoint-probes/{probe_id}",
            patch(admin::endpoint_probes::patch).delete(admin::endpoint_probes::delete),
        )
        // ── admin: releases ───────────────────────────────
        .route(
            "/admin/api/projects/{project_id}/releases",
            get(admin::releases::list),
        )
        // Per-project access for the `user` role. The store behind
        // these has existed since the identity crate was written with
        // no way to reach it, which is why `user` members saw every
        // project.
        .route(
            "/admin/api/projects/{project_id}/visibility",
            get(admin::visibility::list),
        )
        .route(
            "/admin/api/projects/{project_id}/visibility/{user_id}",
            axum::routing::put(admin::visibility::grant).delete(admin::visibility::revoke),
        )
        .route(
            "/admin/api/projects/{project_id}/releases/{release_id}/artifacts",
            get(admin::releases::list_artifacts).post(artifacts_upload::upload),
        )
        .route(
            "/admin/api/releases/{release_id}",
            delete(admin::releases::delete),
        )
        // Session-scoped self endpoints
        .route("/auth/me", get(auth::me))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/sessions", get(sessions_admin::list))
        .route(
            "/auth/sessions/{id_hash_hex}",
            delete(sessions_admin::revoke),
        )
        .route("/auth/notifications", get(notifications::list))
        .route(
            "/auth/notifications/_read_all",
            post(notifications::read_all),
        )
        .route(
            "/auth/notifications/{id}/read",
            post(notifications::read_one),
        )
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            session_middleware,
        ))
        .with_state(state.clone());

    // SaaS cross-workspace endpoints — session-gated AND
    // saasadmin-role-gated (env-driven allowlist).
    let saas_routes = Router::new()
        .route(
            "/admin/api/saas/workspaces",
            get(admin::saas::workspaces).post(admin::saas::create_workspace),
        )
        .route(
            "/admin/api/saas/workspaces/{id}",
            delete(admin::saas::delete_workspace),
        )
        .route(
            "/admin/api/saas/workspaces/{id}/plan",
            post(admin::saas::set_plan),
        )
        .route(
            "/admin/api/saas/workspaces/{id}/suspend",
            post(admin::saas::suspend_workspace),
        )
        .route(
            "/admin/api/saas/workspaces/{id}/resume",
            post(admin::saas::resume_workspace),
        )
        .route("/admin/api/saas/stats", get(admin::saas::workspace_stats))
        .layer(axum_middleware::from_fn(saasadmin_only))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            session_middleware,
        ))
        .with_state(state.clone());

    // Dashboard reads — session-gated.
    //
    // These were public until 2026-07-20 ("Phase E will gate with
    // cookie session"), which meant any caller could read a
    // customer's issues, events, traces, metrics and replays, with
    // `/v1/projects` handing out the project ids to address them by.
    // SDK ingest is a separate group above and keeps its own Bearer
    // st_pk_ gate; ops probes stay open below for k8s and Prometheus.
    let dashboard_routes = Router::new()
        .route("/v1/_describe", get(api_describe::describe))
        .route("/v1/_self_test", get(self_test::handle))
        .route("/v1/projects", get(projects::list))
        .route("/v1/projects/{project_id}/issues", get(issues::list))
        .route(
            "/v1/projects/{project_id}/issues/{issue_id}",
            get(issues::get).patch(issues::patch),
        )
        .route("/v1/issues/{issue_id}/watchers", get(issue_watchers::list))
        .route("/v1/issues/{issue_id}/comments", get(issue_comments::list))
        .route("/v1/issues/{issue_id}/activity", get(activity_log::list))
        .route(
            "/v1/projects/{project_id}/issues/_bulk_patch",
            post(issues::bulk_patch),
        )
        .route("/v1/projects/{project_id}/events", get(events::list))
        .route("/v1/projects/{project_id}/events/trend", get(events::trend))
        .route(
            "/v1/projects/{project_id}/events/_recent",
            get(events_live::handle),
        )
        // Registered after the two literal siblings above. The router
        // prefers a static segment over a capture regardless of order,
        // but `{event_id}` parses as a Uuid — if it ever did win,
        // `/events/trend` would 400 instead of 404, so keep the
        // precedence visible in the source too.
        .route(
            "/v1/projects/{project_id}/events/{event_id}",
            get(events::get),
        )
        .route("/v1/projects/{project_id}/traces", get(spans::list_traces))
        .route(
            "/v1/projects/{project_id}/traces/{trace_id}",
            get(spans::get_trace),
        )
        .route(
            "/v1/projects/{project_id}/metrics",
            get(metrics::list_names),
        )
        // `track_events` had an ingest route and no way back out; these
        // three read it along the three indexes it already carries.
        .route(
            "/v1/projects/{project_id}/track/names",
            get(track_query::names),
        )
        .route(
            "/v1/projects/{project_id}/track/series",
            get(track_query::series),
        )
        .route(
            "/v1/projects/{project_id}/track/recent",
            get(track_query::recent),
        )
        // The SDK's own perf rollups — the numbers behind the promise
        // that Sentori does not make the host app stutter.
        .route(
            "/v1/projects/{project_id}/runtime-metrics",
            get(runtime_metrics_query::names),
        )
        .route(
            "/v1/projects/{project_id}/runtime-metrics/series",
            get(runtime_metrics_query::series),
        )
        // What a user typed when the app asked them what happened.
        .route(
            "/v1/projects/{project_id}/user-reports",
            get(user_reports_query::list),
        )
        .route(
            "/v1/projects/{project_id}/metrics/{name}/timeseries",
            get(metrics::timeseries),
        )
        // Crash evidence: what the SDK captured alongside the event.
        .route(
            "/v1/projects/{project_id}/events/{event_id}/attachments",
            get(attachments::list),
        )
        .route(
            "/v1/projects/{project_id}/attachments/{ref_id}",
            get(attachments::get),
        )
        .route("/v1/projects/{project_id}/replays", get(replays::list))
        .route(
            "/v1/projects/{project_id}/replays/{replay_id}/ndjson",
            get(replays::ndjson),
        )
        .route("/v1/projects/{project_id}/stats", get(stats::project_stats))
        .route("/v1/projects/{project_id}/search", get(search::search))
        .route(
            "/v1/projects/{project_id}/cert/watches",
            get(cert::list_watches),
        )
        .route(
            "/v1/projects/{project_id}/cert/observations",
            get(cert::list_observations),
        )
        .route(
            "/v1/projects/{project_id}/alerts",
            get(alerts::list_for_project),
        )
        .route("/v1/usage", get(usage::current))
        .route("/v1/audit", get(audit::list))
        .route(
            "/v1/alerts",
            get(alerts::list_workspace).post(alerts::create),
        )
        .route(
            "/v1/alerts/{id}",
            get(alerts::get)
                .patch(alerts::update)
                .delete(alerts::delete),
        )
        .route("/v1/alerts/{id}/_fire_test", post(alerts_fire::fire_test))
        .route(
            "/v1/saved-views",
            get(saved_views::list_workspace).post(saved_views::create),
        )
        .route(
            "/v1/saved-views/{id}",
            get(saved_views::get)
                .patch(saved_views::patch)
                .delete(saved_views::delete),
        )
        // legacy fresh-start ingest stubs (defer to SDK-auth path)
        .route(
            "/v1/projects/{project_id}/ingest",
            post(ingest::ingest_event),
        )
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            session_middleware,
        ))
        .with_state(state.clone());

    // Login-shaped endpoints share a per-IP limiter. They have no
    // bearer token to key on, and repeated calls with different bodies
    // is exactly what a brute force looks like. The default budget is
    // ten attempts per five minutes per IP — an operator loosens it
    // via SENTORI_AUTH_RATELIMIT_PER_IP / _WINDOW_SEC without touching
    // the ingest tunables.
    let auth_bruteforce_routes = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/forgot-password", post(auth::forgot))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_rate_limit_mw,
        ))
        .with_state(state.clone());

    // Ops probes + the auth endpoints needed to obtain a session.
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .route("/metrics", get(metrics_prom::handle))
        // ── stripe webhook (public; HMAC-signature authed) ──
        .route("/webhooks/stripe", post(stripe_webhook::ingest))
        // ── auth: dashboard user lifecycle (public) ──────
        //
        // register / login / forgot-password are merged in below as
        // `auth_bruteforce_routes` with their own low-cap per-IP
        // limiter. verify / reset / change-password are shaped
        // differently: verify and reset only work with a fresh
        // single-use token from an email we send, and change-password
        // sits behind the session so it is bounded by that.
        .route("/auth/verify", post(auth::verify))
        .route("/auth/reset-password", post(auth::reset))
        .route("/auth/change-password", post(auth::change_password))
        // ── auth: dashboard OAuth (public) ──────────────
        // Public for the same reason the rows above are: these are
        // how a session is obtained, so gating them on one would
        // lock every OAuth user out.
        .route("/auth/oauth/providers", get(oauth::providers))
        .route("/auth/oauth/{provider}/start", get(oauth::start))
        .route("/auth/oauth/{provider}/callback", get(oauth::callback))
        .with_state(state)
        .merge(dashboard_routes)
        .merge(admin_routes)
        .merge(saas_routes)
        .merge(auth_bruteforce_routes)
        .merge(sdk_routes)
        .fallback(spa_or_api_404)
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
        Ok(res) => res,
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
