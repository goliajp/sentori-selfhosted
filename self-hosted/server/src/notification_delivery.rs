//! Durable outbound attempts. Ambiguous non-idempotent sends are never blindly repeated.
use crate::{integrations::Settings, state::AppState};
use sentori_notifier::{Channel, Notification, Notifier};
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
#[derive(sqlx::FromRow)]
struct Claim {
    id: Uuid,
    project_id: Uuid,
    integration_id: Option<Uuid>,
    user_id: Option<Uuid>,
    destination: String,
    config_version: i64,
    attempt: i32,
    event_kind: String,
    payload: Value,
    generation: i64,
    actor_user_id: Option<Uuid>,
    target_integration_id: Option<Uuid>,
}
struct Target {
    kind: String,
    settings: Settings,
}
struct Outcome {
    state: &'static str,
    error: Option<&'static str>,
    status: Option<i32>,
    external: Option<String>,
}
impl Outcome {
    fn failure(state: &'static str, error: &'static str) -> Self {
        Self {
            state,
            error: Some(error),
            status: None,
            external: None,
        }
    }
    fn delivered() -> Self {
        Self {
            state: "delivered",
            error: None,
            status: None,
            external: None,
        }
    }
}

async fn target(state: &AppState, row: &Claim) -> Result<Result<Target, Outcome>, sqlx::Error> {
    let project: Uuid = row.project_id;
    let current: i64 =
        sqlx::query_scalar("SELECT generation FROM instance_settings WHERE singleton")
            .fetch_one(&state.pool)
            .await?;
    if current != row.generation {
        return Ok(Err(Outcome::failure("cancelled", "instance_changed")));
    }
    if row.target_integration_id.is_some() {
        let owner: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND role='superadmin')",
        )
        .bind(row.actor_user_id)
        .fetch_one(&state.pool)
        .await?;
        if !owner {
            return Ok(Err(Outcome::failure("cancelled", "permission_revoked")));
        }
    }
    let destination: String = row.destination.clone();
    if destination == "user_email" {
        let recipient:Option<String>=sqlx::query_scalar("SELECT u.email FROM users u LEFT JOIN notification_prefs p ON p.user_id=u.id AND p.project_id=$2 WHERE u.id=$1 AND (u.role='superadmin' OR EXISTS(SELECT 1 FROM project_assignments a WHERE a.user_id=u.id AND a.project_id=$2)) AND CASE WHEN $3='issue.new' THEN COALESCE(p.on_new_issue,true) ELSE COALESCE(p.on_regression,true) END")
            .bind(row.user_id).bind(project).bind(row.event_kind.clone()).fetch_optional(&state.pool).await?;
        return Ok(recipient
            .map(|recipient| Target {
                kind: "email".into(),
                settings: Settings {
                    recipient,
                    ..Settings::default()
                },
            })
            .ok_or_else(|| Outcome::failure("cancelled", "subscription_revoked")));
    }
    let Some(vault) = &state.vault else {
        return Ok(Err(Outcome::failure("failed", "vault_unconfigured")));
    };
    let config = if destination == "legacy_webhook" {
        sqlx::query("SELECT 'legacy_webhook'::text AS kind,webhook_secret_blob AS secret_blob FROM projects WHERE id=$1 AND version=$2 AND webhook_secret_blob IS NOT NULL")
            .bind(project).bind(row.config_version).fetch_optional(&state.pool).await?
    } else {
        sqlx::query("SELECT kind,secret_blob FROM outbound_integrations WHERE id=$1 AND project_id=$2 AND version=$3 AND enabled")
            .bind(row.integration_id).bind(project).bind(row.config_version).fetch_optional(&state.pool).await?
    };
    let Some(config) = config else {
        return Ok(Err(Outcome::failure("cancelled", "integration_changed")));
    };
    let kind: String = config.get("kind");
    Ok(crate::integrations::open(
        vault,
        project,
        &kind,
        &config.get::<Vec<u8>, _>("secret_blob"),
    )
    .map(|settings| Target { kind, settings })
    .map_err(|code| Outcome::failure("failed", code)))
}

