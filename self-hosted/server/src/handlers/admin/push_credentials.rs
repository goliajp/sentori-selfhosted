//! Vendor credentials, staged rather than swapped.
//!
//! ```text
//! POST   …/push/credentials              add one (never replaces)
//! GET    …/push/credentials              list, with what we know of each
//! POST   …/push/credentials/{id}/probe   ask the vendor
//! POST   …/push/credentials/{id}/activate  make it the one that sends
//! DELETE …/push/credentials/{id}
//! ```
//!
//! ## Why adding one no longer replaces one
//!
//! This used to be an upsert on `(project_id, kind)`: pasting a key
//! destroyed the working one in the statement that saved the new one.
//! Both ways of holding it wrong are invisible from the file — an App
//! Store Connect `.p8` is the same shape as an APNs `.p8`, and
//! `google-services.json` reads a lot like a service account — so the
//! usual sequence was: paste, save, and find out that night.
//!
//! Now a new credential lands beside the working one, inert. It gets
//! asked of Apple or Google. Someone promotes it. Until they do, the
//! send path does not know it exists.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
};

use crate::push_credential_probe::Verdict;
use crate::session_mw::SessionContext;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

const PROVIDERS: [&str; 5] = ["apns", "fcm", "webpush", "hcm", "mipush"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBody {
    /// Provider: `apns` / `fcm` / `webpush` / `hcm` / `mipush`.
    pub provider: String,
    /// Vendor config — APNs key id + team id + topic, WebPush VAPID
    /// public key, and so on. Stored as JSONB.
    pub config: Value,
    /// Secret material: the PEM, or the service-account JSON.
    pub secret: Option<String>,
    /// What the operator calls it. Two Apple teams, or the old key
    /// kept through a rotation, are otherwise two identical rows.
    pub label: Option<String>,
}

/// Add a credential. Never replaces one.
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    _headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> (StatusCode, Json<Value>) {
    if !PROVIDERS.contains(&body.provider.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_provider" })),
        );
    }

    // The local check, still first: it is free, it is certain, and a
    // credential that cannot sign here will not sign anywhere.
    let mut config = body.config.clone();
    let secret = body.secret.clone().unwrap_or_default();
    if let Err(r) = crate::push_credential_check::check(&body.provider, &mut config, &secret) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_credential",
                "code": r.code,
                "field": r.field,
                "detail": r.detail,
            })),
        );
    }

    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }

    // The first credential of its kind takes over immediately —
    // there is nothing to protect, and making someone add and then
    // promote to get started is ceremony. Every one after that is
    // staged.
    let has_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM push_credentials \
         WHERE project_id = $1 AND kind = $2 AND active)",
    )
    .bind(project_id)
    .bind(&body.provider)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    let id = Uuid::now_v7();
    let result = sqlx::query(
        "INSERT INTO push_credentials \
         (id, project_id, kind, config, secret_blob, active, label) \
         SELECT $1, $2, $3, $4, $5, $6, $7 FROM projects p WHERE p.id = $2 \
         RETURNING id",
    )
    .bind(id)
    .bind(project_id)
    .bind(&body.provider)
    .bind(&config)
    .bind(secret.into_bytes())
    .bind(!has_active)
    .bind(body.label.as_deref())
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(_)) => {
            info!(%project_id, provider = %body.provider, staged = has_active,
                  "admin.push_credentials created");
            crate::audit::record(
                &state.pool,
                Some(project_id),
                ctx.user_id,
                "push_credentials.create",
                "push_credentials",
                &id.to_string(),
                json!({ "provider": body.provider, "active": !has_active }),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({
                    "id": id.to_string(),
                    "provider": body.provider,
                    // Say which of the two things just happened. The
                    // console shows a different next step for each.
                    "active": !has_active,
                })),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(e) => {
            warn!(error = %e, "admin.push_credentials create_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (axum::http::StatusCode, Json<Value>) {
    // 403, like the eleven sibling routes on this project.
    // Answering `[]` made "you may not look" indistinguishable
    // from "there are none", which is the exact question a
    // setup screen is asking.
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    // `secret_blob` is read and never returned. The local check runs
    // again over what is already stored, because a credential saved
    // before the form checked anything is still stored, and the only
    // other way anyone learns it is unusable is a notification that
    // does not arrive.
    let rows = sqlx::query(
        "SELECT id, kind, config, secret_blob, created_at, active, label, \
                last_validated_at, last_validate_status, last_validate_detail \
         FROM push_credentials WHERE project_id = $1 \
         ORDER BY kind, active DESC, created_at DESC",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            let kind = r.get::<String, _>("kind");
            let mut config = r.get::<Value, _>("config");
            let secret = String::from_utf8(r.get::<Vec<u8>, _>("secret_blob")).unwrap_or_default();
            let problem = crate::push_credential_check::check(&kind, &mut config, &secret)
                .err()
                .map(|e| json!({ "code": e.code, "field": e.field }));
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "kind": kind,
                "config": config,
                "label": r.get::<Option<String>, _>("label"),
                "active": r.get::<bool, _>("active"),
                "problem": problem,
                "created_at": crate::wire_time::rfc3339(r.get::<time::OffsetDateTime, _>("created_at")),
                "last_validated_at": crate::wire_time::rfc3339_opt(r.get::<Option<time::OffsetDateTime>, _>("last_validated_at")),
                "last_validate_status": r.get::<Option<String>, _>("last_validate_status"),
                "last_validate_detail": r.get::<Option<String>, _>("last_validate_detail"),
            })
        })
        .collect();
    (
        axum::http::StatusCode::OK,
        Json(json!({ "credentials": out })),
    )
}

