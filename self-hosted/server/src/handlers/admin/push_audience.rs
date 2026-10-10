//! Preview an exact device selection, then queue that reviewed selection atomically.
use super::push_commands::{self, Error, database_error, error};
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewBody {
    #[serde(default)]
    pub app_user_id: Option<String>,
    #[serde(default)]
    pub traits: Option<Value>,
    #[serde(default)]
    pub audience: Option<Value>,
}
fn parse(body: &PreviewBody) -> Result<crate::audience::Audience, Error> {
    crate::audience::from_request(
        body.app_user_id.as_deref(),
        body.traits.as_ref(),
        body.audience.as_ref(),
    )
    .map_err(|detail| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"bad_audience","detail":detail})),
        )
    })?
    .ok_or_else(|| error(StatusCode::BAD_REQUEST, "bad_audience"))
}
struct SelectedDevice {
    id: Uuid,
    sample: Value,
}
async fn selection(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    audience: &crate::audience::Audience,
) -> Result<Vec<SelectedDevice>, Error> {
    let (fragment, binds) = audience.to_sql(2);
    let text = format!(
        "SELECT dt.id,dt.provider,dt.traits,dt.metadata-'auth' AS metadata,dt.user_key IS NOT NULL AS addressable,right(dt.user_key,6) AS user_key_tail FROM device_tokens dt WHERE dt.project_id=$1 AND dt.revoked_at IS NULL AND ({fragment}) ORDER BY dt.id LIMIT 10001"
    );
    let mut query = sqlx::query(AssertSqlSafe(text)).bind(project);
    for value in &binds {
        query = value.attach(query);
    }
    let rows = query.fetch_all(&mut **tx).await.map_err(database_error)?;
    if rows.len() > 10000 {
        return Err(error(StatusCode::BAD_REQUEST, "audience_too_large"));
    }
    Ok(rows.into_iter().map(|row| SelectedDevice {
        id: row.get("id"),
        sample: json!({"id":row.get::<Uuid,_>("id"),"provider":row.get::<String,_>("provider"),"traits":row.get::<Value,_>("traits"),"metadata":row.get::<Value,_>("metadata"),"addressable":row.get::<bool,_>("addressable"),"userKeyTail":row.get::<Option<String>,_>("user_key_tail")}),
    }).collect())
}
pub async fn preview(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Json(body): Json<PreviewBody>,
) -> Result<Json<Value>, Error> {
    super::tokens::ensure_project_access(&state, &ctx, project).await?;
    let audience = parse(&body)?;
    let digest = push_commands::digest(&body)?;
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    push_commands::lock(&mut tx, project).await?;
    sqlx::query("DELETE FROM push_audience_previews WHERE id IN (SELECT id FROM push_audience_previews WHERE expires_at<now() ORDER BY expires_at LIMIT 1000)").execute(&mut *tx).await.map_err(database_error)?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM push_audience_previews WHERE project_id=$1 AND actor_user_id=$2",
    )
    .bind(project)
    .bind(ctx.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(database_error)?;
    if count >= 64 {
        return Err(error(StatusCode::TOO_MANY_REQUESTS, "preview_limit"));
    }
    let rows = selection(&mut tx, project, &audience).await?;
    let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let sample: Vec<Value> = rows.iter().take(8).map(|row| row.sample.clone()).collect();
    let id = Uuid::now_v7();
    let expires:time::OffsetDateTime=sqlx::query_scalar("INSERT INTO push_audience_previews(id,project_id,actor_user_id,digest,generation,token_ids) SELECT $1,$2,$3,$4,generation,$5 FROM instance_settings WHERE singleton RETURNING expires_at").bind(id).bind(project).bind(ctx.user_id).bind(digest).bind(&ids).fetch_one(&mut *tx).await.map_err(database_error)?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(
        json!({"matched":ids.len(),"sample":sample,"previewId":id,"expiresAt":crate::wire_time::rfc3339(expires)}),
    ))
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendBody {
    #[serde(default)]
    pub app_user_id: Option<String>,
    #[serde(default)]
    pub traits: Option<Value>,
    #[serde(default)]
    pub audience: Option<Value>,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub expected_matched: i64,
    pub idempotency_key: Option<String>,
    pub preview_id: Option<Uuid>,
}
pub async fn send(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Json(body): Json<SendBody>,
) -> Result<Json<Value>, Error> {
    super::tokens::ensure_project_access(&state, &ctx, project).await?;
    push_commands::validate_message(&body.title, &body.body)?;
    let command_hash = push_commands::digest(&json!({"action":"audience.send","body":body}))?;
    let selection_body = PreviewBody {
        app_user_id: body.app_user_id,
        traits: body.traits,
        audience: body.audience,
    };
    let audience = parse(&selection_body)?;
    let digest = push_commands::digest(&selection_body)?;
    let key = body
        .idempotency_key
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    push_commands::lock(&mut tx, project).await?;
    if let Some(mut value) =
        push_commands::prior(&mut tx, project, ctx.user_id, &key, &command_hash).await?
    {
        value["queued"] = json!(0);
        value["alreadySent"] = json!(true);
        return Ok(Json(value));
    }
    // Old consoles did not send an ID. Their latest matching, unexpired preview is
    // still an exact selection; new consoles bind the explicit receipt they displayed.
    let preview=sqlx::query("SELECT p.token_ids FROM push_audience_previews p JOIN instance_settings s ON s.singleton AND s.generation=p.generation WHERE p.project_id=$1 AND p.actor_user_id=$2 AND p.digest=$3 AND p.expires_at>now() AND ($4::uuid IS NULL OR p.id=$4) ORDER BY p.created_at DESC,p.id DESC LIMIT 1").bind(project).bind(ctx.user_id).bind(digest).bind(body.preview_id).fetch_optional(&mut *tx).await.map_err(database_error)?.ok_or_else(||error(StatusCode::CONFLICT,"preview_expired"))?;
    let reviewed: Vec<Uuid> = preview.get("token_ids");
    let current: Vec<Uuid> = selection(&mut tx, project, &audience)
        .await?
        .iter()
        .map(|row| row.id)
        .collect();
    if reviewed != current || i64::try_from(current.len()).ok() != Some(body.expected_matched) {
        return Err((
            StatusCode::CONFLICT,
            Json(
                json!({"error":"audience_changed","matched":current.len(),"expected":body.expected_matched}),
            ),
        ));
    }
    // Queue only these IDs. New registrations after the comparison cannot expand it.
    let inserted=sqlx::query("INSERT INTO push_sends(id,project_id,token_id,provider,payload,status,idempotency_key,credential_id) SELECT gen_random_uuid(),$1,dt.id,dt.provider,$2,'queued',$3,pc.id FROM device_tokens dt LEFT JOIN push_credentials pc ON pc.project_id=dt.project_id AND pc.kind=dt.provider AND pc.active WHERE dt.project_id=$1 AND dt.id=ANY($4) AND dt.revoked_at IS NULL ON CONFLICT(project_id,token_id,idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING RETURNING id").bind(project).bind(json!({"title":body.title,"body":body.body})).bind(format!("console:{}:{key}",ctx.user_id)).bind(&reviewed).fetch_all(&mut *tx).await.map_err(database_error)?;
    if inserted.len() != reviewed.len() {
        return Err(error(StatusCode::CONFLICT, "audience_changed"));
    }
    let response = json!({"queued":inserted.len(),"matched":reviewed.len(),"alreadySent":false});
    crate::audit::record_in_transaction(
        &mut tx,
        Some(project),
        ctx.user_id,
        "push.audience_send",
        "push_audience",
        &body
            .preview_id
            .map_or_else(|| project.to_string(), |id| id.to_string()),
        response.clone(),
    )
    .await
    .map_err(database_error)?;
    push_commands::record(
        &mut tx,
        project,
        ctx.user_id,
        &key,
        &command_hash,
        &response,
    )
    .await?;
    tx.commit().await.map_err(database_error)?;
    Ok(Json(response))
}
#[cfg(test)]
mod tests {
    use super::*;

