//! Cloud identity is a separate trust mode, permanently bound to this database.
//! The browser never receives a runtime credential or server-side access grant.
use std::{sync::Arc, time::Duration};

use anyhow::{Context, ensure};
use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    session_mw::{Role, hash_token},
    state::AppState,
};

pub struct Cloud {
    pub workspace: Uuid,
    pub control_origin: String,
    pub runtime_origin: String,
    pub generation: i64,
    secure: bool,
    credential: String,
    endpoint: String,
    client: reqwest::Client,
}

fn origin(value: &str) -> anyhow::Result<bool> {
    let url = url::Url::parse(value)?;
    ensure!(
        url.origin().ascii_serialization() == value
            && url.username().is_empty()
            && url.password().is_none(),
        "Cloud origin must be canonical scheme://host[:port]"
    );
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && local),
        "Cloud origins require HTTPS except localhost"
    );
    Ok(url.scheme() == "https")
}

fn valid_secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn token() -> String {
    let mut value = [0u8; 32];
    rand::rng().fill_bytes(&mut value);
    hex::encode(value)
}

pub async fn bound(pool: &PgPool) -> anyhow::Result<bool> {
    let table: bool =
        sqlx::query_scalar("SELECT to_regclass('public.cloud_instance_binding') IS NOT NULL")
            .fetch_one(pool)
            .await?;
    if !table {
        return Ok(false);
    }
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_instance_binding)")
            .fetch_one(pool)
            .await?,
    )
}

pub async fn configure(pool: &PgPool) -> anyhow::Result<Option<Arc<Cloud>>> {
    let Some(workspace) = crate::env_config::env_or_file("SENTORI_CLOUD_WORKSPACE_ID") else {
        ensure!(
            !bound(pool).await?,
            "Cloud database requires Cloud configuration; refusing local authentication fallback"
        );
        return Ok(None);
    };
    let workspace = Uuid::parse_str(&workspace).context("Cloud workspace ID")?;
    let control_origin = crate::env_config::env_or_file("SENTORI_CLOUD_CONTROL_ORIGIN")
        .context("Cloud control origin required")?;
    let runtime_origin = crate::env_config::env_or_file("SENTORI_PUBLIC_ORIGIN")
        .context("Cloud runtime origin required")?;
    let control_secure = origin(&control_origin)?;
    let secure = origin(&runtime_origin)?;
    ensure!(
        control_secure == secure,
        "Cloud origin security modes must agree"
    );
    let generation: i64 = crate::env_config::env_or_file("SENTORI_CLOUD_GENERATION")
        .context("Cloud generation required")?
        .parse()?;
    ensure!(generation > 0, "invalid Cloud generation");
    let credential = crate::env_config::env_or_file("SENTORI_CLOUD_RUNTIME_KEY")
        .context("Cloud runtime credential required")?;
    ensure!(
        valid_secret(&credential),
        "invalid Cloud runtime credential"
    );
    ensure!(
        crate::credential_vault::from_env().is_some(),
        "Cloud requires SENTORI_MASTER_KEY for encrypted sessions"
    );
    let endpoint =
        if let Some(value) = crate::env_config::env_or_file("SENTORI_CLOUD_FIXTURE_CONTROL_URL") {
            let url = url::Url::parse(&value)?;
            ensure!(
                !secure
                    && url.scheme() == "http"
                    && matches!(
                        url.host_str(),
                        Some("localhost" | "127.0.0.1" | "host.docker.internal")
                    )
                    && url.origin().ascii_serialization() == value
                    && url.username().is_empty()
                    && url.password().is_none(),
                "invalid Cloud fixture transport"
            );
            value
        } else {
            control_origin.clone()
        };
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(2))
        .build()?;
    let cloud = Cloud {
        workspace,
        control_origin,
        runtime_origin,
        generation,
        secure,
        credential,
        endpoint,
        client,
    };
    bind(pool, &cloud).await?;
    Ok(Some(Arc::new(cloud)))
}