/// One credential's kind, config and secret, if it is this project's.
async fn load(
    state: &AppState,
    project_id: Uuid,
    id: Uuid,
) -> Result<(String, Value, String), (StatusCode, Json<Value>)> {
    let row = sqlx::query(
        "SELECT kind, config, secret_blob FROM push_credentials \
         WHERE id = $1 AND project_id = $2",
    )
    .bind(id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        warn!(error = %e, "admin.push_credentials load_failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        )
    })?;

    let row = row.ok_or((
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "credential_not_found" })),
    ))?;

    Ok((
        row.get::<String, _>("kind"),
        row.get::<Value, _>("config"),
        String::from_utf8(row.get::<Vec<u8>, _>("secret_blob")).unwrap_or_default(),
    ))
}

fn verdict_json(v: &Verdict) -> Value {
    let (code, field, detail) = match v {
        Verdict::Ok => (None, None, None),
        Verdict::Limited {
            code,
            field,
            detail,
        }
        | Verdict::Rejected {
            code,
            field,
            detail,
        } => (Some(*code), *field, Some(detail.clone())),
        Verdict::Unreachable { detail } | Verdict::NotImplemented { detail } => {
            (None, None, Some(detail.clone()))
        }
    };
    json!({
        "status": v.status(),
        "code": code,
        "field": field,
        "detail": detail,
        // Whether the console may offer the promote button.
        "safeToActivate": v.safe_to_activate(),
    })
}

/// Ask the vendor about this credential. Delivers nothing.
pub async fn probe(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let (kind, config, secret) = match load(&state, project_id, id).await {
        Ok(v) => v,
        Err(e) => return e,
    };

    let verdict = crate::push_credential_probe::probe(&kind, &config, &secret).await;

    let detail = match &verdict {
        Verdict::Ok => None,
        Verdict::Limited { detail, .. }
        | Verdict::Rejected { detail, .. }
        | Verdict::Unreachable { detail }
        | Verdict::NotImplemented { detail } => Some(detail.clone()),
    };

    if let Err(e) = sqlx::query(
        "UPDATE push_credentials \
            SET last_validated_at = now(), last_validate_status = $3, \
                last_validate_detail = $4 \
          WHERE id = $1 AND project_id = $2",
    )
    .bind(id)
    .bind(project_id)
    .bind(verdict.status())
    .bind(detail.as_deref())
    .execute(&state.pool)
    .await
    {
        warn!(error = %e, "admin.push_credentials probe_store_failed");
    }

    (StatusCode::OK, Json(verdict_json(&verdict)))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivateBody {
    /// Promote in spite of a bad verdict.
    ///
    /// The probe mapping is new and has never been exercised against
    /// a real Apple or Google refusal, so an operator who knows more
    /// than it does needs a way past. The console only sends this
    /// from a checkbox that names the verdict being overridden.
    #[serde(default)]
    pub force: bool,
}

/// Make this the credential that sends.
pub async fn activate(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ActivateBody>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }

    let row = match sqlx::query(
        "SELECT kind, last_validate_status FROM push_credentials \
         WHERE id = $1 AND project_id = $2",
    )
    .bind(id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "credential_not_found" })),
            );
        }
        Err(e) => {
            warn!(error = %e, "admin.push_credentials activate_load_failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };
    let kind: String = row.get("kind");
    let status: Option<String> = row.get("last_validate_status");

    // Refused only for the two verdicts that mean the vendor told us
    // it will not work. `unreachable` and a provider with no probe
    // are unknowns, not failures — blocking on those would make an
    // Apple outage, or a provider we have never probed, a reason
    // nobody can rotate a key.
    let known_bad = matches!(status.as_deref(), Some("rejected" | "limited"));
    if known_bad && !body.force {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "verdict_says_no",
                "status": status,
                "detail": "the vendor refused this credential — probe it again, or force",
            })),
        );
    }

    // Two statements, one transaction. The partial unique index
    // permits one active row per (project, kind), so the old one has
    // to stand down before the new one stands up; a crash between
    // them would otherwise leave the project with none.
    let mut tx = match state.pool.begin().await {
        Ok(t) => t,
        Err(e) => {
            warn!(error = %e, "admin.push_credentials activate_tx_failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };

    let swapped = async {
        sqlx::query(
            "UPDATE push_credentials SET active = false \
             WHERE project_id = $1 AND kind = $2 AND active AND id <> $3",
        )
        .bind(project_id)
        .bind(&kind)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE push_credentials SET active = true WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    }
    .await;

    if let Err(e) = swapped {
        warn!(error = %e, "admin.push_credentials activate_failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        );
    }

    crate::audit::record(
        &state.pool,
        Some(project_id),
        ctx.user_id,
        "push_credentials.activate",
        "push_credentials",
        &id.to_string(),
        json!({ "kind": kind, "forced": body.force, "verdict": status }),
    )
    .await;
    info!(%project_id, %kind, forced = body.force, "admin.push_credentials activated");

    (StatusCode::OK, Json(json!({ "ok": true })))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }
    let result = sqlx::query("DELETE FROM push_credentials WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(id)
        .execute(&state.pool)
        .await;
    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "credential_not_found" })),
        ),
        Ok(_) => {
            crate::audit::record(
                &state.pool,
                Some(project_id),
                ctx.user_id,
                "push_credentials.delete",
                "push_credentials",
                &id.to_string(),
                json!({}),
            )
            .await;
            (StatusCode::NO_CONTENT, Json(json!({})))
        }
        Err(e) => {
            warn!(error = %e, "admin.push_credentials delete_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
}
