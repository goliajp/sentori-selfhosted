//! Compatible test-send endpoint; the console uses the same transactional command handler.
use super::push_commands::{self, Action, Command, Error};
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPushBody {
    pub device_token_id: Uuid,
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_body")]
    pub body: String,
    pub command_id: Option<String>,
}
fn default_title() -> String {
    "Sentori test".into()
}
fn default_body() -> String {
    "hello from sentori dashboard".into()
}
pub async fn handle(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Json(body): Json<TestPushBody>,
) -> Result<(StatusCode, Json<Value>), Error> {
    let value = push_commands::execute(
        &state,
        &ctx,
        project,
        Command {
            command_id: body
                .command_id
                .unwrap_or_else(|| Uuid::now_v7().to_string()),
            action: Action::TestSend {
                device_id: body.device_token_id,
                title: body.title,
                body: body.body,
            },
        },
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(value)))
}
