//! Dashboard OAuth login — GitHub + Google authorization-code flow.
//!
//! Hand-rolled rather than pulled from the `oauth2` crate: both
//! providers implement the same small slice of the spec and reqwest
//! is already a dependency, so the crate would add transitive weight
//! for two URL builders and two POSTs.
//!
//! Three endpoints:
//!
//!   GET /auth/oauth/providers
//!       `{"github": bool, "google": bool}` — the dashboard renders
//!       only the buttons whose credentials are configured.
//!
//!   GET /auth/oauth/{provider}/start
//!       Mints a 256-bit state token, sets it as a short-lived
//!       HttpOnly cookie, and 302s to the provider's authorize URL
//!       carrying the same token.
//!
//!   GET /auth/oauth/{provider}/callback?code&state
//!       Verifies state against the cookie, exchanges the code,
//!       fetches userinfo, resolves the account, mints a session
//!       and 302s to the dashboard root.
//!
//! Account resolution, in order:
//!   1. `user_oauth_identities` by `(provider, subject)` — the only
//!      match that is authoritative, since an email address can be
//!      reassigned upstream but a subject cannot.
//!   2. `users` by the provider's **verified** email — first-time
//!      linking only, so an operator who registered with a password
//!      can later sign in with the same address via OAuth.
//!   3. Otherwise a new user, email-verified because the provider
//!      vouched for it, with a password hash that cannot verify.
//!
//! Environment:
//!   SENTORI_BASE_URL                  → callback URL prefix
//!   SENTORI_GITHUB_CLIENT_ID/_SECRET  → GitHub OAuth app
//!   SENTORI_GOOGLE_CLIENT_ID/_SECRET  → Google OAuth client
//!
//! Both providers must have their callback registered as
//! `${SENTORI_BASE_URL}/auth/oauth/{provider}/callback`.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use sentori_auth_session::RequestMeta;
use sentori_workspace_identity::{Identity, Role, UserId};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;

const STATE_COOKIE: &str = "sentori_oauth_state";
const STATE_TTL_MINUTES: i64 = 10;

/// Where a successful login lands. The SPA resolves the signed-in user
/// via `/auth/me`, so no state travels in the URL.
///
/// `/main`, deliberately not `/`: this is a **server-side** 302, i.e. a
/// full page load. On the SaaS deployment Caddy serves the marketing
/// site at `/` and routes every other path to this server's SPA, so a
/// redirect to `/` handed freshly-authenticated users the marketing
/// homepage instead of the dashboard. (Password login dodged it by
/// navigating client-side inside the already-loaded SPA — only the
/// OAuth callback, being a fresh browser navigation, hit Caddy.)
/// `/main` reaches the SPA on both SaaS and self-hosted and survives a
/// refresh or bookmark, so it is the canonical dashboard home.
const DASHBOARD_ROOT: &str = "/main";

struct ProviderConfig {
    name: &'static str,
    authorize_url: &'static str,
    token_url: &'static str,
    user_info_url: &'static str,
    scope: &'static str,
    /// Appended to the authorize URL verbatim. Provider-specific
    /// because `prompt` is a Google parameter; GitHub reads it
    /// differently and is better left alone.
    extra_authorize_params: &'static [(&'static str, &'static str)],
    /// Maps the provider's userinfo JSON to the fields we store.
    /// Returns `None` when a required field is absent or the email
    /// is not verified.
    extract_user: fn(&Value) -> Option<UserInfo>,
}

#[derive(Debug, PartialEq, Eq)]
struct UserInfo {
    subject: String,
    email: String,
    display_name: Option<String>,
    avatar_url: Option<String>,
}

fn resolve_provider(name: &str) -> Option<ProviderConfig> {
    match name {
        "github" => Some(ProviderConfig {
            name: "github",
            authorize_url: "https://github.com/login/oauth/authorize",
            token_url: "https://github.com/login/oauth/access_token",
            user_info_url: "https://api.github.com/user",
            scope: "read:user user:email",
            extra_authorize_params: &[],
            extract_user: extract_github,
        }),
        "google" => Some(ProviderConfig {
            name: "google",
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            user_info_url: "https://openidconnect.googleapis.com/v1/userinfo",
            scope: "openid email profile",
            extra_authorize_params: &[("prompt", "select_account")],
            extract_user: extract_google,
        }),
        _ => None,
    }
}