async fn bind(pool: &PgPool, cloud: &Cloud) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("LOCK TABLE cloud_instance_binding IN EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let binding: Option<(Uuid,String,String,i64,Vec<u8>)> = sqlx::query_as("SELECT workspace_id,control_origin,runtime_origin,generation,credential_hash FROM cloud_instance_binding WHERE singleton").fetch_optional(&mut *tx).await?;
    if let Some((workspace, control, runtime, generation, credential)) = binding {
        ensure!(
            workspace == cloud.workspace
                && control == cloud.control_origin
                && runtime == cloud.runtime_origin
                && generation == cloud.generation
                && credential == hash_token(&cloud.credential),
            "Cloud database binding mismatch"
        );
    } else {
        let populated: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users) OR EXISTS(SELECT 1 FROM projects)",
        )
        .fetch_one(&mut *tx)
        .await?;
        ensure!(
            !populated,
            "Cloud binding requires an empty data plane; use the migration workflow for existing data"
        );
        sqlx::query("INSERT INTO cloud_instance_binding(workspace_id,control_origin,runtime_origin,generation,credential_hash) VALUES($1,$2,$3,$4,$5)")
            .bind(cloud.workspace).bind(&cloud.control_origin).bind(&cloud.runtime_origin).bind(cloud.generation).bind(hash_token(&cloud.credential).as_slice()).execute(&mut *tx).await?;
        sqlx::query("UPDATE instance_settings SET initialized_at=now() WHERE singleton")
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub struct Error(StatusCode, &'static str);
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
fn unauthorized() -> Error {
    Error(StatusCode::UNAUTHORIZED, "cloud_session_invalid")
}
fn unavailable() -> Error {
    Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "cloud_identity_unavailable",
    )
}
impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        tracing::warn!(%error,"Cloud data-plane identity database operation failed");
        unavailable()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    subject: Uuid,
    email: String,
    display_name: String,
    role: String,
    workspace_id: Uuid,
    generation: i64,
    access_epoch: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    grant: Option<String>,
}
impl Cloud {
    fn session_name(&self) -> &'static str {
        if self.secure {
            "__Host-sentori_session"
        } else {
            "sentori_session"
        }
    }
    fn verifier_name(&self) -> &'static str {
        if self.secure {
            "__Host-sentori_cloud_verifier"
        } else {
            "sentori_cloud_verifier"
        }
    }
    pub fn session_token(&self, headers: &HeaderMap) -> Option<String> {
        cookie(headers, self.session_name())
    }
    pub fn session_cookie(&self, value: &str, ttl: i64) -> String {
        self.cookie(self.session_name(), value, "/", ttl)
    }
    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        action: &str,
        body: serde_json::Value,
    ) -> Result<T, Error> {
        let mut response = self
            .client
            .post(format!(
                "{}/cloud/internal/workspaces/{}/{action}",
                self.endpoint, self.workspace
            ))
            .bearer_auth(&self.credential)
            .json(&body)
            .send()
            .await
            .map_err(|_| unavailable())?;
        if matches!(response.status().as_u16(), 401 | 403) {
            return Err(unauthorized());
        }
        if !response.status().is_success() {
            return Err(unavailable());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            if bytes.len() + chunk.len() > 16 * 1024 {
                return Err(unavailable());
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| unavailable())
    }

    pub async fn entitlement(&self) -> Result<crate::cloud_quota::Entitlement, Error> {
        self.request("entitlement", json!({})).await
    }

    async fn identity(&self, action: &str, body: serde_json::Value) -> Result<Identity, Error> {
        let value: Identity = self.request(action, body).await?;
        if value.workspace_id != self.workspace
            || value.generation != self.generation
            || value.expires_at <= OffsetDateTime::now_utc()
            || Role::from_db_str(&value.role).is_none()
            || value.email.len() > 254
            || value.display_name.chars().count() > 80
        {
            return Err(unauthorized());
        }
        Ok(value)
    }
    fn cookie(&self, name: &str, value: &str, path: &str, ttl: i64) -> String {
        format!(
            "{name}={value}; Path={path}; HttpOnly; SameSite=Lax; Max-Age={ttl}{}",
            if self.secure { "; Secure" } else { "" }
        )
    }
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|v| {
            let (key, value) = v.trim().split_once('=')?;
            (key == name).then(|| value.to_owned())
        })
}

pub async fn start(State(state): State<Arc<AppState>>) -> Result<Response, Error> {
    let cloud = state.cloud.as_ref().ok_or_else(unauthorized)?;
    let verifier = token();
    let url = format!(
        "{}/cloud/access?workspaceId={}&challenge={}",
        cloud.control_origin,
        cloud.workspace,
        hex::encode(hash_token(&verifier))
    );
    let mut response = Redirect::to(&url).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cloud.cookie(cloud.verifier_name(), &verifier, "/", 300))
            .map_err(|_| unavailable())?,
    );
    Ok(response)
}

pub async fn auth_config(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(match &state.cloud {
        Some(cloud) => json!({"mode":"cloud","controlOrigin":cloud.control_origin}),
        None => json!({"mode":"password"}),
    })
}