    /// The same audience, compiled at two offsets, selects the same
    /// thing and binds the same values in the same order.
    ///
    /// This is what lets the count and the send be two statements
    /// rather than one renumbered one — and renumbering is what put a
    /// fragment of the condition into the notification's payload.
    #[test]
    fn two_offsets_of_one_audience_agree_on_everything_but_the_numbers() {
        let v = json!({ "all": [
            { "trait": "plan", "is": "pro" },
            { "device": "appVersion", "versionGte": "4.2" } ] });
        let parsed = crate::audience::from_request(None, None, Some(&v));
        assert!(parsed.is_ok(), "the audience did not parse");
        let Ok(Some(a)) = parsed else { return };
        let (at2, b2) = a.to_sql(2);
        let (at4, b4) = a.to_sql(4);
        assert_eq!(
            b2.len(),
            b4.len(),
            "the two offsets bound different amounts"
        );
        assert_eq!(
            format!("{b2:?}"),
            format!("{b4:?}"),
            "the two offsets bound different values, or in a different order"
        );
        let renumbered: String = at2
            .split('$')
            .enumerate()
            .map(|(i, part)| {
                if i == 0 {
                    return part.to_string();
                }
                let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
                match digits.parse::<usize>() {
                    Ok(n) => format!("${}{}", n + 2, &part[digits.len()..]),
                    Err(_) => format!("${part}"),
                }
            })
            .collect();
        assert_eq!(
            renumbered, at4,
            "the two offsets produced different clauses"
        );
    }
}