/// GitHub's `/user` response carries an `email` field, but it is the
/// *public profile* address: the user types it in freely and GitHub
/// never confirms it. So this extractor refuses to read it, and
/// instead requires the `email` / `email_verified` pair that the
/// callback injects from `/user/emails` (see [`primary_verified_email`]).
fn extract_github(v: &Value) -> Option<UserInfo> {
    if !v
        .get("email_verified")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let id = v.get("id").and_then(Value::as_i64)?;
    let email = v.get("email").and_then(Value::as_str)?;
    if email.is_empty() {
        return None;
    }
    Some(UserInfo {
        subject: id.to_string(),
        email: email.to_ascii_lowercase(),
        display_name: v
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| v.get("login").and_then(Value::as_str))
            .map(ToString::to_string),
        avatar_url: v
            .get("avatar_url")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    })
}

fn extract_google(v: &Value) -> Option<UserInfo> {
    if !v
        .get("email_verified")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let sub = v.get("sub").and_then(Value::as_str)?;
    let email = v.get("email").and_then(Value::as_str)?;
    if email.is_empty() {
        return None;
    }
    Some(UserInfo {
        subject: sub.to_string(),
        email: email.to_ascii_lowercase(),
        display_name: v
            .get("name")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        avatar_url: v
            .get("picture")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    })
}

/// The verified primary address out of GitHub's `/user/emails` list.
///
/// Both flags are required: `primary` alone would accept an address
/// the account holder has claimed but never proved, which is exactly
/// the bridge an attacker would use to land on someone else's `users`
/// row in step 2 of account resolution.
fn primary_verified_email(emails: &[Value]) -> Option<String> {
    emails
        .iter()
        .find(|e| {
            e.get("verified").and_then(Value::as_bool).unwrap_or(false)
                && e.get("primary").and_then(Value::as_bool).unwrap_or(false)
        })
        .and_then(|e| e.get("email").and_then(Value::as_str))
        .map(str::to_ascii_lowercase)
}

fn provider_credentials(name: &str) -> Option<(String, String)> {
    let upper = match name {
        "github" => "GITHUB",
        "google" => "GOOGLE",
        _ => return None,
    };
    let id = read_env(&format!("SENTORI_{upper}_CLIENT_ID"))?;
    let secret = read_env(&format!("SENTORI_{upper}_CLIENT_SECRET"))?;
    Some((id, secret))
}

fn read_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn base_url() -> String {
    read_env("SENTORI_BASE_URL").unwrap_or_else(|| "http://localhost:8080".to_string())
}

fn callback_url(base: &str, provider: &str) -> String {
    format!(
        "{}/auth/oauth/{provider}/callback",
        base.trim_end_matches('/')
    )
}

/// Build the provider's authorize URL. Split out from [`start`] so
/// the parameter set — `state` above all — is testable without a
/// request.
fn authorize_url(cfg: &ProviderConfig, client_id: &str, redirect_uri: &str, state: &str) -> String {
    let mut ser = url::form_urlencoded::Serializer::new(String::new());
    ser.append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", cfg.scope)
        .append_pair("state", state)
        .append_pair("response_type", "code");
    for (k, v) in cfg.extra_authorize_params {
        ser.append_pair(k, v);
    }
    format!("{}?{}", cfg.authorize_url, ser.finish())
}

/// Whether the state returned by the provider matches the one this
/// browser was issued at `/start`.
///
/// An absent cookie is a mismatch, not a pass: without it there is
/// nothing tying the callback to a login this browser began, which is
/// the entire point of the parameter.
fn state_matches(cookie: Option<&str>, returned: &str) -> bool {
    // Constant-time. The state is a 64-char hex string, short-lived and
    // single-use, so a timing attack is impractical — but the `==`
    // check that used to sit here still reintroduced a byte-level
    // timing leak that the rest of this codebase takes care to
    // prevent. The zero-cost fix belongs where the cost is measured.
    match cookie {
        Some(c) if !c.is_empty() => {
            sentori_cookie_session::constant_time_eq(c.as_bytes(), returned.as_bytes())
        }
        _ => false,
    }
}

fn random_state() -> String {
    let mut bytes = [0u8; 32];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut bytes);
    hex::encode(bytes)
}

// ── handlers ──────────────────────────────────────────────────

/// GET /auth/oauth/providers
pub async fn providers() -> Json<Value> {
    Json(json!({
        "github": provider_credentials("github").is_some(),
        "google": provider_credentials("google").is_some(),
    }))
}

