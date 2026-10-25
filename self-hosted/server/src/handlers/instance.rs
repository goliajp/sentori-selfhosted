//! Durable self-hosted instance identity, configuration and setup checks.
//! Public URL and owner credentials remain operator-managed environment
//! configuration; accepting them here would be undone by the next bootstrap.

use std::{sync::Arc, time::Duration};

use axum::{Extension, Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{session_mw::SessionContext, state::AppState};

type Reply = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn failure(status: StatusCode, code: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"error": code, "code": code})))
}

fn internal(error: &sqlx::Error) -> (StatusCode, Json<Value>) {
    tracing::warn!(%error, "instance operation failed");
    failure(StatusCode::INTERNAL_SERVER_ERROR, "internal")
}

fn owner(ctx: &SessionContext) -> Result<(), (StatusCode, Json<Value>)> {
    if ctx.role.is_superadmin() {
        Ok(())
    } else {
        Err(failure(StatusCode::FORBIDDEN, "owner_required"))
    }
}

fn public_origin() -> String {
    std::env::var("SENTORI_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

fn valid_origin(raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    (url.scheme() == "https" || url.scheme() == "http" && local)
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path() == "/"
}

#[derive(sqlx::FromRow)]
struct InstanceRow {
    id: Uuid,
    name: String,
    generation: i64,
    version: i64,
    initialized_at: Option<OffsetDateTime>,
    last_check: Option<Value>,
    checked_at: Option<OffsetDateTime>,
    operating_mode: String,
    restored_from: Option<Uuid>,
    restored_source_id: Option<Uuid>,
    restored_at: Option<OffsetDateTime>,
}

pub async fn get(State(state): State<Arc<AppState>>) -> Reply {
    let row: InstanceRow = sqlx::query_as(
        "SELECT id, name, generation, version, initialized_at, last_check, checked_at, operating_mode, restored_from, restored_source_id, restored_at FROM instance_settings WHERE singleton = true",
    ).fetch_one(&state.pool).await.map_err(|error| internal(&error))?;
    Ok(Json(json!({
        "id": row.id, "name": row.name, "generation": row.generation, "version": row.version,
        "initializedAt": row.initialized_at.map(crate::wire_time::rfc3339), "lastCheck": row.last_check,
        "checkedAt": row.checked_at.map(crate::wire_time::rfc3339), "origin": public_origin(),
        "configurationSource": {"name":"database", "origin":"environment", "owner":if state.cloud.is_some() { "cloud" } else { "environment" }},
        "serverVersion": env!("CARGO_PKG_VERSION"),
        "operatingMode": row.operating_mode, "restoredFrom": row.restored_from,
        "restoredSourceId": row.restored_source_id,
        "restoredAt": row.restored_at.map(crate::wire_time::rfc3339),
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateBody {
    name: String,
    expected_version: i64,
}

async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &SessionContext,
    id: Uuid,
    action: &str,
    payload: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO audit_logs (id, actor_user_id, action, target_type, target_id, payload,ip) VALUES ($1, $2, $3, 'instance', $4, $5,$6)")
        .bind(Uuid::now_v7()).bind(ctx.user_id).bind(action).bind(id.to_string()).bind(payload)
        .bind(sentori_request_origin::current()).execute(&mut **tx).await?;
    Ok(())
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<UpdateBody>,
) -> Reply {
    owner(&ctx)?;
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 80 || body.expected_version < 1 {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "invalid_instance_configuration",
        ));
    }
    let mut tx = state.pool.begin().await.map_err(|error| internal(&error))?;
    let row: Option<(Uuid, i64)> = sqlx::query_as(
        "UPDATE instance_settings SET name = $1, version = version + 1, last_check = NULL, checked_at = NULL, updated_at = now() WHERE singleton = true AND version = $2 RETURNING id, version",
    ).bind(name).bind(body.expected_version).fetch_optional(&mut *tx).await.map_err(|error| internal(&error))?;
    let Some((id, version)) = row else {
        return Err(failure(StatusCode::CONFLICT, "instance_changed"));
    };
    audit(
        &mut tx,
        &ctx,
        id,
        "instance.configure",
        json!({"name": name, "version": version}),
    )
    .await
    .map_err(|error| internal(&error))?;
    tx.commit().await.map_err(|error| internal(&error))?;
    get(State(state)).await
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckResult {
    version: i64,
    origin: String,
    database: bool,
    origin_valid: bool,
    #[serde(flatten)]
    storage: StorageCheck,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageCheck {
    blob_storage: bool,
    durable_storage: bool,
}

impl CheckResult {
    fn usable(&self, version: i64, origin: &str) -> bool {
        self.version == version
            && self.origin == origin
            && self.database
            && self.storage.blob_storage
            && self.storage.durable_storage
            && self.origin_valid
    }
}

async fn blob_probe(state: &AppState) -> bool {
    // Unique content ensures deleting the temporary probe cannot delete an
    // object used by an event or another simultaneous check.
    let payload = format!("sentori-storage-probe:{}", Uuid::now_v7()).into_bytes();
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let hash = state.attachments.put(&payload).await.ok()?;
        let read = state.attachments.get(&hash).await;
        let deleted = state.attachments.delete(&hash).await;
        Some(read.is_ok_and(|bytes| bytes == payload) && deleted.is_ok())
    })
    .await;
    matches!(result, Ok(Some(true)))
}

pub async fn check(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Reply {
    owner(&ctx)?;
    let (version,): (i64,) =
        sqlx::query_as("SELECT version FROM instance_settings WHERE singleton = true")
            .fetch_one(&state.pool)
            .await
            .map_err(|error| internal(&error))?;
    let origin = public_origin();
    let database = matches!(
        tokio::time::timeout(
            Duration::from_secs(5),
            sqlx::query("SELECT 1").execute(&state.pool)
        )
        .await,
        Ok(Ok(_))
    );
    let result = CheckResult {
        version,
        origin_valid: valid_origin(&origin),
        origin,
        database,
        storage: StorageCheck {
            blob_storage: blob_probe(&state).await,
            durable_storage: matches!(state.attachments, crate::blob_store::AttachmentStore::Fs(_)),
        },
    };
    let value = serde_json::to_value(&result)
        .map_err(|_| failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"))?;
    let mut tx = state.pool.begin().await.map_err(|error| internal(&error))?;
    let row: Option<(Uuid,)> = sqlx::query_as("UPDATE instance_settings SET last_check = $1, checked_at = now() WHERE singleton = true AND version = $2 RETURNING id")
        .bind(&value).bind(version).fetch_optional(&mut *tx).await.map_err(|error| internal(&error))?;
    let Some((id,)) = row else {
        return Err(failure(StatusCode::CONFLICT, "instance_changed"));
    };
    audit(&mut tx, &ctx, id, "instance.checked", value.clone())
        .await
        .map_err(|error| internal(&error))?;
    tx.commit().await.map_err(|error| internal(&error))?;
    Ok(Json(value))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitializeBody {
    expected_version: i64,
}

pub async fn initialize(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<InitializeBody>,
) -> Reply {
    owner(&ctx)?;
    let mut tx = state.pool.begin().await.map_err(|error| internal(&error))?;
    let (id, version, initialized, checked_at, check): (Uuid, i64, Option<OffsetDateTime>, Option<OffsetDateTime>, Option<Value>) = sqlx::query_as(
        "SELECT id, version, initialized_at, checked_at, last_check FROM instance_settings WHERE singleton = true FOR UPDATE",
    ).fetch_one(&mut *tx).await.map_err(|error| internal(&error))?;
    if version != body.expected_version {
        return Err(failure(StatusCode::CONFLICT, "instance_changed"));
    }
    if initialized.is_none() {
        let fresh = checked_at.is_some_and(|at| {
            let age = OffsetDateTime::now_utc() - at;
            age >= time::Duration::ZERO && age <= time::Duration::minutes(10)
        });
        let valid = check
            .and_then(|v| serde_json::from_value::<CheckResult>(v).ok())
            .is_some_and(|result| result.usable(version, &public_origin()));
        if !fresh || !valid {
            return Err(failure(StatusCode::CONFLICT, "environment_check_required"));
        }
        sqlx::query("UPDATE instance_settings SET initialized_at = now(), updated_at = now() WHERE singleton = true")
            .execute(&mut *tx).await.map_err(|error| internal(&error))?;
        audit(
            &mut tx,
            &ctx,
            id,
            "instance.initialized",
            json!({"version": version}),
        )
        .await
        .map_err(|error| internal(&error))?;
    }
    tx.commit().await.map_err(|error| internal(&error))?;
    get(State(state)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_cannot_embed_credentials_paths_or_insecure_remote_hosts() {
        for valid in [
            "https://sentori.example.com",
            "http://localhost:8080",
            "http://127.0.0.1",
            "http://[::1]:8080",
        ] {
            assert!(valid_origin(valid), "{valid}");
        }
        for invalid in [
            "http://remote.example.com",
            "https://user:pass@example.com",
            "https://example.com/path",
            "https://example.com?key=x",
            "file:///tmp",
            "https://example.com#x",
        ] {
            assert!(!valid_origin(invalid), "{invalid}");
        }
    }

    #[test]
    fn initialization_proof_is_bound_to_configuration_and_real_durable_storage() {
        let mut proof = CheckResult {
            version: 2,
            origin: "https://example.com".into(),
            database: true,
            storage: StorageCheck {
                blob_storage: true,
                durable_storage: true,
            },
            origin_valid: true,
        };
        assert!(proof.usable(2, "https://example.com"));
        assert!(!proof.usable(3, "https://example.com"));
        assert!(!proof.usable(2, "https://changed.example.com"));
        proof.storage.durable_storage = false;
        assert!(!proof.usable(2, "https://example.com"));
    }
}
