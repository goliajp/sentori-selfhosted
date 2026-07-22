//! `project` + `token` subcommand impls — operate on the
//! `/admin/api/*` REST surface that's session-gated in v0.2.

use anyhow::{Context, Result};
use serde_json::Value;

const DEFAULT_API_URL: &str = "https://sentori.golia.jp";

fn resolve_api_url(arg: Option<String>) -> String {
    arg.or_else(|| std::env::var("SENTORI_ADMIN_URL").ok())
        .unwrap_or_else(|| DEFAULT_API_URL.to_string())
}

fn token_value(arg: Option<String>) -> Result<String> {
    arg.or_else(|| std::env::var("SENTORI_ADMIN_TOKEN").ok())
        .or_else(|| std::env::var("SENTORI_TOKEN").ok())
        .context(
            "no admin token provided — pass --token or set SENTORI_ADMIN_TOKEN / SENTORI_TOKEN",
        )
}

fn client(token: &str) -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))?,
    );
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(20))
        .build()?)
}

// ── project ────────────────────────────────────────────────

pub async fn project_list(
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!("{}/v1/projects", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Vec<Value> = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        println!("{:<38}  {:<24}  name", "id", "slug");
        for p in &body {
            println!(
                "{:<38}  {:<24}  {}",
                p["id"].as_str().unwrap_or("?"),
                p["slug"].as_str().unwrap_or("?"),
                p["name"].as_str().unwrap_or("?"),
            );
        }
    }
    Ok(())
}

