//! GET /v1/projects/:project_id/issues — K5 issue list.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use sentori_workspace_identity::ProjectId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize, Default)]
pub struct ListQuery {
    /// Optional status filter — `active` / `resolved` /
    /// `regressed` / `ignored`.
    pub status: Option<String>,
    /// Max rows (default 100, max 500).
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct IssueRow {
    pub id: Uuid,
    pub fingerprint: String,
    pub error_type: String,
    pub message_sample: String,
    pub kind: String,
    pub status: String,
    pub event_count: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub first_seen: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_seen: OffsetDateTime,
    pub last_release: String,
    pub last_environment: String,
}

/// Positional shape of the `issues` list query: id, fingerprint,
/// error_type, message_sample, kind, status, event_count,
/// first_seen, last_seen, last_release, last_environment.
type IssueListRow = (
    Uuid,
    String,
    String,
    String,
    String,
    String,
    i64,
    OffsetDateTime,
    OffsetDateTime,
    String,
    String,
);

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<IssueRow>>, (StatusCode, String)> {
    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let _pid = ProjectId::from_uuid(project_id);
    let limit = q.limit.unwrap_or(100).min(500);

    // The two branches carry different parameter counts, so each
    // builds its own bind chain rather than sharing one with a
    // placeholder for the absent status filter.
    let rows: Vec<IssueListRow> = if let Some(status) = q.status.as_deref() {
        sqlx::query_as(
            "SELECT id, fingerprint, error_type, message_sample, kind, status,
                    event_count, first_seen, last_seen, last_release, last_environment
             FROM issues
             WHERE project_id = $1 AND workspace_id = $2 AND status = $3
             ORDER BY last_seen DESC LIMIT $4",
        )
        .bind(project_id)
        .bind(ctx.workspace_id.into_uuid())
        .bind(status)
        .bind(i64::from(limit))
        .fetch_all(&state.pool)
        .await
    } else {
        sqlx::query_as(
            "SELECT id, fingerprint, error_type, message_sample, kind, status,
                    event_count, first_seen, last_seen, last_release, last_environment
             FROM issues
             WHERE project_id = $1 AND workspace_id = $2
             ORDER BY last_seen DESC LIMIT $3",
        )
        .bind(project_id)
        .bind(ctx.workspace_id.into_uuid())
        .bind(i64::from(limit))
        .fetch_all(&state.pool)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    fingerprint,
                    error_type,
                    message_sample,
                    kind,
                    status,
                    event_count,
                    first_seen,
                    last_seen,
                    last_release,
                    last_environment,
                )| IssueRow {
                    id,
                    fingerprint,
                    error_type,
                    message_sample,
                    kind,
                    status,
                    event_count,
                    first_seen,
                    last_seen,
                    last_release,
                    last_environment,
                },
            )
            .collect(),
    ))
}

/// PATCH /v1/projects/:project_id/issues/:issue_id
///
/// Body: `{ status?: "active" | "resolved" | "regressed" | "ignored",
///           resolved_in_release?: string }`
pub async fn patch(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path((_project_id, issue_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<PatchBody>,
) -> Result<StatusCode, (StatusCode, String)> {
    use sentori_event_pipeline::IssueStatus;
    use sentori_issue_store::IssuePatch;

    // The mutation goes through IssueStore, which takes no
    // workspace argument, so the guard is the only scoping layer
    // here — it addresses exactly the id being written.
    super::tenant::guard_issue(&state, ctx.workspace_id, issue_id).await?;

    let status = match body.status.as_deref() {
        None => None,
        Some("active") => Some(IssueStatus::Active),
        Some("resolved") => Some(IssueStatus::Resolved),
        Some("regressed") => Some(IssueStatus::Regressed),
        Some("ignored") => Some(IssueStatus::Ignored),
        Some(other) => {
            return Err((StatusCode::BAD_REQUEST, format!("invalid status: {other}")));
        }
    };
    // Reject an out-of-range priority here rather than letting the
    // CHECK constraint turn it into a 500. The set is closed and the
    // caller is a UI that knows it.
    if let Some(p) = body.priority.as_deref()
        && sentori_event_pipeline::IssuePriority::from_db_str(p).is_err()
    {
        return Err((StatusCode::BAD_REQUEST, format!("invalid priority: {p}")));
    }
    let patch = IssuePatch {
        status,
        assignee_user_id: body
            .assignee_user_id
            .into_patch()
            .map(|o| o.map(sentori_workspace_identity::UserId::from_uuid)),
        priority: body.priority.clone(),
        labels: body.labels.clone(),
        resolved_in_release: body.resolved_in_release,
    };
    state
        .issues
        .patch(ctx.workspace_id, issue_id, patch, OffsetDateTime::now_utc())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Some(status_label) = body.status.as_deref() {
        crate::notify::notify_issue_watchers(
            &state.pool,
            issue_id,
            None,
            "issue_status",
            serde_json::json!({
                "issue_id": issue_id.to_string(),
                "status": status_label,
            }),
        )
        .await;
        crate::notify::audit(
            &state.pool,
            ctx.workspace_id.into_uuid(),
            None,
            None,
            "issue.status",
            Some("issue"),
            Some(&issue_id.to_string()),
            serde_json::json!({ "status": status_label }),
        )
        .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, Default)]
pub struct PatchBody {
    pub status: Option<String>,
    pub resolved_in_release: Option<String>,
    /// `p0`–`p3`.
    pub priority: Option<String>,
    /// Replaces the set — the caller sends the labels it wants the
    /// issue to end up with, not a delta.
    pub labels: Option<Vec<String>>,
    /// Absent leaves the assignee alone, `null` clears it, a uuid
    /// assigns.
    #[serde(default)]
    pub assignee_user_id: FieldPatch<Uuid>,
}

/// What a PATCH says about a nullable field.
///
/// Three states, because "leave it alone" and "set it to nobody" are
/// different requests and a plain `Option` can only carry two. Absent
/// from the body is [`Self::Leave`]; `null` is [`Self::Clear`].
#[derive(Debug, Clone, Copy, Default)]
pub enum FieldPatch<T> {
    /// The key was absent.
    #[default]
    Leave,
    /// The key was present. `None` means the caller sent `null`.
    Set(Option<T>),
}

// Deserialized by hand rather than `#[serde(untagged)]`: untagged tries
// each variant in order and a unit variant accepts `null`, so `null`
// landed on `Leave` and "unassign" silently became "leave as is". Here
// the field's presence is decided by `#[serde(default)]` on the struct
// field, and anything that reaches this impl was present.
impl<'de, T: Deserialize<'de>> Deserialize<'de> for FieldPatch<T> {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(de).map(Self::Set)
    }
}

