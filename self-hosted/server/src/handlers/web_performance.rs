//! Opt-in sampled visits. Metric revisions replace a visit, never its denominator.
use super::jobs::database_error;
use crate::{session_mw::SessionContext, state::AppState};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use time::OffsetDateTime;
use uuid::Uuid;
type Reply = (StatusCode, Json<Value>);
fn invalid() -> Reply {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error":"invalid_web_observation"})),
    )
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Measurement {
    id: String,
    value: f64,
    unit: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Visit {
    id: Uuid,
    revision: i32,
    #[serde(with = "time::serde::rfc3339")]
    started_at: OffsetDateTime,
    app_identifier: String,
    release: String,
    environment: String,
    route: String,
    device_class: String,
    navigation_type: String,
    sample_rate: f64,
    browser: String,
    metrics: BTreeMap<String, Measurement>,
    missing: BTreeMap<String, String>,
    error_count: i32,
    resource_error_count: i32,
    diagnostics: Value,
}
fn label(value: &str, max: usize) -> bool {
    value.len() <= max && !value.chars().any(char::is_control)
}
impl Visit {
    fn valid(&self, now: OffsetDateTime) -> bool {
        self.revision > 0
            && self.revision <= 1_000_000
            && !self.app_identifier.is_empty()
            && label(&self.app_identifier, 200)
            && label(&self.release, 200)
            && label(&self.environment, 100)
            && label(&self.route, 240)
            && self.route.starts_with('/')
            && !self.route.starts_with("//")
            && !self.route.contains(['?', '#', '@'])
            && matches!(self.device_class.as_str(), "mobile" | "desktop")
            && matches!(
                self.navigation_type.as_str(),
                "document" | "bfcache" | "spa"
            )
            && matches!(
                self.browser.as_str(),
                "Chrome" | "Firefox" | "Safari" | "Edge" | "Other"
            )
            && self.sample_rate.is_finite()
            && self.sample_rate > 0.0
            && self.sample_rate <= 1.0
            && (0..=1000).contains(&self.error_count)
            && (0..=1000).contains(&self.resource_error_count)
            && self.started_at <= now + time::Duration::minutes(5)
            && self.started_at >= now - time::Duration::days(1)
            && self.metrics.len() <= 6
            && self.metrics.iter().all(|(name, m)| {
                let permitted = if self.navigation_type == "spa" {
                    name == "routeReady"
                } else {
                    matches!(name.as_str(), "LCP" | "INP" | "CLS" | "FCP" | "TTFB")
                };
                permitted
                    && !m.id.is_empty()
                    && label(&m.id, 100)
                    && m.value.is_finite()
                    && m.value >= 0.0
                    && m.value <= if name == "CLS" { 1000.0 } else { 86_400_000.0 }
                    && m.unit == if name == "CLS" { "score" } else { "ms" }
            })
            && self.missing.len() <= 6
            && (self.navigation_type != "spa" || self.missing.is_empty())
            && self.missing.iter().all(|(k, v)| {
                matches!(k.as_str(), "LCP" | "INP" | "CLS" | "FCP" | "TTFB")
                    && label(v, 60)
                    && !self.metrics.contains_key(k)
            })
            && valid_diagnostics(&self.diagnostics)
    }
}
fn valid_diagnostics(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .keys()
        .any(|k| !matches!(k.as_str(), "navigation" | "resources" | "longTasks"))
    {
        return false;
    }
    if let Some(n) = object.get("navigation") {
        let Some(n) = n.as_object() else { return false };
        if n.len() > 5
            || n.iter().any(|(k, v)| {
                !matches!(
                    k.as_str(),
                    "dnsMs" | "connectMs" | "tlsMs" | "requestMs" | "responseMs"
                ) || !(v.is_null()
                    || v.as_f64()
                        .is_some_and(|x| x.is_finite() && (0.0..=86_400_000.0).contains(&x)))
            })
        {
            return false;
        }
    }
    let Some(resources) = object.get("resources").and_then(Value::as_array) else {
        return false;
    };
    if resources.len() > 10 {
        return false;
    }
    for r in resources {
        let Some(r) = r.as_object() else { return false };
        if r.len() != 5 {
            return false;
        }
        let Some(raw) = r.get("url").and_then(Value::as_str) else {
            return false;
        };
        let Ok(url) = url::Url::parse(raw) else {
            return false;
        };
        if raw.len() > 500
            || !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return false;
        }
        if !r
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|s| label(s, 30))
            || !r
                .get("durationMs")
                .and_then(Value::as_f64)
                .is_some_and(|n| (0.0..=86_400_000.0).contains(&n))
            || !r.get("timingAvailable").is_some_and(Value::is_boolean)
        {
            return false;
        }
        if !r
            .get("transferBytes")
            .is_some_and(|v| v.is_null() || v.as_u64().is_some_and(|n| n <= 1_000_000_000))
        {
            return false;
        }
    }
    let Some(tasks) = object.get("longTasks").and_then(Value::as_object) else {
        return false;
    };
    tasks.len() == 3
        && tasks.iter().all(|(k, v)| {
            matches!(k.as_str(), "count" | "totalMs" | "maxMs")
                && v.as_f64()
                    .is_some_and(|n| (0.0..=86_400_000.0).contains(&n))
        })
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    visits: Vec<Visit>,
}
pub async fn ingest(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<IngestContext>,
    Json(body): Json<Envelope>,
) -> Result<Json<Value>, Reply> {
    if body.visits.is_empty()
        || body.visits.len() > 20
        || body
            .visits
            .iter()
            .any(|v| !v.valid(OffsetDateTime::now_utc()))
    {
        return Err(invalid());
    }
    crate::cloud_quota::payload(&state, &body)
        .await
        .map_err(|e| e.reply())?;
    let mut tx = state.pool.begin().await.map_err(database_error)?;
    // One project lock covers quota and all revisions in the batch.
    sqlx::query("SELECT id FROM projects WHERE id=$1 FOR UPDATE")
        .bind(ctx.project_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(database_error)?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM web_visits WHERE project_id=$1")
        .bind(ctx.project_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(database_error)?;
    let ids: Vec<Uuid> = body.visits.iter().map(|v| v.id).collect();
    let added: i64 = sqlx::query_scalar("SELECT count(DISTINCT candidate) FROM unnest($2::uuid[]) candidate WHERE NOT EXISTS(SELECT 1 FROM web_visits WHERE project_id=$1 AND id=candidate)")
        .bind(ctx.project_id).bind(&ids).fetch_one(&mut *tx).await.map_err(database_error)?;
    if count + added > 100_000 {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":"web_visit_capacity","retentionDays":30})),
        ));
    }
    let mut accepted = 0;
    for visit in body.visits {
        let app:Option<Uuid>=sqlx::query_scalar("SELECT id FROM apps WHERE project_id=$1 AND identifier=$2 AND platform='web' AND coalesce(kind,'webapp')='webapp'").bind(ctx.project_id).bind(&visit.app_identifier).fetch_optional(&mut *tx).await.map_err(database_error)?;
        let Some(app) = app else {
            return Err((
                StatusCode::NOT_FOUND,
                Json(
                    json!({"error":"web_app_not_found","hint":"Create a webapp with the SDK's explicit identifier in this project"}),
                ),
            ));
        };
        let result=sqlx::query("INSERT INTO web_visits(project_id,app_id,id,revision,started_at,release,environment,route,device_class,navigation_type,sample_rate,browser,error_count,resource_error_count,metrics,missing,diagnostics) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17) ON CONFLICT(project_id,id) DO UPDATE SET revision=EXCLUDED.revision,error_count=EXCLUDED.error_count,resource_error_count=EXCLUDED.resource_error_count,metrics=EXCLUDED.metrics,missing=EXCLUDED.missing,diagnostics=EXCLUDED.diagnostics WHERE web_visits.revision<EXCLUDED.revision AND web_visits.app_id=EXCLUDED.app_id AND web_visits.started_at=EXCLUDED.started_at AND web_visits.sample_rate=EXCLUDED.sample_rate AND web_visits.route=EXCLUDED.route AND web_visits.release=EXCLUDED.release AND web_visits.environment=EXCLUDED.environment AND web_visits.device_class=EXCLUDED.device_class AND web_visits.navigation_type=EXCLUDED.navigation_type")
            .bind(ctx.project_id).bind(app).bind(visit.id).bind(visit.revision).bind(visit.started_at).bind(&visit.release).bind(&visit.environment).bind(&visit.route).bind(&visit.device_class).bind(&visit.navigation_type).bind(visit.sample_rate).bind(&visit.browser).bind(visit.error_count).bind(visit.resource_error_count)
            .bind(json!(visit.metrics)).bind(json!(visit.missing)).bind(visit.diagnostics).execute(&mut *tx).await.map_err(database_error)?;
        accepted += result.rows_affected();
    }
    tx.commit().await.map_err(database_error)?;
    Ok(Json(json!({"accepted":accepted,"source":"rum"})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Filter {
    app: Uuid,
    environment: Option<String>,
    release: Option<String>,
    route: Option<String>,
    device: Option<String>,
    hours: Option<i32>,
}
pub(super) async fn require_web_app(
    state: &AppState,
    project: Uuid,
    app: Uuid,
) -> Result<(), Reply> {
    let found:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM apps WHERE id=$1 AND project_id=$2 AND platform='web' AND coalesce(kind,'webapp')='webapp')").bind(app).bind(project).fetch_one(&state.pool).await.map_err(database_error)?;
    if found {
        Ok(())
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error":"web_app_not_found"})),
        ))
    }
}
async fn summary(state: &AppState, project: Uuid, filter: Filter) -> Result<Json<Value>, Reply> {
    require_web_app(state, project, filter.app).await?;
    let hours = filter.hours.unwrap_or(24);
    if !(1..=720).contains(&hours)
        || filter
            .device
            .as_deref()
            .is_some_and(|d| !matches!(d, "mobile" | "desktop"))
        || [&filter.environment, &filter.release, &filter.route]
            .iter()
            .any(|v| v.as_ref().is_some_and(|s| !label(s, 240)))
    {
        return Err(invalid());
    }
    let result:Value=sqlx::query_scalar(
        "WITH filtered AS MATERIALIZED (
          SELECT id,metrics,missing,error_count,sample_rate,device_class,navigation_type,started_at
          FROM web_visits WHERE project_id=$1 AND app_id=$2 AND started_at>now()-$3*interval '1 hour'
          AND ($4::text IS NULL OR environment=$4) AND ($5::text IS NULL OR release=$5)
          AND ($6::text IS NULL OR route=$6) AND ($7::text IS NULL OR device_class=$7)
        ), web_errors AS (
          SELECT e.issue_id,e.occurred_at,e.payload FROM events e WHERE e.project_id=$1 AND e.app_id=$2 AND e.kind='error' AND e.occurred_at>now()-$3*interval '1 hour'
          AND ($4::text IS NULL OR e.environment=$4) AND ($5::text IS NULL OR e.release=$5)
          AND ($6::text IS NULL OR e.payload#>>'{context,webVisit,route}'=$6)
          AND ($7::text IS NULL OR e.payload#>>'{context,webVisit,device}'=$7)
          ORDER BY e.occurred_at DESC,e.id DESC LIMIT 500
        ), metric_values AS (
          SELECT device_class,m.key AS name,count(*) AS n,percentile_cont(0.75) WITHIN GROUP(ORDER BY (m.value->>'value')::float8) AS p75
          FROM filtered CROSS JOIN LATERAL jsonb_each(metrics) m GROUP BY device_class,m.key
        ), coverage AS (
          SELECT m.key AS name,m.value AS reason,count(*) AS n FROM filtered CROSS JOIN LATERAL jsonb_each_text(missing) m GROUP BY m.key,m.value
        ), stats AS (
          SELECT count(*) AS visits,count(*) FILTER(WHERE navigation_type<>'spa') AS documents,
            count(*) FILTER(WHERE navigation_type<>'spa' AND error_count>0) AS errors,
            sum(1.0/sample_rate) FILTER(WHERE navigation_type<>'spa' AND error_count>0)/NULLIF(sum(1.0/sample_rate) FILTER(WHERE navigation_type<>'spa'),0) AS error_rate
          FROM filtered
        ) SELECT jsonb_build_object(
          'source','rum','appId',$2::uuid,'windowHours',$3::int,'visits',stats.visits,'documentVisits',stats.documents,
          'errorVisits',stats.errors,'estimatedErrorRate',CASE WHEN stats.documents=0 THEN NULL ELSE coalesce(stats.error_rate,0) END,
          'sampleRates',coalesce((SELECT jsonb_agg(rate) FROM (SELECT DISTINCT sample_rate AS rate FROM filtered ORDER BY sample_rate) rates),'[]'::jsonb),
          'visitsByDevice',coalesce((SELECT jsonb_object_agg(device_class,n) FROM (SELECT device_class,count(*) AS n FROM filtered GROUP BY device_class) counts),'{}'::jsonb),
          'percentilePopulation','sampled-visits','retentionDays',least(30,coalesce((SELECT (limits->>'eventRetentionDays')::int FROM cloud_entitlement WHERE singleton),30)),
          'errors',coalesce((SELECT jsonb_agg(jsonb_build_object('issueId',x.issue_id,'title',i.group_title,'occurrences',x.n,'correlatedVisits',x.visits,'lastSeen',x.at) ORDER BY x.at DESC) FROM (SELECT issue_id,count(*) AS n,count(DISTINCT payload#>>'{context,webVisit,id}') AS visits,max(occurred_at) AS at FROM web_errors GROUP BY issue_id ORDER BY at DESC LIMIT 20) x JOIN issues i ON i.id=x.issue_id AND i.project_id=$1),'[]'::jsonb),'errorEventLimit',500,
          'metrics',coalesce((SELECT jsonb_agg(jsonb_build_object('device',device_class,'metric',name,'unit',CASE WHEN name='CLS' THEN 'score' ELSE 'ms' END,'p75',p75,'samples',n,'minimumSamples',20,
            'assessment',CASE WHEN n<20 THEN 'insufficient-data' WHEN name IN ('LCP','INP','CLS') AND p75>CASE name WHEN 'LCP' THEN 2500 WHEN 'INP' THEN 200 ELSE 0.1 END THEN 'needs-attention' WHEN name IN ('LCP','INP','CLS') THEN 'good' ELSE 'observed' END) ORDER BY device_class COLLATE \"C\",name COLLATE \"C\") FROM metric_values),'[]'::jsonb),
          'coverage',coalesce((SELECT jsonb_object_agg(name,reasons) FROM (SELECT name,jsonb_object_agg(reason,n) AS reasons FROM coverage GROUP BY name) grouped),'{}'::jsonb),
          'recent',coalesce((SELECT jsonb_agg(jsonb_build_object('id',v.id,'at',v.started_at,'route',v.route,'browser',v.browser,'device',v.device_class,'navigationType',v.navigation_type,'release',v.release,'environment',v.environment,'metrics',v.metrics,'missing',v.missing,'diagnostics',v.diagnostics) ORDER BY v.started_at DESC,v.id DESC)
            FROM (SELECT id FROM filtered ORDER BY started_at DESC,id DESC LIMIT 20) recent JOIN web_visits v ON v.project_id=$1 AND v.id=recent.id),'[]'::jsonb)
        ) FROM stats"
    ).bind(project).bind(filter.app).bind(hours).bind(filter.environment).bind(filter.release).bind(filter.route).bind(filter.device).fetch_one(&state.pool).await.map_err(database_error)?;
    Ok(Json(result))
}
pub async fn api_summary(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<IngestContext>,
    Query(filter): Query<Filter>,
) -> Result<Json<Value>, Reply> {
    super::sdk::require_admin_token(&ctx)?;
    summary(&state, ctx.project_id, filter).await
}
pub async fn admin_summary(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project): Path<Uuid>,
    Query(filter): Query<Filter>,
) -> Result<Json<Value>, Reply> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project).await?;
    summary(&state, project, filter).await
}