/// GET /auth/oauth/{provider}/start
pub async fn start(Path(provider): Path<String>, jar: CookieJar) -> Response {
    let Some(cfg) = resolve_provider(&provider) else {
        return bad_request("unknown_provider");
    };
    let Some((client_id, _secret)) = provider_credentials(cfg.name) else {
        return bad_request("oauth_not_configured");
    };

    let state = random_state();
    let base = base_url();
    let location = authorize_url(&cfg, &client_id, &callback_url(&base, cfg.name), &state);

    let cookie = Cookie::build((STATE_COOKIE, state))
        .path("/")
        .http_only(true)
        .secure(crate::handlers::auth::secure_cookies())
        .same_site(SameSite::Lax)
        .max_age(Duration::minutes(STATE_TTL_MINUTES))
        .build();

    (jar.add(cookie), Redirect::to(&location)).into_response()
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// GET /auth/oauth/{provider}/callback?code&state
pub async fn callback(
    State(app): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Query(params): Query<CallbackParams>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Response {
    let Some(cfg) = resolve_provider(&provider) else {
        return bad_request("unknown_provider");
    };
    let Some((client_id, client_secret)) = provider_credentials(cfg.name) else {
        return bad_request("oauth_not_configured");
    };

    if let Some(err) = params.error.as_deref() {
        warn!(provider = %cfg.name, error = %err, "oauth: provider returned error");
        return Redirect::to("/login?oauth=denied").into_response();
    }
    let (Some(code), Some(returned_state)) = (params.code, params.state) else {
        return bad_request("missing_code_or_state");
    };

    if !state_matches(jar.get(STATE_COOKIE).map(Cookie::value), &returned_state) {
        warn!(provider = %cfg.name, "oauth: state mismatch — rejecting callback");
        return bad_request("state_mismatch");
    }

    let redirect_uri = callback_url(&base_url(), cfg.name);
    let client = match reqwest::Client::builder()
        .user_agent("sentori-server")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "oauth: http client build failed");
            return server_error("http_client_failed");
        }
    };

    let Some(access_token) = exchange_code(
        &client,
        &cfg,
        &code,
        &redirect_uri,
        &client_id,
        &client_secret,
    )
    .await
    else {
        return server_error("token_exchange_failed");
    };

    let Some(info) = fetch_user_info(&client, &cfg, &access_token).await else {
        return bad_request("email_unverified_or_missing");
    };

    let user_id = match link_or_create_user(&app.pool, cfg.name, &info).await {
        Ok(id) => id,
        Err(e) => {
            error!(provider = %cfg.name, error = %e, "oauth: account resolution failed");
            return server_error("account_link_failed");
        }
    };

    issue_session(&app, user_id, &headers, jar).await
}

/// POST the authorization code to the provider's token endpoint.
///
/// Returns `None` on any failure. Nothing here logs the code, the
/// secret, the token, or a response body that could contain them.
async fn exchange_code(
    client: &reqwest::Client,
    cfg: &ProviderConfig,
    code: &str,
    redirect_uri: &str,
    client_id: &str,
    client_secret: &str,
) -> Option<String> {
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];
    let resp = match client
        .post(cfg.token_url)
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!(provider = %cfg.name, error = %e, "oauth: token request failed");
            return None;
        }
    };
    if !resp.status().is_success() {
        // The body is deliberately not logged: providers echo the
        // submitted client_secret back in some error shapes.
        warn!(provider = %cfg.name, status = %resp.status(), "oauth: token exchange non-2xx");
        return None;
    }
    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            error!(provider = %cfg.name, error = %e, "oauth: token response parse failed");
            return None;
        }
    };
    let token = body.get("access_token").and_then(Value::as_str);
    if token.is_none() {
        warn!(provider = %cfg.name, "oauth: no access_token in token response");
    }
    token.map(ToString::to_string)
}