impl<T> FieldPatch<T> {
    /// Collapse to the shape `IssuePatch` takes.
    ///
    /// The nested option is that type's existing contract, so this is
    /// the one place it has to be spelled — the lint is right that it
    /// is unreadable, which is why `FieldPatch` exists everywhere else.
    #[allow(clippy::option_option)]
    fn into_patch(self) -> Option<Option<T>> {
        match self {
            Self::Leave => None,
            Self::Set(v) => Some(v),
        }
    }
}

/// POST /v1/projects/:project_id/issues/_bulk_patch
/// Body: { ids: [uuid…], status?: "resolved" | ... }
pub async fn bulk_patch(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<BulkPatchBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use sentori_event_pipeline::IssueStatus;
    use sentori_issue_store::IssuePatch;

    super::tenant::guard_project(&state, ctx.workspace_id, project_id).await?;

    let status = match body.status.as_deref() {
        None => None,
        Some("active") => Some(IssueStatus::Active),
        Some("resolved") => Some(IssueStatus::Resolved),
        Some("regressed") => Some(IssueStatus::Regressed),
        Some("ignored") => Some(IssueStatus::Ignored),
        Some(other) => {
            return Err((StatusCode::BAD_REQUEST, format!("invalid status: {other}")));
        }
    };
    // Reject an out-of-range priority here rather than letting the
    // CHECK constraint turn it into a 500. The set is closed and the
    // caller is a UI that knows it.
    if let Some(p) = body.priority.as_deref()
        && sentori_event_pipeline::IssuePriority::from_db_str(p).is_err()
    {
        return Err((StatusCode::BAD_REQUEST, format!("invalid priority: {p}")));
    }
    let patch = IssuePatch {
        status,
        assignee_user_id: body
            .assignee_user_id
            .into_patch()
            .map(|o| o.map(sentori_workspace_identity::UserId::from_uuid)),
        priority: body.priority.clone(),
        labels: body.labels.clone(),
        resolved_in_release: body.resolved_in_release,
    };
    // Unlike the other handlers the ids here come from the body,
    // not the path, so guarding project_id says nothing about
    // them. IssueStore::bulk_patch takes no workspace argument, so
    // narrow the id set to the caller's workspace before handing it
    // over. Ids outside it drop out and simply don't count toward
    // `updated` — the same silent no-op the store already applies
    // to ids that don't exist.
    let ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM issues WHERE id = ANY($1) AND workspace_id = $2")
            .bind(&body.ids)
            .bind(ctx.workspace_id.into_uuid())
            .fetch_all(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let outcome = state
        .issues
        .bulk_patch(ctx.workspace_id, &ids, patch, OffsetDateTime::now_utc())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Some(status_label) = body.status.as_deref() {
        crate::notify::audit(
            &state.pool,
            ctx.workspace_id.into_uuid(),
            None,
            None,
            "issue.bulk_status",
            Some("issue"),
            None,
            serde_json::json!({
                "status": status_label,
                "count": body.ids.len(),
                "updated": outcome.updated,
            }),
        )
        .await;
    }
    Ok(Json(serde_json::json!({
        "updated": outcome.updated,
    })))
}

#[derive(Deserialize, Default)]
pub struct BulkPatchBody {
    pub ids: Vec<Uuid>,
    pub status: Option<String>,
    pub resolved_in_release: Option<String>,
    /// `p0`–`p3`. Triaging a morning's worth of new issues to the same
    /// priority is the reason this endpoint takes a list at all.
    pub priority: Option<String>,
    /// Replaces the set on every id.
    pub labels: Option<Vec<String>>,
    /// Absent leaves alone, `null` unassigns, a uuid assigns.
    #[serde(default)]
    pub assignee_user_id: FieldPatch<Uuid>,
}

/// GET /v1/projects/:project_id/issues/:issue_id
pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<crate::session_mw::SessionContext>,
    Path((_project_id, issue_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use sqlx::Row;

    super::tenant::guard_issue(&state, ctx.workspace_id, issue_id).await?;

    let row = sqlx::query(
        "SELECT id, project_id, fingerprint, error_type, message_sample, kind, status, \
                event_count, first_seen, last_seen, last_release, last_environment, \
                regressed_at, regressed_in_release, resolved_at \
         FROM issues WHERE id = $1 AND workspace_id = $2",
    )
    .bind(issue_id)
    .bind(ctx.workspace_id.into_uuid())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "issue_not_found".to_string()))?;
    let _ = ProjectId::from_uuid;
    Ok(Json(serde_json::json!({
        "id": row.get::<Uuid, _>("id").to_string(),
        "project_id": row.get::<Uuid, _>("project_id").to_string(),
        "fingerprint": row.get::<String, _>("fingerprint"),
        "error_type": row.get::<String, _>("error_type"),
        "message_sample": row.try_get::<String, _>("message_sample").unwrap_or_default(),
        "kind": row.get::<String, _>("kind"),
        "status": row.get::<String, _>("status"),
        "event_count": row.get::<i64, _>("event_count"),
        "first_seen": crate::wire_time::rfc3339(row.get::<OffsetDateTime, _>("first_seen")),
        "last_seen": crate::wire_time::rfc3339(row.get::<OffsetDateTime, _>("last_seen")),
        "last_release": row.get::<String, _>("last_release"),
        "last_environment": row.get::<String, _>("last_environment"),
        "regressed_at": crate::wire_time::rfc3339_opt(row.try_get::<Option<OffsetDateTime>, _>("regressed_at").ok().flatten()),
        "regressed_in_release": row.try_get::<Option<String>, _>("regressed_in_release").ok().flatten(),
        "resolved_at": crate::wire_time::rfc3339_opt(row.try_get::<Option<OffsetDateTime>, _>("resolved_at").ok().flatten()),
    })))
}

