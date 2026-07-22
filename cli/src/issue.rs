// Phase 29 sub-E: `sentori-cli issue list / resolve / silence`.
//
// Thin wrappers around the admin API surface that issue.tsx already
// uses from the dashboard. The CLI's value-add is being scriptable from
// CI / release-cut hooks — e.g. on tag push, mark every "regressed in
// previous release" issue resolved-in-release for the new tag.
//
// All three commands resolve the admin token and API base via the same
// env-fallback chain as `upload dsym` / `upload mapping`.

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{Value, json};

use crate::urlencoding;

/// Resolve the admin token with an explicit env lookup so tests don't
/// have to mutate global env state (which races with parallel tests).
pub(crate) fn resolve_token_with(
    cli: Option<String>,
    env_lookup: impl Fn(&str) -> Option<String>,
) -> Result<String> {
    cli.or_else(|| env_lookup("SENTORI_ADMIN_TOKEN"))
        .or_else(|| env_lookup("SENTORI_TOKEN"))
        .context("token: pass --token or set SENTORI_ADMIN_TOKEN / SENTORI_TOKEN")
}

pub(crate) fn resolve_token(cli: Option<String>) -> Result<String> {
    resolve_token_with(cli, |k| std::env::var(k).ok())
}

pub(crate) fn resolve_api_url(cli: Option<String>) -> String {
    cli.or_else(|| std::env::var("SENTORI_ADMIN_URL").ok())
        .or_else(|| {
            std::env::var("SENTORI_INGEST_URL")
                .ok()
                .map(|s| s.replace("ingest.", "api."))
        })
        .unwrap_or_else(|| "https://sentori.golia.jp".to_string())
}

/// Render an issue list in a dense one-line-per-issue format matching
/// the dashboard table column order: short id, status, errorType,
/// eventCount, lastSeen.
pub(crate) fn format_issues_table(issues: &[Value]) -> String {
    if issues.is_empty() {
        return "No issues.".to_string();
    }
    let mut out = String::new();
    for i in issues {
        let id = i
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("????????????");
        let short = id.get(..12).unwrap_or(id);
        let status = i.get("status").and_then(Value::as_str).unwrap_or("?");
        let kind = i.get("errorType").and_then(Value::as_str).unwrap_or("?");
        let count = i.get("eventCount").and_then(Value::as_u64).unwrap_or(0);
        let last = i.get("lastSeen").and_then(Value::as_str).unwrap_or("?");
        out.push_str(&format!(
            "{short}  {status:<10}  {kind:<32}  events={count:<5}  last={last}\n"
        ));
    }
    out
}

pub async fn list(
    project_id: String,
    status: String,
    release: Option<String>,
    limit: u32,
    json_out: bool,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let token = resolve_token(token)?;
    let base = resolve_api_url(api_url);

    let mut url = format!(
        "{}/admin/api/projects/{}/issues?limit={}",
        base.trim_end_matches('/'),
        project_id,
        limit
    );
    if status != "all" {
        url.push_str(&format!("&status={}", urlencoding(&status)));
    }
    if let Some(r) = release.as_deref() {
        url.push_str(&format!("&release={}", urlencoding(r)));
    }

    let resp = Client::new().get(&url).bearer_auth(&token).send().await?;
    let status_code = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status_code.is_success() {
        anyhow::bail!("list failed: {status_code} {body}");
    }

    if json_out {
        println!("{body}");
    } else {
        let issues: Vec<Value> = serde_json::from_str(&body).context("parsing issue list body")?;
        print!("{}", format_issues_table(&issues));
    }
    Ok(())
}

pub async fn resolve(
    issue_id: String,
    project_id: String,
    in_release: Option<String>,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let token = resolve_token(token)?;
    let base = resolve_api_url(api_url);

    let mut body = json!({ "status": "resolved" });
    if let Some(r) = in_release {
        body["resolvedInRelease"] = Value::String(r);
    }

    let url = format!(
        "{}/admin/api/projects/{}/issues/{}",
        base.trim_end_matches('/'),
        project_id,
        issue_id
    );
    let resp = Client::new()
        .patch(&url)
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await?;
    let status_code = resp.status();
    let resp_body = resp.text().await.unwrap_or_default();
    if !status_code.is_success() {
        anyhow::bail!("resolve failed: {status_code} {resp_body}");
    }
    println!("OK ({status_code}): issue {issue_id} resolved");
    Ok(())
}