/// Provisioning probes use the workspace credential, never a dashboard session.
pub async fn health(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, Error> {
    let cloud = state.cloud.as_ref().ok_or_else(unauthorized)?;
    let raw = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(unauthorized)?;
    if headers.contains_key(header::ORIGIN)
        || !sentori_cookie_session::constant_time_eq(raw.as_bytes(), cloud.credential.as_bytes())
    {
        return Err(unauthorized());
    }
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    let data = token();
    let probe = async {
        let hash = state.attachments.put(data.as_bytes()).await?;
        let bytes = state.attachments.get(&hash).await;
        state.attachments.delete(&hash).await?;
        Ok::<_, sentori_attachment_store::BlobError>(bytes? == data.as_bytes())
    };
    if !matches!(
        tokio::time::timeout(Duration::from_secs(5), probe).await,
        Ok(Ok(true))
    ) {
        return Err(unavailable());
    }
    Ok(Json(
        json!({"workspaceId":cloud.workspace,"generation":cloud.generation,"origin":cloud.runtime_origin,"serverVersion":env!("CARGO_PKG_VERSION"),"database":true,"blobStorage":true,"durableStorage":matches!(state.attachments,crate::blob_store::AttachmentStore::Fs(_))}),
    ))
}

// No third-party resources or interpolated content on the fragment landing page.
// Clear the ticket from browser history before initiating any network request.
pub async fn complete_page(State(state): State<Arc<AppState>>) -> Result<Response, Error> {
    if state.cloud.is_none() {
        return Err(unauthorized());
    }
    Ok(Html(r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Sign in to Sentori</title><p id="status">Signing in…</p><script src="/auth/cloud/complete.js"></script></html>"#).into_response())
}
pub async fn complete_script() -> Response {
    ([(header::CONTENT_TYPE,"text/javascript; charset=utf-8")],r"(async()=>{const ticket=new URLSearchParams(location.hash.slice(1)).get('ticket');history.replaceState(null,'','/auth/cloud/complete');try{if(!ticket)throw Error();const r=await fetch('/auth/cloud/complete',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({ticket})});if(!r.ok)throw Error();location.replace('/');}catch{const p=document.getElementById('status');p.textContent='Sign-in expired. ';const a=document.createElement('a');a.href='/auth/cloud/start';a.textContent='Try again';p.append(a);}})();").into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Complete {
    ticket: String,
}
#[derive(Serialize, Deserialize)]
struct SealedGrant {
    workspace: Uuid,
    generation: i64,
    session: String,
    subject: Uuid,
    grant: String,
}

pub async fn complete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Complete>,
) -> Result<Response, Error> {
    let cloud = state.cloud.as_ref().ok_or_else(unauthorized)?;
    let verifier = cookie(&headers, cloud.verifier_name()).ok_or_else(unauthorized)?;
    if !valid_secret(&body.ticket) || !valid_secret(&verifier) {
        return Err(unauthorized());
    }
    let identity = cloud
        .identity(
            "exchange",
            json!({"ticket":body.ticket,"verifier":verifier}),
        )
        .await?;
    let grant = identity
        .grant
        .as_deref()
        .filter(|v| valid_secret(v))
        .ok_or_else(unauthorized)?;
    let raw = token();
    let hash = hash_token(&raw);
    let sealed = SealedGrant {
        workspace: cloud.workspace,
        generation: cloud.generation,
        session: hex::encode(hash),
        subject: identity.subject,
        grant: grant.to_owned(),
    };
    let bytes = serde_json::to_vec(&sealed).map_err(|_| unavailable())?;
    let encrypted = state
        .vault
        .as_ref()
        .ok_or_else(unavailable)?
        .seal(&bytes)
        .map_err(|_| unavailable())?;
    let mut tx = state.pool.begin().await?;
    let user = reconcile_user(&mut tx, &identity).await?;
    sqlx::query("INSERT INTO auth_sessions(id_hash,user_id,expires_at,sealed_cloud_grant,cloud_generation) VALUES($1,$2,$3,$4,$5)")
        .bind(hash.as_slice()).bind(user).bind(identity.expires_at).bind(encrypted).bind(cloud.generation).execute(&mut *tx).await?;
    tx.commit().await?;
    let ttl = (identity.expires_at - OffsetDateTime::now_utc())
        .whole_seconds()
        .max(0);
    let mut response = Json(json!({"ok":true})).into_response();
    for value in [
        cloud.session_cookie(&raw, ttl),
        cloud.cookie(cloud.verifier_name(), "", "/", 0),
    ] {
        response.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_str(&value).map_err(|_| unavailable())?,
        );
    }
    Ok(response)
}

async fn reconcile_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    identity: &Identity,
) -> Result<Uuid, Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,4192))")
        .bind(identity.subject.to_string())
        .execute(&mut **tx)
        .await?;
    let existing: Option<(Uuid, String)> =
        sqlx::query_as("SELECT user_id,access_epoch FROM cloud_user_identities WHERE subject=$1")
            .bind(identity.subject)
            .fetch_optional(&mut **tx)
            .await?;
    let user = if let Some((id, epoch)) = existing {
        if epoch != identity.access_epoch {
            sqlx::query("DELETE FROM auth_sessions WHERE user_id=$1")
                .bind(id)
                .execute(&mut **tx)
                .await?;
            sqlx::query("DELETE FROM project_assignments WHERE user_id=$1")
                .bind(id)
                .execute(&mut **tx)
                .await?;
            sqlx::query("UPDATE cloud_user_identities SET access_epoch=$2 WHERE subject=$1")
                .bind(identity.subject)
                .bind(&identity.access_epoch)
                .execute(&mut **tx)
                .await?;
        }
        id
    } else {
        let id = Uuid::now_v7();
        // A Cloud subject is never matched to a local account by email.
        sqlx::query("INSERT INTO users(id,email,password_hash,role,display_name) VALUES($1,$2,'!cloud',$3,$4)")
            .bind(id).bind(&identity.email).bind(&identity.role).bind(&identity.display_name).execute(&mut **tx).await?;
        sqlx::query(
            "INSERT INTO cloud_user_identities(subject,user_id,access_epoch) VALUES($1,$2,$3)",
        )
        .bind(identity.subject)
        .bind(id)
        .bind(&identity.access_epoch)
        .execute(&mut **tx)
        .await?;
        id
    };
    sqlx::query(
        "UPDATE users SET email=$2,display_name=$3,role=$4,last_login_at=now() WHERE id=$1",
    )
    .bind(user)
    .bind(&identity.email)
    .bind(&identity.display_name)
    .bind(&identity.role)
    .execute(&mut **tx)
    .await?;
    Ok(user)
}