#[cfg(test)]
// A fixture that will not parse is a broken test, not a runtime path;
// failing loudly is the whole point here.
#[allow(clippy::expect_used)]
mod patch_wire_tests {
    use super::*;

    fn body(json: &str) -> PatchBody {
        serde_json::from_str(json).expect("valid PatchBody")
    }

    /// Absent, null and a value are three different requests. Losing
    /// this distinction means "unassign" silently becomes "leave as is",
    /// which looks like the button not working.
    #[test]
    fn assignee_absent_leaves_it_alone() {
        assert!(body(r"{}").assignee_user_id.into_patch().is_none());
    }

    #[test]
    fn assignee_null_clears_it() {
        let p = body(r#"{"assignee_user_id":null}"#)
            .assignee_user_id
            .into_patch();
        assert_eq!(p, Some(None));
    }

    #[test]
    fn assignee_uuid_assigns_it() {
        let id = "019e3589-9d7f-7013-9952-e3f287104954";
        let p = body(&format!(r#"{{"assignee_user_id":"{id}"}}"#))
            .assignee_user_id
            .into_patch();
        assert_eq!(p, Some(Some(id.parse().expect("uuid"))));
    }

    /// Labels replace rather than merge — the caller sends the set it
    /// wants, so an empty array must reach the store as an empty set
    /// and not be mistaken for "unspecified".
    #[test]
    fn empty_label_array_is_a_request_to_clear() {
        assert_eq!(body(r#"{"labels":[]}"#).labels, Some(vec![]));
        assert_eq!(body(r"{}").labels, None);
    }

    #[test]
    fn priority_round_trips_through_the_db_form() {
        for p in sentori_event_pipeline::IssuePriority::ALL {
            let parsed = sentori_event_pipeline::IssuePriority::from_db_str(p.as_db_str());
            assert_eq!(parsed.ok(), Some(p));
        }
    }
}