/// Fetch the provider's userinfo and reduce it to [`UserInfo`].
///
/// For GitHub this also fetches `/user/emails` and folds the verified
/// primary address in — `/user` alone cannot supply one this flow is
/// allowed to trust.
async fn fetch_user_info(
    client: &reqwest::Client,
    cfg: &ProviderConfig,
    access_token: &str,
) -> Option<UserInfo> {
    let mut body: Value = match client
        .get(cfg.user_info_url)
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(v) => v,
            Err(e) => {
                error!(provider = %cfg.name, error = %e, "oauth: userinfo parse failed");
                return None;
            }
        },
        Err(e) => {
            error!(provider = %cfg.name, error = %e, "oauth: userinfo fetch failed");
            return None;
        }
    };

    if cfg.name == "github" {
        let emails: Vec<Value> = match client
            .get("https://api.github.com/user/emails")
            .bearer_auth(access_token)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(r) => r.json().await.unwrap_or_default(),
            Err(e) => {
                error!(error = %e, "oauth: github /user/emails fetch failed");
                Vec::new()
            }
        };
        if let Some(email) = primary_verified_email(&emails) {
            body["email"] = Value::String(email);
            body["email_verified"] = Value::Bool(true);
        } else {
            warn!("oauth: github account has no verified primary email");
            // Overwrite rather than merely omit: /user supplies its own
            // unverified `email`, and leaving that in place would let
            // the extractor read it.
            body["email"] = Value::Null;
            body["email_verified"] = Value::Bool(false);
        }
    }

    let info = (cfg.extract_user)(&body);
    if info.is_none() {
        warn!(provider = %cfg.name, "oauth: userinfo missing fields or email unverified");
    }
    info
}

/// Resolve the provider identity to a Sentori user, creating and
/// linking as needed. See the module docs for the ordering rationale.
async fn link_or_create_user(
    pool: &PgPool,
    provider: &str,
    info: &UserInfo,
) -> anyhow::Result<UserId> {
    // 1. The authoritative match: this exact upstream account.
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM user_oauth_identities WHERE provider = $1 AND subject = $2",
    )
    .bind(provider)
    .bind(&info.subject)
    .fetch_optional(pool)
    .await?;

    if let Some((user_id,)) = existing {
        sqlx::query(
            "UPDATE user_oauth_identities \
             SET display_name = COALESCE($1, display_name), \
                 avatar_url   = COALESCE($2, avatar_url), \
                 last_login_at = now() \
             WHERE provider = $3 AND subject = $4",
        )
        .bind(info.display_name.as_deref())
        .bind(info.avatar_url.as_deref())
        .bind(provider)
        .bind(&info.subject)
        .execute(pool)
        .await?;
        return Ok(UserId::from_uuid(user_id));
    }

    // New users land in the self-hosted default workspace — the same
    // one bootstrap::ensure_first_owner() creates and that main.rs
    // scopes AppState to.
    let identity = Identity::new(pool.clone(), crate::bootstrap::default_workspace_id());

    // 2. Bridge by verified email onto an existing account. Reached
    //    only for a first link, since step 1 owns every later login.
    if let Some(user) = identity.users().find_by_email(&info.email).await? {
        insert_identity(pool, user.id, provider, info).await?;
        info!(user_id = %user.id, %provider, "oauth.link_existing");
        return Ok(user.id);
    }

    // 3. First sight of this person — create the account.
    //
    // The password hash is a sentinel, not a hash: argon2 verify
    // cannot parse it, so `/auth/login` can never succeed against
    // this row no matter what is posted to it.
    let user = identity
        .users()
        .create(&info.email, &format!("oauth:{provider}:no-password"))
        .await?;
    identity.members().add(user.id, Role::User, None).await?;
    // The provider vouched for the address, so there is nothing for
    // an emailed verify link to establish.
    identity.users().mark_email_verified(user.id).await?;
    insert_identity(pool, user.id, provider, info).await?;
    info!(user_id = %user.id, %provider, "oauth.register");
    Ok(user.id)
}

async fn insert_identity(
    pool: &PgPool,
    user_id: UserId,
    provider: &str,
    info: &UserInfo,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO user_oauth_identities \
         (id, user_id, provider, subject, display_name, avatar_url, last_login_at) \
         VALUES ($1, $2, $3, $4, $5, $6, now())",
    )
    .bind(Uuid::now_v7())
    .bind(user_id.into_uuid())
    .bind(provider)
    .bind(&info.subject)
    .bind(info.display_name.as_deref())
    .bind(info.avatar_url.as_deref())
    .execute(pool)
    .await
    // UNIQUE (user_id, provider) trips when the account already has a
    // different upstream subject for this provider. Refusing is the
    // right answer: silently repointing the link would let whoever
    // controls the second upstream account take over the first.
    .map_err(|e| anyhow::anyhow!("link {provider} identity: {e}"))?;
    Ok(())
}