pub async fn project_create(
    name: String,
    slug: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/admin/api/projects", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&url)
        .json(&serde_json::json!({ "name": name, "slug": slug }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    println!("created project {}", body["id"]);
    println!("  name: {}", body["name"]);
    println!("  slug: {}", body["slug"]);
    Ok(())
}

pub async fn project_delete(
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    c.delete(&url).send().await?.error_for_status()?;
    println!("deleted project {project_id}");
    Ok(())
}

// ── token ──────────────────────────────────────────────────

pub async fn token_list(
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/tokens",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["tokens"].as_array().cloned().unwrap_or_default();
    println!(
        "{:<38}  {:<7}  {:<6}  {:<24}  label",
        "id", "kind", "last4", "created"
    );
    for t in &rows {
        println!(
            "{:<38}  {:<7}  {:<6}  {:<24}  {}",
            t["id"].as_str().unwrap_or("?"),
            t["kind"].as_str().unwrap_or("?"),
            t["last4"].as_str().unwrap_or("—"),
            t["created_at"].as_str().unwrap_or("?"),
            t["label"].as_str().unwrap_or("—"),
        );
    }
    Ok(())
}

pub async fn token_mint(
    project_id: String,
    label: Option<String>,
    kind: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/tokens",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&url)
        .json(&serde_json::json!({ "label": label, "kind": kind }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    println!("minted token {} ({})", body["token_id"], body["kind"]);
    println!();
    println!("  {}", body["token"].as_str().unwrap_or("?"));
    println!();
    println!("This is shown ONCE. Paste into SDK init({{ token, ingestUrl }}).");
    Ok(())
}

pub async fn token_revoke(
    token_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/admin/api/tokens/{token_id}", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    c.delete(&url).send().await?.error_for_status()?;
    println!("revoked token {token_id}");
    Ok(())
}

// ── audit ──────────────────────────────────────────────────

pub async fn audit_list(
    project_id: Option<String>,
    actor: Option<String>,
    action: Option<String>,
    limit: u32,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let mut url = format!("{}/v1/audit?limit={limit}", resolve_api_url(api_url));
    if let Some(p) = project_id {
        url.push_str(&format!("&project_id={p}"));
    }
    if let Some(a) = actor {
        url.push_str(&format!("&actor_user_id={a}"));
    }
    if let Some(act) = action {
        url.push_str(&format!("&action={act}"));
    }
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Vec<Value> = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    println!(
        "{:<24}  {:<28}  {:<10}  {:<10}",
        "when", "action", "actor", "project"
    );
    for e in &body {
        println!(
            "{:<24}  {:<28}  {:<10}  {:<10}",
            e["created_at"].as_str().unwrap_or("?"),
            e["action"].as_str().unwrap_or("?"),
            e["actor_user_id"]
                .as_str()
                .map(|s| &s[..s.len().min(8)])
                .unwrap_or("system"),
            e["project_id"]
                .as_str()
                .map(|s| &s[..s.len().min(8)])
                .unwrap_or("workspace"),
        );
    }
    Ok(())
}

// ── member ─────────────────────────────────────────────────

pub async fn member_list(token: Option<String>, api_url: Option<String>, json: bool) -> Result<()> {
    let url = format!("{}/admin/api/members", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["members"].as_array().cloned().unwrap_or_default();
    println!("{:<38}  {:<8}  added", "user_id", "role");
    for m in &rows {
        println!(
            "{:<38}  {:<8}  {}",
            m["user_id"].as_str().unwrap_or("?"),
            m["role"].as_str().unwrap_or("?"),
            m["added_at"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn member_remove(
    user_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/admin/api/members/{user_id}", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    c.delete(&url).send().await?.error_for_status()?;
    println!("removed member {user_id}");
    Ok(())
}

// ── invite ─────────────────────────────────────────────────

pub async fn invite_mint(
    email: String,
    role: String,
    invited_by: String,
    expires_in_days: i64,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/admin/api/invites", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&url)
        .json(&serde_json::json!({
            "email": email,
            "role": role,
            "invited_by": invited_by,
            "expires_in_days": expires_in_days,
        }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    println!(
        "invite {} for {} (role {}, expires {})",
        body["invite_id"],
        email,
        role,
        body["expires_at"].as_str().unwrap_or("?")
    );
    println!();
    println!("  {}", body["token"].as_str().unwrap_or("?"));
    println!();
    println!("Forward this token to {email} — they paste into /auth/invites/<token>/accept.");
    Ok(())
}

// ── alert ──────────────────────────────────────────────────

pub async fn alert_list(token: Option<String>, api_url: Option<String>, json: bool) -> Result<()> {
    let url = format!("{}/v1/alerts", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Vec<Value> = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    println!(
        "{:<38}  {:<6}  {:<32}  {:<6}  trigger",
        "id", "active", "name", "throttle"
    );
    for a in &body {
        println!(
            "{:<38}  {:<6}  {:<32}  {:<6}  {}",
            a["id"].as_str().unwrap_or("?"),
            if a["enabled"].as_bool().unwrap_or(true) {
                "on"
            } else {
                "off"
            },
            a["name"].as_str().unwrap_or("?"),
            a["throttle_minutes"].as_i64().unwrap_or(0),
            a["trigger_kind"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn alert_delete(
    alert_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/v1/alerts/{alert_id}", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    c.delete(&url).send().await?.error_for_status()?;
    println!("deleted alert {alert_id}");
    Ok(())
}

pub async fn alert_show(
    alert_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/v1/alerts/{alert_id}", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

// ── saved-view ─────────────────────────────────────────────

pub async fn view_list(
    target: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/v1/saved-views?target={target}",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Vec<Value> = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    println!("{:<38}  {:<10}  {:<10}  name", "id", "target", "scope");
    for v in &body {
        println!(
            "{:<38}  {:<10}  {:<10}  {}",
            v["id"].as_str().unwrap_or("?"),
            v["target"].as_str().unwrap_or("?"),
            v["scope"].as_str().unwrap_or("?"),
            v["name"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn view_delete(
    view_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/v1/saved-views/{view_id}", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    c.delete(&url).send().await?.error_for_status()?;
    println!("deleted view {view_id}");
    Ok(())
}

// ── cert ───────────────────────────────────────────────────

pub async fn cert_list(
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/v1/projects/{project_id}/cert/observations",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Vec<Value> = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    println!("{:<40}  {:<25}  expires", "domain", "issuer");
    for o in &body {
        println!(
            "{:<40}  {:<25}  {}",
            o["domain"].as_str().unwrap_or("?"),
            o["issuer_name"]
                .as_str()
                .map(|s| &s[..s.len().min(25)])
                .unwrap_or("?"),
            o["not_after"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn cert_watch(
    project_id: String,
    domain: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/cert/watches",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    c.post(&url)
        .json(&serde_json::json!({ "domain": domain }))
        .send()
        .await?
        .error_for_status()?;
    println!("now watching {domain}");
    Ok(())
}

pub async fn cert_unwatch(
    project_id: String,
    domain: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/cert/watches/{domain}",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    c.delete(&url).send().await?.error_for_status()?;
    println!("stopped watching {domain}");
    Ok(())
}

// ── usage ──────────────────────────────────────────────────

pub async fn usage_show(token: Option<String>, api_url: Option<String>, json: bool) -> Result<()> {
    let url = format!("{}/v1/usage", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    println!(
        "plan: {}    status: {}    period: {}",
        body["plan"].as_str().unwrap_or("?"),
        body["status"].as_str().unwrap_or("?"),
        body["period_yyyymm"].as_str().unwrap_or("?"),
    );
    for key in ["events", "spans", "replays"] {
        let g = &body[key];
        let count = g["count"].as_i64().unwrap_or(0);
        let limit = g["limit"].as_i64().unwrap_or(0);
        let dropped = g["dropped"].as_i64().unwrap_or(0);
        println!(
            "  {:<8} {:>10} / {:>10}  dropped {:>6}",
            key, count, limit, dropped
        );
    }
    Ok(())
}

// ── stats ──────────────────────────────────────────────────

pub async fn trace_list(
    project_id: String,
    limit: u32,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/v1/projects/{project_id}/traces?limit={limit}",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["traces"].as_array().cloned().unwrap_or_default();
    println!(
        "{:<38}  {:<10}  {:>8}  {:>6}  name",
        "trace_id", "root_op", "spans", "ms"
    );
    for t in &rows {
        println!(
            "{:<38}  {:<10}  {:>8}  {:>6}  {}",
            t["trace_id"].as_str().unwrap_or("?"),
            t["root_op"].as_str().unwrap_or("—"),
            t["span_count"].as_i64().unwrap_or(0),
            t["duration_ms"].as_i64().unwrap_or(0),
            t["root_name"].as_str().unwrap_or(""),
        );
    }
    Ok(())
}

pub async fn replay_list(
    project_id: String,
    limit: u32,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/v1/projects/{project_id}/replays?limit={limit}",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["replays"].as_array().cloned().unwrap_or_default();
    println!("{:<38}  {:>7}  {:>7}  event", "id", "ms", "frames");
    for r in &rows {
        println!(
            "{:<38}  {:>7}  {:>7}  {}",
            r["id"].as_str().unwrap_or("?"),
            r["duration_ms"].as_i64().unwrap_or(0),
            r["frame_count"].as_i64().unwrap_or(0),
            r["event_id"]
                .as_str()
                .map(|s| &s[..s.len().min(8)])
                .unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn metric_list(
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/v1/projects/{project_id}/metrics",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["metrics"].as_array().cloned().unwrap_or_default();
    println!("{:<40}  {:>10}  {:>10}  last", "name", "24h count", "avg");
    for m in &rows {
        println!(
            "{:<40}  {:>10}  {:>10.2}  {}",
            m["name"].as_str().unwrap_or("?"),
            m["total_count"].as_i64().unwrap_or(0),
            m["avg_value"].as_f64().unwrap_or(0.0),
            m["last_bucket"].as_str().unwrap_or("—"),
        );
    }
    Ok(())
}

pub async fn comment_post(
    issue_id: String,
    body_md: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/issues/{issue_id}/comments",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&url)
        .json(&serde_json::json!({ "body_md": body_md }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    println!(
        "posted comment {} on issue {}",
        body["id"].as_str().unwrap_or("?"),
        body["issue_id"].as_str().unwrap_or("?"),
    );
    Ok(())
}

pub async fn comment_list(
    issue_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!("{}/v1/issues/{issue_id}/comments", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["comments"].as_array().cloned().unwrap_or_default();
    for c in &rows {
        println!(
            "  [{}] {}",
            c["author_user_id"]
                .as_str()
                .map(|s| &s[..s.len().min(8)])
                .unwrap_or("?"),
            c["body_md"].as_str().unwrap_or("")
        );
    }
    Ok(())
}

pub async fn alerts_list(token: Option<String>, api_url: Option<String>, json: bool) -> Result<()> {
    let url = format!("{}/v1/alerts", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["alerts"].as_array().cloned().unwrap_or_default();
    println!("{:<10}  {:<5}  {:<18}  name", "id", "on", "trigger");
    for a in &rows {
        println!(
            "{:<10}  {:<5}  {:<18}  {}",
            a["id"]
                .as_str()
                .map(|x| &x[..x.len().min(8)])
                .unwrap_or("?"),
            if a["enabled"].as_bool().unwrap_or(false) && !a["muted"].as_bool().unwrap_or(false) {
                "on"
            } else {
                "off"
            },
            a["trigger_kind"].as_str().unwrap_or("?"),
            a["name"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn alert_channel_add(
    alert_id: String,
    kind: String,
    url: String,
    secret: Option<String>,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let base = resolve_api_url(api_url);
    let c = client(&token_value(token)?)?;
    // Fetch existing channels.
    let cur: Value = c
        .get(format!("{base}/v1/alerts/{alert_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut channels = cur["channels"].as_array().cloned().unwrap_or_default();
    let mut new_ch = serde_json::Map::new();
    new_ch.insert("kind".to_string(), Value::String(kind));
    new_ch.insert("url".to_string(), Value::String(url));
    if let Some(s) = secret {
        new_ch.insert("secret".to_string(), Value::String(s));
    }
    channels.push(Value::Object(new_ch));
    c.patch(format!("{base}/v1/alerts/{alert_id}"))
        .json(&serde_json::json!({ "channels": channels }))
        .send()
        .await?
        .error_for_status()?;
    println!("added channel ({} total)", channels.len());
    Ok(())
}

pub async fn alert_fire_test(
    alert_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/v1/alerts/{alert_id}/_fire_test",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.post(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    let delivered = body["delivered"].as_i64().unwrap_or(0);
    let errors = body["errors"].as_array().cloned().unwrap_or_default();
    if errors.is_empty() {
        println!("✓ delivered to {delivered} channel(s)");
    } else {
        println!("delivered {delivered}; errors:");
        for e in errors {
            println!("  ✗ {}", e.as_str().unwrap_or("?"));
        }
    }
    Ok(())
}

pub async fn alert_patch(
    alert_id: String,
    enabled: Option<bool>,
    muted: Option<bool>,
    throttle_minutes: Option<i32>,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/v1/alerts/{alert_id}", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let mut body = serde_json::Map::new();
    if let Some(b) = enabled {
        body.insert("enabled".to_string(), serde_json::json!(b));
    }
    if let Some(b) = muted {
        body.insert("muted".to_string(), serde_json::json!(b));
    }
    if let Some(t) = throttle_minutes {
        body.insert("throttle_minutes".to_string(), serde_json::json!(t));
    }
    c.patch(&url).json(&body).send().await?.error_for_status()?;
    println!("patched {alert_id}");
    Ok(())
}

pub async fn webhook_test(
    url: String,
    secret: Option<String>,
    message: Option<String>,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let endpoint = format!("{}/admin/api/webhooks/test", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&endpoint)
        .json(&serde_json::json!({
            "url": url,
            "secret": secret,
            "message": message,
        }))
        .send()
        .await?;
    let body: Value = resp.json().await?;
    if body["delivered"].as_bool().unwrap_or(false) {
        println!(
            "✓ delivered, status={}",
            body["status"].as_i64().unwrap_or(0)
        );
    } else {
        println!("✗ {}", body["error"].as_str().unwrap_or("unknown"));
    }
    Ok(())
}

pub async fn push_retry(
    project_id: String,
    send_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/push/sends/{send_id}/retry",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    c.post(&url).send().await?.error_for_status()?;
    println!("requeued {send_id}");
    Ok(())
}

pub async fn push_retry_all_failed(
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/push/sends/_retry_all_failed",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.post(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    println!(
        "requeued {} failed sends",
        body["requeued"].as_i64().unwrap_or(0)
    );
    Ok(())
}

pub async fn push_sends_list(
    project_id: String,
    status: Option<String>,
    limit: u32,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let mut url = format!(
        "{}/admin/api/projects/{project_id}/push/sends?limit={limit}",
        resolve_api_url(api_url)
    );
    if let Some(s) = status {
        url.push_str(&format!("&status={s}"));
    }
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["sends"].as_array().cloned().unwrap_or_default();
    println!(
        "{:<10}  {:<8}  {:<10}  {:>4}  outcome / error",
        "id", "provider", "status", "rty"
    );
    for s in &rows {
        println!(
            "{:<10}  {:<8}  {:<10}  {:>4}  {}",
            s["id"]
                .as_str()
                .map(|x| &x[..x.len().min(8)])
                .unwrap_or("?"),
            s["provider"].as_str().unwrap_or("?"),
            s["status"].as_str().unwrap_or("?"),
            s["retry_count"].as_i64().unwrap_or(0),
            s["error"]
                .as_str()
                .or_else(|| s["provider_outcome"].as_str())
                .unwrap_or("—"),
        );
    }
    Ok(())
}

pub async fn push_test(
    project_id: String,
    device_token_id: String,
    title: String,
    body_text: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/push/test",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&url)
        .json(&serde_json::json!({
            "deviceTokenId": device_token_id,
            "title": title,
            "body": body_text,
        }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    println!(
        "queued test push  send_id={}  provider={}",
        body["send_id"].as_str().unwrap_or("?"),
        body["provider"].as_str().unwrap_or("?"),
    );
    Ok(())
}

pub async fn ingest_test(
    error_type: String,
    message: String,
    release: String,
    environment: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/v1/events", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&url)
        .json(&serde_json::json!({
            "kind": "error",
            "error_type": error_type,
            "message": message,
            "platform": "javascript",
            "release": release,
            "environment": environment,
            "timestamp": null,
        }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    println!(
        "{}: issue={} event={}",
        if body["is_new_issue"].as_bool().unwrap_or(false) {
            "new"
        } else {
            "existing"
        },
        body["issue_id"].as_str().unwrap_or("?"),
        body["event_id"].as_str().unwrap_or("?"),
    );
    Ok(())
}

pub async fn probe_list(
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/endpoint-probes",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["probes"].as_array().cloned().unwrap_or_default();
    println!("{:<10}  {:<6}  {:>5}  url", "id", "on", "every");
    for p in &rows {
        println!(
            "{:<10}  {:<6}  {:>5}  {}",
            p["id"]
                .as_str()
                .map(|s| &s[..s.len().min(8)])
                .unwrap_or("?"),
            if p["enabled"].as_bool().unwrap_or(true) {
                "on"
            } else {
                "off"
            },
            p["interval_sec"].as_i64().unwrap_or(60),
            p["endpoint_url"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn probe_create(
    project_id: String,
    target_url: String,
    method: String,
    interval_sec: i32,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/endpoint-probes",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&url)
        .json(&serde_json::json!({
            "name": target_url,
            "target_url": target_url,
            "method": method,
            "interval_sec": interval_sec,
        }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    println!("created probe {}", body["id"].as_str().unwrap_or("?"));
    Ok(())
}

pub async fn init_wizard(api_url: Option<String>) -> Result<()> {
    use std::io::{self, BufRead, Write};
    let api = resolve_api_url(api_url);
    println!("sentori-cli init — server: {api}");
    println!();

    // 1. Server probe.
    let c = reqwest::Client::new();
    let health: Value = c
        .get(format!("{api}/healthz"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    println!(
        "✓ server reachable  version={}  db={}",
        health["version"].as_str().unwrap_or("?"),
        health["db"].as_str().unwrap_or("?"),
    );

    // 2. Login.
    let stdin = io::stdin();
    let mut input = stdin.lock();
    print!("email: ");
    io::stdout().flush()?;
    let mut email = String::new();
    input.read_line(&mut email)?;
    let email = email.trim().to_string();
    print!("password: ");
    io::stdout().flush()?;
    let mut password = String::new();
    input.read_line(&mut password)?;
    let password = password.trim().to_string();

    let body: Value = c
        .post(format!("{api}/auth/login"))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let session_token = body["session_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing session_token"))?
        .to_string();
    println!("✓ logged in as {}", body["email"].as_str().unwrap_or("?"));

    // 3. List projects.
    let projects: Vec<Value> = c
        .get(format!("{api}/v1/projects"))
        .header("authorization", format!("Bearer {session_token}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if projects.is_empty() {
        println!("⚠ no projects yet. Create one via:");
        println!(
            "   sentori-cli project create \"MyApp\" myapp --token {session_token} \\\n     --api-url {api}"
        );
    } else {
        println!("✓ {} project(s):", projects.len());
        for p in &projects {
            println!(
                "    {}  {}  {}",
                p["id"].as_str().unwrap_or("?"),
                p["name"].as_str().unwrap_or("?"),
                p["slug"].as_str().unwrap_or("?"),
            );
        }
    }

    println!();
    println!("Done. Next steps:");
    println!("  export SENTORI_ADMIN_TOKEN={session_token}");
    println!("  export SENTORI_ADMIN_URL={api}");
    println!("  sentori-cli describe       # endpoint catalog");
    println!("  sentori-cli stats --project <id>");
    Ok(())
}

pub async fn auth_login(email: String, password: String, api_url: Option<String>) -> Result<()> {
    let url = format!("{}/auth/login", resolve_api_url(api_url));
    let c = reqwest::Client::new();
    let resp = c
        .post(&url)
        .json(&serde_json::json!({
            "email": email,
            "password": password,
        }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    let token = body["session_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing session_token"))?;
    println!("Logged in as {}", body["email"].as_str().unwrap_or("?"));
    println!();
    println!("export SENTORI_ADMIN_TOKEN={token}");
    println!();
    println!("Or save to ~/.sentori-token for sentori-cli auto-pickup:");
    println!("  echo {token} > ~/.sentori-token");
    Ok(())
}

pub async fn auth_logout(token: Option<String>, api_url: Option<String>) -> Result<()> {
    let url = format!("{}/auth/logout", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    c.post(&url)
        .json(&serde_json::json!({}))
        .send()
        .await?
        .error_for_status()?;
    println!("logged out");
    Ok(())
}

pub async fn push_cred_list(
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/push/credentials",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["credentials"].as_array().cloned().unwrap_or_default();
    println!("{:<10}  {:<10}  {:<20}", "kind", "validated", "created");
    for c in &rows {
        println!(
            "{:<10}  {:<10}  {:<20}",
            c["kind"].as_str().unwrap_or("?"),
            c["last_validate_status"].as_str().unwrap_or("never"),
            c["created_at"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn push_cred_upsert(
    project_id: String,
    provider: String,
    config_json: String,
    secret_path: Option<String>,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    use std::fs;
    let config: Value = serde_json::from_str(&config_json).context("config json invalid")?;
    let secret = if let Some(p) = secret_path {
        Some(fs::read_to_string(&p).context("read secret file")?)
    } else {
        None
    };
    let url = format!(
        "{}/admin/api/projects/{project_id}/push/credentials",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&url)
        .json(&serde_json::json!({
            "provider": provider,
            "config": config,
            "secret": secret,
        }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    println!(
        "upserted push credentials id={} provider={}",
        body["id"].as_str().unwrap_or("?"),
        body["provider"].as_str().unwrap_or("?")
    );
    Ok(())
}

pub async fn push_cred_delete(
    project_id: String,
    kind: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/push/credentials/{kind}",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    c.delete(&url).send().await?.error_for_status()?;
    println!("deleted {kind} credentials for project {project_id}");
    Ok(())
}

pub async fn session_list(
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!("{}/auth/sessions", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["sessions"].as_array().cloned().unwrap_or_default();
    println!("sessions: {}", rows.len());
    for s in &rows {
        println!(
            "  {}  expires {}  ip {}",
            s["id_hash_hex"]
                .as_str()
                .map(|h| &h[..h.len().min(12)])
                .unwrap_or("?"),
            s["expires_at"].as_str().unwrap_or("?"),
            s["ip"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn session_revoke(
    id_hash_hex: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/auth/sessions/{id_hash_hex}", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    c.delete(&url).send().await?.error_for_status()?;
    println!("revoked session {id_hash_hex}");
    Ok(())
}

pub async fn watcher_list(
    issue_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!("{}/v1/issues/{issue_id}/watchers", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["watchers"].as_array().cloned().unwrap_or_default();
    println!("watchers: {}", rows.len());
    for w in &rows {
        println!(
            "  {} (since {})",
            w["user_id"]
                .as_str()
                .map(|s| &s[..s.len().min(8)])
                .unwrap_or("?"),
            w["started_at"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn unwatch_issue(
    issue_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/issues/{issue_id}/watchers",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    c.delete(&url).send().await?.error_for_status()?;
    println!("unwatched {issue_id}");
    Ok(())
}

pub async fn issue_watch(
    issue_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/issues/{issue_id}/watchers",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    c.post(&url)
        .json(&serde_json::json!({}))
        .send()
        .await?
        .error_for_status()?;
    println!("watching {issue_id}");
    Ok(())
}

pub async fn notification_list(
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!("{}/auth/notifications", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    println!("unread: {}", body["unread"].as_i64().unwrap_or(0));
    let rows = body["notifications"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for n in &rows {
        let read = n["read_at"].is_string();
        println!(
            "  {} [{}] {}",
            if read { "·" } else { "●" },
            n["kind"].as_str().unwrap_or("?"),
            n["created_at"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn notification_read_all(token: Option<String>, api_url: Option<String>) -> Result<()> {
    let url = format!("{}/auth/notifications/_read_all", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    c.post(&url)
        .json(&serde_json::json!({}))
        .send()
        .await?
        .error_for_status()?;
    println!("marked all read");
    Ok(())
}

pub async fn describe(api_url: Option<String>, json: bool) -> Result<()> {
    let url = format!("{}/v1/_describe", resolve_api_url(api_url));
    let c = reqwest::Client::new();
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    println!("server: {}", body["version"].as_str().unwrap_or("?"));
    println!(
        "token prefix: {}",
        body["sdk_token_prefix"].as_str().unwrap_or("?")
    );
    println!(
        "session cookie: {}",
        body["session_cookie"].as_str().unwrap_or("?")
    );
    if let Some(groups) = body["endpoints"].as_object() {
        for (group, list) in groups {
            let n = list.as_array().map(|a| a.len()).unwrap_or(0);
            println!("  {group:<15}  {n:>3} endpoints");
        }
    }
    Ok(())
}

pub async fn health_check(api_url: Option<String>) -> Result<()> {
    let url = format!("{}/healthz", resolve_api_url(api_url));
    // No auth needed
    let c = reqwest::Client::new();
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    println!(
        "status:  {}\ndb:      {}\nversion: {}",
        body["status"].as_str().unwrap_or("?"),
        body["db"].as_str().unwrap_or("?"),
        body["version"].as_str().unwrap_or("?"),
    );
    Ok(())
}

pub async fn ops_report(api_url: Option<String>) -> Result<()> {
    let base = resolve_api_url(api_url.clone());
    let c = reqwest::Client::new();

    // healthz
    println!("== /healthz ==");
    if let Ok(resp) = c.get(format!("{base}/healthz")).send().await {
        let status = resp.status().as_u16();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        println!(
            "  HTTP {} status={} db={} version={} pool={}/{} push_queued={} push_failed_24h={}",
            status,
            body["status"].as_str().unwrap_or("?"),
            body["db"].as_str().unwrap_or("?"),
            body["version"].as_str().unwrap_or("?"),
            body["pool_size"].as_u64().unwrap_or(0),
            body["pool_idle"].as_u64().unwrap_or(0),
            body["push_queued"].as_u64().unwrap_or(0),
            body["push_failed_24h"].as_u64().unwrap_or(0),
        );
    }

    // self-test
    println!("\n== /v1/_self_test ==");
    let _ = self_test(api_url.clone()).await;

    // metrics summary (grep a few known names)
    println!("\n== /metrics (highlights) ==");
    if let Ok(text) = c
        .get(format!("{base}/metrics"))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        && let Ok(s) = text.text().await
    {
        for line in s.lines() {
            if line.starts_with("sentori_db_pool_")
                || line.starts_with("sentori_push_")
                || line.starts_with("sentori_events_")
                || line.starts_with("sentori_issues_")
                || line.starts_with("sentori_alerts_active")
                || line.starts_with("sentori_user_sessions_active")
                || line.starts_with("sentori_build_info")
            {
                println!("  {line}");
            }
        }
    }
    Ok(())
}

pub async fn self_test(api_url: Option<String>) -> Result<()> {
    let url = format!("{}/v1/_self_test", resolve_api_url(api_url));
    let c = reqwest::Client::new();
    let resp = c.get(&url).send().await?;
    let status = resp.status().as_u16();
    let body: Value = resp.json().await?;
    let overall = body["ok"].as_bool().unwrap_or(false);
    println!(
        "{} v{}  (HTTP {})",
        if overall {
            "✓ all checks pass"
        } else {
            "✗ some checks failed"
        },
        body["version"].as_str().unwrap_or("?"),
        status,
    );
    for c in body["checks"].as_array().cloned().unwrap_or_default() {
        let ok = c["ok"].as_bool().unwrap_or(false);
        println!(
            "  {}  {}{}",
            if ok { "✓" } else { "✗" },
            c["name"].as_str().unwrap_or("?"),
            c["detail"]
                .as_str()
                .map(|d| format!("  ({d})"))
                .unwrap_or_default(),
        );
    }
    if !overall {
        std::process::exit(1);
    }
    Ok(())
}

pub async fn metrics_raw(api_url: Option<String>) -> Result<()> {
    let url = format!("{}/metrics", resolve_api_url(api_url));
    let c = reqwest::Client::new();
    let text = c.get(&url).send().await?.error_for_status()?.text().await?;
    print!("{text}");
    Ok(())
}

pub async fn me_show(token: Option<String>, api_url: Option<String>) -> Result<()> {
    let url = format!("{}/auth/me", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    println!(
        "user_id:        {}\nemail:          {}\nemail_verified: {}\ncreated_at:     {}",
        body["user_id"].as_str().unwrap_or("?"),
        body["email"].as_str().unwrap_or("?"),
        body["email_verified"].as_bool().unwrap_or(false),
        body["created_at"].as_str().unwrap_or("?"),
    );
    Ok(())
}

pub async fn live_tail(token: Option<String>, api_url: Option<String>) -> Result<()> {
    use anyhow::anyhow;
    use std::io::{self, Write};
    let url = format!("{}/v1/events/_recent", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c
        .get(&url)
        .header("accept", "text/event-stream")
        .send()
        .await?
        .error_for_status()?;
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| anyhow!("stream: {e}"))?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(idx) = buf.find("\n\n") {
            let frame = buf[..idx].to_string();
            buf.drain(..idx + 2);
            for line in frame.lines() {
                if let Some(data) = line.strip_prefix("data:") {
                    let json: serde_json::Value =
                        serde_json::from_str(data.trim()).unwrap_or(serde_json::Value::Null);
                    println!(
                        "{}  {}  {}  {}  issue={}",
                        json["timestamp"].as_str().unwrap_or("?"),
                        json["kind"].as_str().unwrap_or("?"),
                        json["platform"].as_str().unwrap_or("?"),
                        json["release"].as_str().unwrap_or("?"),
                        json["issue_id"]
                            .as_str()
                            .map(|s| &s[..s.len().min(8)])
                            .unwrap_or("?"),
                    );
                    io::stdout().flush().ok();
                }
            }
        }
    }
    Ok(())
}

pub async fn release_list(
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/releases",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["releases"].as_array().cloned().unwrap_or_default();
    println!("{:<38}  deploy_at            name", "id");
    for r in &rows {
        println!(
            "{:<38}  {:<20}  {}",
            r["id"].as_str().unwrap_or("?"),
            r["deploy_at"].as_str().unwrap_or("—"),
            r["name"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn release_artifacts(
    project_id: String,
    release_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/admin/api/projects/{project_id}/releases/{release_id}/artifacts",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["artifacts"].as_array().cloned().unwrap_or_default();
    println!("{:<10}  {:>10}  hash             name", "kind", "size");
    for a in &rows {
        println!(
            "{:<10}  {:>10}  {:<14}  {}",
            a["kind"].as_str().unwrap_or("?"),
            a["size_bytes"].as_i64().unwrap_or(0),
            a["content_hash"]
                .as_str()
                .map(|s| &s[..s.len().min(14)])
                .unwrap_or("?"),
            a["name"].as_str().unwrap_or("?"),
        );
    }
    Ok(())
}

pub async fn push_send(
    native_tokens: Vec<String>,
    title: String,
    body_text: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!("{}/v1/push/send", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c
        .post(&url)
        .json(&serde_json::json!({
            "nativeTokens": native_tokens,
            "payload": { "title": title, "body": body_text },
        }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    println!(
        "queued {} push(es): {}",
        body["queued"].as_i64().unwrap_or(0),
        serde_json::to_string(&body["send_ids"])?,
    );
    Ok(())
}

pub async fn replay_download(
    project_id: String,
    replay_id: String,
    token: Option<String>,
    api_url: Option<String>,
) -> Result<()> {
    let url = format!(
        "{}/v1/projects/{project_id}/replays/{replay_id}/ndjson",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let bytes = c
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    // Print raw NDJSON to stdout — pipe to file: > replay.ndjson
    use std::io::Write;
    std::io::stdout().write_all(&bytes)?;
    Ok(())
}

pub async fn search_project(
    project_id: String,
    query: String,
    limit: u32,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    // Minimal manual encode: replace unsafe chars. Good enough
    // for typical search terms (alphanumerics + spaces + dots).
    let encoded = query
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect::<String>();
    let url = format!(
        "{}/v1/projects/{project_id}/search?q={encoded}&limit={limit}",
        resolve_api_url(api_url),
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let issues = body["issues"].as_array().cloned().unwrap_or_default();
    let events = body["events"].as_array().cloned().unwrap_or_default();
    if !issues.is_empty() {
        println!("── Issues ──");
        for i in &issues {
            println!(
                "  [{}] {} — {}",
                i["status"].as_str().unwrap_or("?"),
                i["error_type"].as_str().unwrap_or("?"),
                i["message_sample"]
                    .as_str()
                    .map(|s| &s[..s.len().min(60)])
                    .unwrap_or(""),
            );
        }
    }
    if !events.is_empty() {
        println!("── Events ──");
        for e in &events {
            println!(
                "  {} {} {} {}",
                e["timestamp"].as_str().unwrap_or("?"),
                e["kind"].as_str().unwrap_or("?"),
                e["release"].as_str().unwrap_or("?"),
                e["environment"].as_str().unwrap_or("?"),
            );
        }
    }
    Ok(())
}

pub async fn stats_show(
    project_id: String,
    token: Option<String>,
    api_url: Option<String>,
    json: bool,
) -> Result<()> {
    let url = format!(
        "{}/v1/projects/{project_id}/stats",
        resolve_api_url(api_url)
    );
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    println!("project {project_id}");
    println!(
        "  events 24h:    {:>10}",
        body["events_24h"].as_i64().unwrap_or(0)
    );
    println!(
        "  issues active: {:>10}",
        body["issues_active"].as_i64().unwrap_or(0)
    );
    println!(
        "  spans 24h:     {:>10}",
        body["spans_24h"].as_i64().unwrap_or(0)
    );
    println!(
        "  metrics 24h:   {:>10}  (buckets)",
        body["metrics_buckets_24h"].as_i64().unwrap_or(0)
    );
    println!(
        "  replays 24h:   {:>10}",
        body["replays_24h"].as_i64().unwrap_or(0)
    );
    Ok(())
}

pub async fn invite_list(token: Option<String>, api_url: Option<String>, json: bool) -> Result<()> {
    let url = format!("{}/admin/api/invites", resolve_api_url(api_url));
    let c = client(&token_value(token)?)?;
    let resp = c.get(&url).send().await?.error_for_status()?;
    let body: Value = resp.json().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let rows = body["invites"].as_array().cloned().unwrap_or_default();
    println!("{:<38}  {:<28}  {:<6}  status", "id", "email", "role");
    for i in &rows {
        let accepted = !i["accepted_at"].is_null();
        let status = if accepted { "accepted" } else { "pending" };
        println!(
            "{:<38}  {:<28}  {:<6}  {}",
            i["id"].as_str().unwrap_or("?"),
            i["email"].as_str().unwrap_or("?"),
            i["role"].as_str().unwrap_or("?"),
            status,
        );
    }
    Ok(())
}