async fn send(state: &AppState, row: &Claim, target: &Target) -> Outcome {
    let payload: Value = row.payload.clone();
    let id: Uuid = row.id;
    if target.kind == "email" {
        let Some(transport) = state.mailer.transport() else {
            return Outcome::failure("failed", "smtp_unconfigured");
        };
        let body = format!(
            "{}\n\n{}{}\n\nSentori delivery: {id}",
            payload["body"].as_str().unwrap_or(""),
            state.mailer.base_url(),
            payload["route"].as_str().unwrap_or("/")
        );
        let notification = Notification::new(
            Channel::Email,
            target.settings.recipient.clone(),
            payload["title"].as_str().unwrap_or("Sentori notification"),
            body,
        )
        .with_project(row.project_id);
        return match tokio::time::timeout(Duration::from_secs(15), transport.send(&notification))
            .await
        {
            Ok(Ok(())) => Outcome::delivered(),
            // SMTP may have accepted DATA before a connection failure. The UI exposes uncertainty.
            Ok(Err(_)) => Outcome::failure("uncertain", "smtp_delivery_uncertain"),
            Err(_) => Outcome::failure("uncertain", "timeout"),
        };
    }
    let prepared = match crate::integrations::prepare(
        &target.kind,
        &target.settings,
        id,
        &payload,
        state.mailer.base_url(),
    ) {
        Ok(prepared) => prepared,
        Err(code) => return Outcome::failure("failed", code),
    };
    let retryable = matches!(target.kind.as_str(), "webhook" | "legacy_webhook");
    match crate::public_http::post(
        &prepared.endpoint,
        prepared.headers,
        prepared.body,
        matches!(target.kind.as_str(), "github" | "jira"),
    )
    .await
    {
        Ok(response) if (200..300).contains(&response.status) => {
            let external =
                crate::integrations::external_url(&target.kind, &target.settings, &response.body);
            if matches!(target.kind.as_str(), "github" | "jira") && external.is_none() {
                return Outcome::failure("uncertain", "invalid_provider_response");
            }
            Outcome {
                state: "delivered",
                error: None,
                status: Some(i32::from(response.status)),
                external,
            }
        }
        Ok(response) => {
            let transient = response.status == 429 || response.status >= 500;
            let status = if retryable && transient {
                "retry_wait"
            } else if response.status >= 500 {
                "uncertain"
            } else {
                "failed"
            };
            Outcome {
                state: status,
                error: Some(if (300..400).contains(&response.status) {
                    "redirect_refused"
                } else {
                    "http_rejected"
                }),
                status: Some(i32::from(response.status)),
                external: None,
            }
        }
        Err(failure) => Outcome::failure(
            if retryable && failure.may_have_sent {
                "retry_wait"
            } else if failure.may_have_sent {
                "uncertain"
            } else {
                "failed"
            },
            failure.code,
        ),
    }
}