/// Mint a session and hand back the redirect, with the same cookie
/// `handlers::auth::login` sets — same name, same flags, same signed
/// value — so both login paths are indistinguishable downstream.
async fn issue_session(
    app: &Arc<AppState>,
    user_id: UserId,
    headers: &HeaderMap,
    jar: CookieJar,
) -> Response {
    let auth_svc = crate::handlers::auth::auth(app);
    let meta = RequestMeta {
        ip: crate::client_ip::client_ip(headers),
        user_agent: headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string),
    };
    let expires_at = OffsetDateTime::now_utc() + auth_svc.opts().session_ttl;
    let minted = match auth_svc.sessions().create(user_id, expires_at, &meta).await {
        Ok(m) => m,
        Err(e) => {
            error!(error = %e, "oauth: session mint failed");
            return server_error("session_failed");
        }
    };
    let signed = sentori_cookie_session::SignedCookie::seal(
        auth_svc.cookie_key(),
        minted.session_id.to_wire_string().as_bytes(),
    );
    info!(%user_id, "auth.login.oauth");

    // The state cookie has done its job; clearing it keeps a stale
    // value from being replayed against a later callback.
    let cleared_state = Cookie::build((STATE_COOKIE, ""))
        .path("/")
        .max_age(Duration::seconds(0))
        .build();

    let mut resp = (jar.add(cleared_state), Redirect::to(DASHBOARD_ROOT)).into_response();
    if let Ok(hv) =
        axum::http::HeaderValue::from_str(&crate::handlers::auth::session_cookie_header(&signed))
    {
        resp.headers_mut().append(SET_COOKIE, hv);
    }
    resp
}

fn bad_request(error: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
}