pub async fn resolve(state: &AppState, hash: &[u8; 32]) -> Result<(Uuid, Role), Error> {
    let cloud = state.cloud.as_ref().ok_or_else(unauthorized)?;
    let row: Option<(Uuid,Uuid,Vec<u8>,String)>=sqlx::query_as("SELECT s.user_id,c.subject,s.sealed_cloud_grant,c.access_epoch FROM auth_sessions s JOIN cloud_user_identities c ON c.user_id=s.user_id WHERE s.id_hash=$1 AND s.expires_at>now() AND s.cloud_generation=$2 AND s.sealed_cloud_grant IS NOT NULL")
        .bind(hash.as_slice()).bind(cloud.generation).fetch_optional(&state.pool).await?;
    let (user, subject, encrypted, epoch) = row.ok_or_else(unauthorized)?;
    let bytes = state
        .vault
        .as_ref()
        .ok_or_else(unavailable)?
        .open(&encrypted)
        .map_err(|_| unauthorized())?;
    let grant: SealedGrant = serde_json::from_slice(&bytes).map_err(|_| unauthorized())?;
    if grant.workspace != cloud.workspace
        || grant.generation != cloud.generation
        || grant.session != hex::encode(hash)
        || grant.subject != subject
    {
        return Err(unauthorized());
    }
    let identity = cloud
        .identity("introspect", json!({"grant":grant.grant}))
        .await?;
    if identity.subject != subject || identity.access_epoch != epoch {
        return Err(unauthorized());
    }
    let role = Role::from_db_str(&identity.role).ok_or_else(unauthorized)?;
    // Recheck local revocation after the network round trip.
    let present: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM auth_sessions WHERE id_hash=$1 AND expires_at>now())",
    )
    .bind(hash.as_slice())
    .fetch_one(&state.pool)
    .await?;
    if !present {
        return Err(unauthorized());
    }
    Ok((user, role))
}

pub async fn boundary(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let Some(cloud) = &state.cloud else {
        return next.run(req).await;
    };
    let path = req.uri().path();
    let dashboard = path.starts_with("/admin/api/") || path.starts_with("/auth/");
    let runtime_probe = path == "/cloud/runtime/health";
    let mutation = !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if dashboard
        && mutation
        && req
            .headers()
            .get(header::ORIGIN)
            .and_then(|h| h.to_str().ok())
            != Some(&cloud.runtime_origin)
    {
        return Error(StatusCode::FORBIDDEN, "invalid_origin").into_response();
    }
    let local_auth = matches!(
        path,
        "/auth/login" | "/auth/forgot-password" | "/auth/reset-password" | "/auth/change-password"
    );
    let local_user = mutation
        && (path == "/admin/api/users"
            || (path.starts_with("/admin/api/users/") && !path.contains("/projects/")));
    let local_instance =
        mutation && (path == "/admin/api/instance" || path.starts_with("/admin/api/instance/"));
    if local_auth || local_user || local_instance {
        return Error(StatusCode::FORBIDDEN, "managed_by_cloud").into_response();
    }
    let mut response = next.run(req).await;
    if dashboard || runtime_probe {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
            .headers_mut()
            .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    }
    response
}