pub async fn silence(
    issue_id: String,
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let token = resolve_token(token)?;
    let base = resolve_api_url(api_url);

    let url = format!(
        "{}/admin/api/projects/{}/issues/{}",
        base.trim_end_matches('/'),
        project_id,
        issue_id
    );
    let resp = Client::new()
        .patch(&url)
        .bearer_auth(&token)
        .json(&json!({ "status": "silenced" }))
        .send()
        .await?;
    let status_code = resp.status();
    let resp_body = resp.text().await.unwrap_or_default();
    if !status_code.is_success() {
        anyhow::bail!("silence failed: {status_code} {resp_body}");
    }
    println!("OK ({status_code}): issue {issue_id} silenced");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    use crate::{Cli, Command, IssueKind};

    #[test]
    fn parses_three_issue_subcommands() {
        // list
        let cli = Cli::try_parse_from([
            "sentori-cli",
            "issue",
            "list",
            "--project",
            "019508a0-0000-0000-0000-000000000000",
            "--status",
            "active",
            "--release",
            "myapp@1.2.3+456",
            "--limit",
            "5",
            "--json",
        ])
        .expect("list parses");
        match cli.command {
            Command::Issue {
                kind:
                    IssueKind::List {
                        project_id,
                        status,
                        release,
                        limit,
                        json,
                        ..
                    },
            } => {
                assert_eq!(project_id, "019508a0-0000-0000-0000-000000000000");
                assert_eq!(status, "active");
                assert_eq!(release.as_deref(), Some("myapp@1.2.3+456"));
                assert_eq!(limit, 5);
                assert!(json);
            }
            _ => panic!("expected Issue::List"),
        }

        // resolve
        let cli = Cli::try_parse_from([
            "sentori-cli",
            "issue",
            "resolve",
            "01923456-7890-7000-8000-aabbccddeeff",
            "--project",
            "019508a0-0000-0000-0000-000000000000",
            "--in-release",
            "myapp@1.2.4+457",
        ])
        .expect("resolve parses");
        match cli.command {
            Command::Issue {
                kind:
                    IssueKind::Resolve {
                        issue_id,
                        project_id,
                        in_release,
                        ..
                    },
            } => {
                assert_eq!(issue_id, "01923456-7890-7000-8000-aabbccddeeff");
                assert_eq!(project_id, "019508a0-0000-0000-0000-000000000000");
                assert_eq!(in_release.as_deref(), Some("myapp@1.2.4+457"));
            }
            _ => panic!("expected Issue::Resolve"),
        }

        // silence
        let cli = Cli::try_parse_from([
            "sentori-cli",
            "issue",
            "silence",
            "01923456-7890-7000-8000-aabbccddeeff",
            "--project",
            "019508a0-0000-0000-0000-000000000000",
        ])
        .expect("silence parses");
        match cli.command {
            Command::Issue {
                kind: IssueKind::Silence { issue_id, .. },
            } => {
                assert_eq!(issue_id, "01923456-7890-7000-8000-aabbccddeeff");
            }
            _ => panic!("expected Issue::Silence"),
        }
    }

    #[test]
    fn format_table_renders_columns_and_empty_message() {
        // empty
        assert_eq!(format_issues_table(&[]), "No issues.");

        // one row exercising every column
        let rows = vec![serde_json::json!({
            "id": "01923456-7890-7000-8000-aabbccddeeff",
            "status": "active",
            "errorType": "TypeError",
            "eventCount": 42,
            "lastSeen": "2026-05-11T03:14:15Z",
        })];
        let out = format_issues_table(&rows);
        // Short id is first 12 chars of the dashed UUID form.
        assert!(out.starts_with("01923456-789"), "short id leads: {out}");
        assert!(out.contains("active"));
        assert!(out.contains("TypeError"));
        assert!(out.contains("events=42"));
        assert!(out.contains("2026-05-11T03:14:15Z"));
    }

    #[test]
    fn missing_token_error_mentions_env_vars() {
        // Use the with_env variant so we don't mutate process env (would
        // race with other parallel tests reading env).
        let err = resolve_token_with(None, |_| None).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("SENTORI_ADMIN_TOKEN") && msg.contains("SENTORI_TOKEN"),
            "error message must name both env vars: {msg}"
        );

        // Explicit token wins; env miss is irrelevant.
        let tok =
            resolve_token_with(Some("explicit".into()), |_| None).expect("explicit token wins");
        assert_eq!(tok, "explicit");

        // Env fallback works when CLI flag is absent.
        let tok = resolve_token_with(None, |k| {
            if k == "SENTORI_ADMIN_TOKEN" {
                Some("from-env".into())
            } else {
                None
            }
        })
        .expect("env fallback");
        assert_eq!(tok, "from-env");
    }
}