fn server_error(error: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": error })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    //! Everything reachable without a network or a database: which
    //! providers exist, what the authorize URL carries, how each
    //! provider's userinfo JSON reduces, and that state comparison
    //! rejects what it must.

    use super::{
        ProviderConfig, UserInfo, authorize_url, callback_url, extract_github, extract_google,
        primary_verified_email, resolve_provider, state_matches,
    };
    use serde_json::{Value, json};

    // The crate denies unwrap and warns on expect, so tests unwrap
    // through these instead.
    fn cfg(name: &str) -> ProviderConfig {
        match resolve_provider(name) {
            Some(c) => c,
            None => unreachable!("{name} must resolve"),
        }
    }

    fn array(v: &Value) -> &[Value] {
        match v.as_array() {
            Some(a) => a.as_slice(),
            None => unreachable!("test fixture must be an array"),
        }
    }

    fn extracted(v: &Value) -> UserInfo {
        match extract_github(v) {
            Some(u) => u,
            None => unreachable!("fixture must extract"),
        }
    }

    #[test]
    fn only_github_and_google_resolve() {
        assert!(resolve_provider("github").is_some());
        assert!(resolve_provider("google").is_some());
        for unknown in ["", "facebook", "GitHub", "github "] {
            assert!(
                resolve_provider(unknown).is_none(),
                "{unknown} must not resolve"
            );
        }
    }

    #[test]
    fn authorize_url_carries_state_and_callback() {
        let cfg = cfg("github");
        let url = authorize_url(
            &cfg,
            "client-abc",
            "https://sentori.example/auth/oauth/github/callback",
            "state-xyz",
        );
        assert!(url.starts_with("https://github.com/login/oauth/authorize?"));
        // The state parameter is what ties the callback back to this
        // browser; its absence would silently disable the CSRF check.
        assert!(url.contains("state=state-xyz"), "{url}");
        assert!(url.contains("client_id=client-abc"), "{url}");
        assert!(url.contains("response_type=code"), "{url}");
        // Reserved characters in scope must survive encoding.
        assert!(url.contains("scope=read%3Auser+user%3Aemail"), "{url}");
        assert!(
            url.contains(
                "redirect_uri=https%3A%2F%2Fsentori.example%2Fauth%2Foauth%2Fgithub%2Fcallback"
            ),
            "{url}"
        );
    }

    #[test]
    fn google_authorize_url_asks_for_account_selection() {
        let cfg = cfg("google");
        let url = authorize_url(&cfg, "id", "https://x/cb", "s");
        assert!(url.contains("prompt=select_account"), "{url}");
    }

    #[test]
    fn callback_url_does_not_double_slash() {
        assert_eq!(
            callback_url("https://sentori.example/", "google"),
            "https://sentori.example/auth/oauth/google/callback"
        );
        assert_eq!(
            callback_url("https://sentori.example", "google"),
            "https://sentori.example/auth/oauth/google/callback"
        );
    }

    #[test]
    fn google_userinfo_extracts() {
        let v = json!({
            "sub": "1029384756",
            "email": "Ops@Example.COM",
            "email_verified": true,
            "name": "Ops Person",
            "picture": "https://lh3.example/a.jpg",
        });
        assert_eq!(
            extract_google(&v),
            Some(UserInfo {
                subject: "1029384756".to_string(),
                // Normalised, or the same person matches two rows.
                email: "ops@example.com".to_string(),
                display_name: Some("Ops Person".to_string()),
                avatar_url: Some("https://lh3.example/a.jpg".to_string()),
            })
        );
    }

    #[test]
    fn google_unverified_email_is_refused() {
        // Google will hand out an unverified address for some
        // account types; linking on it would let anyone who can set
        // that address claim the matching Sentori user.
        for claim in [json!(false), json!(null), json!("true")] {
            let v = json!({ "sub": "1", "email": "a@b.com", "email_verified": claim });
            assert_eq!(extract_google(&v), None, "claim {claim} must be refused");
        }
        let missing = json!({ "sub": "1", "email": "a@b.com" });
        assert_eq!(extract_google(&missing), None);
    }

    #[test]
    fn github_userinfo_extracts_once_email_is_folded_in() {
        // Shape as it reaches the extractor: /user plus the verified
        // primary from /user/emails.
        let v = json!({
            "id": 583_211,
            "login": "octocat",
            "name": "Mona Lisa",
            "avatar_url": "https://avatars.example/u/1",
            "email": "mona@example.com",
            "email_verified": true,
        });
        assert_eq!(
            extract_github(&v),
            Some(UserInfo {
                subject: "583211".to_string(),
                email: "mona@example.com".to_string(),
                display_name: Some("Mona Lisa".to_string()),
                avatar_url: Some("https://avatars.example/u/1".to_string()),
            })
        );
    }

    #[test]
    fn github_falls_back_to_login_when_name_is_absent_or_blank() {
        for name in [json!(null), json!("")] {
            let v = json!({
                "id": 1, "login": "octocat", "name": name,
                "email": "a@b.com", "email_verified": true,
            });
            let got = extracted(&v);
            assert_eq!(got.display_name.as_deref(), Some("octocat"));
        }
    }

    #[test]
    fn github_profile_email_alone_is_not_enough() {
        // The raw /user payload — an email with no verification
        // marker. Accepting it is the account-takeover path, since
        // GitHub never confirms the public profile address.
        let raw = json!({
            "id": 1,
            "login": "octocat",
            "email": "victim@example.com",
        });
        assert_eq!(extract_github(&raw), None);
    }

    #[test]
    fn github_primary_email_requires_both_flags() {
        let emails = json!([
            { "email": "old@example.com",  "primary": false, "verified": true },
            { "email": "claimed@example.com", "primary": true, "verified": false },
        ]);
        let list = array(&emails);
        assert_eq!(primary_verified_email(list), None);

        let emails = json!([
            { "email": "old@example.com", "primary": false, "verified": true },
            { "email": "Real@Example.com", "primary": true, "verified": true },
        ]);
        let list = array(&emails);
        assert_eq!(
            primary_verified_email(list),
            Some("real@example.com".to_string())
        );
        assert_eq!(primary_verified_email(&[]), None);
    }

    #[test]
    fn state_must_match_the_cookie() {
        assert!(state_matches(Some("abc123"), "abc123"));
    }

    #[test]
    fn state_mismatch_and_absence_are_rejected() {
        // A missing cookie is the shape a cross-site forged callback
        // takes, so it must fail exactly like a wrong value does.
        assert!(!state_matches(None, "abc123"));
        assert!(!state_matches(Some(""), ""));
        assert!(!state_matches(Some(""), "abc123"));
        assert!(!state_matches(Some("abc123"), "abc124"));
        assert!(!state_matches(Some("abc123"), "ABC123"));
        assert!(!state_matches(Some("abc123"), ""));
    }

    #[test]
    fn state_tokens_are_random_and_long() {
        let a = super::random_state();
        let b = super::random_state();
        assert_eq!(a.len(), 64, "256 bits, hex encoded");
        assert_ne!(a, b);
    }
}