async fn cycle(state: &AppState) -> Result<bool, sqlx::Error> {
    // An expired send could already have reached a non-idempotent provider. Retain that
    // uncertainty across restarts instead of turning lease recovery into duplicate tickets.
    sqlx::query("UPDATE notification_deliveries SET state='uncertain',error_code='worker_interrupted',lease_owner=NULL,lease_until=NULL,finished_at=now() WHERE id IN (SELECT id FROM notification_deliveries WHERE state='running' AND lease_until<now() LIMIT 32 FOR UPDATE SKIP LOCKED)").execute(&state.pool).await?;
    let token = Uuid::now_v7();
    let row=sqlx::query_as::<_,Claim>("WITH ready AS (SELECT id FROM notification_deliveries WHERE state IN ('queued','retry_wait') AND attempt<5 AND next_attempt_at<=now() ORDER BY next_attempt_at,id LIMIT 1 FOR UPDATE SKIP LOCKED), claimed AS (UPDATE notification_deliveries d SET state='running',attempt=attempt+1,lease_owner=$1,lease_until=now()+interval '30 seconds' FROM ready WHERE d.id=ready.id RETURNING d.*) SELECT d.*,e.kind AS event_kind,e.payload,e.generation,e.actor_user_id,e.target_integration_id FROM claimed d JOIN notification_events e ON e.id=d.event_id")
        .bind(token).fetch_optional(&state.pool).await?;
    let Some(row) = row else { return Ok(false) };
    let (target, mut outcome) = match target(state, &row).await? {
        Ok(target) => {
            let result = send(state, &row, &target).await;
            (Some(target), result)
        }
        Err(result) => (None, result),
    };
    if outcome.state == "retry_wait" && row.attempt >= 5 {
        outcome.state = "uncertain";
        outcome.error = Some("retry_exhausted");
    }
    complete(state, &row, token, &outcome, target.as_ref()).await?;
    Ok(true)
}
async fn complete(
    state: &AppState,
    row: &Claim,
    token: Uuid,
    outcome: &Outcome,
    target: Option<&Target>,
) -> Result<(), sqlx::Error> {
    let id: Uuid = row.id;
    let mut tx = state.pool.begin().await?;
    let changed=sqlx::query("UPDATE notification_deliveries SET state=$3,error_code=$4,http_status=$5,external_url=$6,lease_owner=NULL,lease_until=NULL,next_attempt_at=now()+LEAST(300,attempt*attempt*5)*interval '1 second',finished_at=CASE WHEN $3='retry_wait' THEN NULL ELSE now() END WHERE id=$1 AND state='running' AND lease_owner=$2 AND lease_until>now()")
        .bind(id).bind(token).bind(outcome.state).bind(outcome.error).bind(outcome.status).bind(&outcome.external).execute(&mut *tx).await?;
    if changed.rows_affected() == 1
        && let Some(target) = target
    {
        let payload: Value = row.payload.clone();
        // The legacy SMTP status surface reads this table. Log only stable codes, never
        // reqwest errors (which can contain webhook credentials) or provider response bodies.
        sqlx::query("INSERT INTO delivery_log(id,project_id,channel,recipient,subject,metadata,status,retries,error,dedup_key,sent_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,CASE WHEN $7='delivered' THEN now() ELSE NULL END)")
            .bind(Uuid::now_v7()).bind(row.project_id).bind(if target.kind=="email"{"email"}else{"webhook"})
            .bind(if target.kind=="email"{target.settings.recipient.as_str()}else{target.kind.as_str()})
            .bind(payload["title"].as_str().unwrap_or("Sentori notification")).bind(json!({"deliveryId":id,"outcome":outcome.state}))
            .bind(if outcome.state=="delivered"{"delivered"}else{"failed"}).bind(row.attempt-1).bind(outcome.error).bind(format!("notification:{id}:{}",row.attempt)).execute(&mut *tx).await?;
    }
    tx.commit().await
}
pub fn spawn(state: &Arc<AppState>) {
    let router = state.clone();
    tokio::spawn(async move {
        loop {
            if let Err(error) = crate::notification_queue::migrate_legacy(&router).await {
                tracing::warn!(%error,"webhook credential migration failed");
            }
            match crate::notification_queue::route(&router).await {
                Ok(true) => {}
                Ok(false) => tokio::time::sleep(Duration::from_millis(500)).await,
                Err(error) => {
                    tracing::warn!(%error,"notification routing failed");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });
    for _ in 0..4 {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                match cycle(&state).await {
                    Ok(true) => {}
                    Ok(false) => tokio::time::sleep(Duration::from_millis(500)).await,
                    Err(error) => {
                        tracing::warn!(%error,"notification dispatch failed");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        });
    }
}
