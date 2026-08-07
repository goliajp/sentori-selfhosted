//! Issue email notifications — the first channel subscribed to the
//! issue system (design.md §9): a new issue or a regression mails
//! the people responsible for the project.
//!
//! Recipients = the owner plus every admin assigned to the project,
//! minus per-user opt-outs in `notification_prefs` (no row = both
//! kinds on, so a fresh install with SMTP configured notifies
//! without further setup).
//!
//! Everything here is fire-and-forget from the ingest path: the
//! ingest response never waits on SMTP, and every failure degrades
//! to a WARN log. Dedup rides `delivery_log.dedup_key` (one mail
//! per issue+occasion+recipient), so an event burst producing the
//! same new issue cannot double-send.

use sentori_notifier::{Channel, Notification, NotifierService};
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use crate::state::AppState;

/// Spawn the notification task for one ingest outcome. Synchronous
/// and instant on the caller's path.
pub fn spawn_issue_notification(
    state: &AppState,
    project_id: Uuid,
    issue_id: Uuid,
    is_new_issue: bool,
    regressed: bool,
) {
    if !(is_new_issue || regressed) {
        return;
    }
    let Some(transport) = state.mailer.transport() else {
        // No SMTP — the channel is simply absent (design.md §9:
        // channels are optional subscribers, never load-bearing).
        return;
    };
    let pool = state.pool.clone();
    let base_url = state.mailer.base_url().to_string();
    tokio::spawn(async move {
        let mut service = NotifierService::new(pool.clone());
        service.register(transport);
        if let Err(e) = notify(
            &pool,
            &service,
            &base_url,
            project_id,
            issue_id,
            is_new_issue,
        )
        .await
        {
            warn!(%issue_id, error = %e, "issue notification failed");
        }
    });
}

#[derive(sqlx::FromRow)]
struct IssueRow {
    kind: String,
    group_title: String,
    message_sample: String,
    users_count: i64,
    event_count: i64,
    last_release: String,
    last_environment: String,
    regressed_at: Option<time::OffsetDateTime>,
    resolved_in_release: Option<String>,
}

async fn notify(
    pool: &PgPool,
    service: &NotifierService,
    base_url: &str,
    project_id: Uuid,
    issue_id: Uuid,
    is_new_issue: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let issue: IssueRow = sqlx::query_as(
        "SELECT kind, group_title, message_sample, users_count, event_count, \
                last_release, last_environment, regressed_at, resolved_in_release \
         FROM issues WHERE id = $1",
    )
    .bind(issue_id)
    .fetch_one(pool)
    .await?;

    let project_name: String = sqlx::query_scalar("SELECT name FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await?;

    // Owner + assigned admins, minus the opt-outs for this occasion.
    let pref_col = if is_new_issue {
        "on_new_issue"
    } else {
        "on_regression"
    };
    let recipients: Vec<(Uuid, String)> = sqlx::query_as(&format!(
        "SELECT DISTINCT u.id, u.email FROM users u \
         LEFT JOIN project_assignments pa \
                ON pa.user_id = u.id AND pa.project_id = $1 \
         LEFT JOIN notification_prefs np \
                ON np.user_id = u.id AND np.project_id = $1 \
         WHERE (u.role = 'superadmin' OR pa.user_id IS NOT NULL) \
           AND COALESCE(np.{pref_col}, TRUE)"
    ))
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    if recipients.is_empty() {
        return Ok(());
    }

    let occasion = if is_new_issue { "New" } else { "Regression" };
    let subject = format!(
        "[sentori/{project_name}] {occasion}: {} — {}",
        issue.kind, issue.group_title
    );
    let link = format!("{base_url}/issues/{issue_id}");

    let message_line = if issue.message_sample.is_empty() {
        String::new()
    } else {
        format!("  {}\n", issue.message_sample)
    };
    let reopened_line = match (&issue.resolved_in_release, is_new_issue) {
        (Some(fixed), false) => {
            format!("  was resolved in {fixed} — this recurrence reopened it\n")
        }
        _ => String::new(),
    };
    let body = format!(
        "{occasion} {kind} in {project_name}\n\n  {title}\n{message_line}\n  \
         impact: {users} user(s) · {events} event(s)\n  \
         seen in: {release} ({environment})\n{reopened_line}\n\
         Open the issue:\n\n  {link}\n",
        kind = issue.kind,
        title = issue.group_title,
        users = issue.users_count,
        events = issue.event_count,
        release = issue.last_release,
        environment = issue.last_environment,
    );

    // One logical mail per issue+occasion+recipient. Regressions
    // key on the regression timestamp so a later, separate
    // regression notifies again.
    let occasion_key = if is_new_issue {
        "new".to_string()
    } else {
        issue.regressed_at.map_or_else(
            || "regressed".to_string(),
            |t| format!("regressed-{}", t.unix_timestamp()),
        )
    };

    for (user_id, email) in recipients {
        let n = Notification::new(Channel::Email, email.clone(), subject.clone(), body.clone())
            .with_project(project_id)
            .with_dedup_key(format!("issue-{issue_id}-{occasion_key}-{user_id}"));
        if let Err(e) = service.dispatch(&n).await {
            warn!(%issue_id, email, error = %e, "issue mail dispatch failed");
        }
    }
    Ok(())
}
