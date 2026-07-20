use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use walkdir::WalkDir;

mod admin;
mod dsym;
mod issue;

const DEFAULT_INGEST_URL: &str = "https://ingest.sentori.golia.jp";

#[derive(Parser)]
#[command(
    name = "sentori-cli",
    version,
    about = "Sentori CLI — upload source maps and other release artifacts"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Upload release artifacts (source maps for now; dSYM / ProGuard later).
    Upload {
        #[command(subcommand)]
        kind: UploadKind,
    },
    /// Operate on issues — list, resolve, silence. Reads admin token from
    /// the same env-fallback chain as `upload dsym`.
    Issue {
        #[command(subcommand)]
        kind: IssueKind,
    },
    /// Project CRUD via /admin/api/projects/*.
    Project {
        #[command(subcommand)]
        kind: ProjectKind,
    },
    /// SDK ingest token CRUD via /admin/api/projects/<id>/tokens.
    Token {
        #[command(subcommand)]
        kind: TokenKind,
    },
    /// Workspace audit log (list / filter via /v1/audit).
    Audit {
        #[arg(long = "project")]
        project_id: Option<String>,
        #[arg(long = "actor")]
        actor: Option<String>,
        #[arg(long)]
        action: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Workspace member ops via /admin/api/members.
    Member {
        #[command(subcommand)]
        kind: MemberKind,
    },
    /// Workspace invite ops via /admin/api/invites.
    Invite {
        #[command(subcommand)]
        kind: InviteKind,
    },
    /// Alert rule list / delete via /v1/alerts.
    Alert {
        #[command(subcommand)]
        kind: AlertKind,
    },
    /// Saved view list / delete via /v1/saved-views.
    View {
        #[command(subcommand)]
        kind: ViewKind,
    },
    /// Cert monitor ops via /v1/projects/<id>/cert/* + /admin/api/.../cert/watches.
    Cert {
        #[command(subcommand)]
        kind: CertKind,
    },
    /// Workspace usage report via /v1/usage.
    Usage {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Per-project trace list via /v1/projects/<id>/traces.
    Trace {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Per-project replay list via /v1/projects/<id>/replays.
    Replay {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Download raw NDJSON replay blob (pipe to file).
    ReplayGet {
        #[arg(long = "project")]
        project_id: String,
        replay_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Per-project metric list via /v1/projects/<id>/metrics.
    Metric {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Post a markdown comment to an issue.
    Comment {
        issue_id: String,
        body_md: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// List comments for an issue.
    Comments {
        issue_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// User's notification inbox (Bearer or session cookie).
    Inbox {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Mark all current-user notifications read.
    InboxReadAll {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Self-describing API catalog (route surface + version).
    Describe {
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Server healthz check (no auth needed).
    Health {
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Aggregate diagnostic — healthz + self-test + metrics summary.
    Ops {
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Run /v1/_self_test smoke checks; exit 1 if any fail.
    SelfTest {
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Dump raw Prometheus /metrics text (pipe into grep/awk).
    Metrics {
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Show the current session user (Bearer or session cookie).
    #[command(alias = "whoami")]
    Me {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Live-tail recent events for the bearer's project via SSE.
    /// Streams until ctrl-c.
    LiveTail {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Per-project release list.
    Release {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Per-release artifact list (sourcemap / dsym / proguard).
    ReleaseArtifacts {
        #[arg(long = "project")]
        project_id: String,
        release_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Send a test push notification.
    PushSend {
        /// Native device tokens (apns / fcm / webpush endpoint URLs).
        #[arg(long = "to", value_delimiter = ',')]
        native_tokens: Vec<String>,
        #[arg(long, default_value = "Sentori test")]
        title: String,
        #[arg(long = "body", default_value = "hello from sentori-cli")]
        body_text: String,
        /// Must be a `public` token for the project that owns
        /// the device tokens.
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// List workspace alert rules.
    Alerts {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Append a channel to an alert rule (webhook/slack).
    AlertChannelAdd {
        alert_id: String,
        #[arg(long, default_value = "webhook")]
        kind: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        secret: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Fire-test an alert rule's channels.
    AlertFire {
        alert_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Patch an alert rule (enabled/muted/throttle).
    AlertPatch {
        alert_id: String,
        #[arg(long)]
        enabled: Option<bool>,
        #[arg(long)]
        muted: Option<bool>,
        #[arg(long = "throttle-minutes")]
        throttle_minutes: Option<i32>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Send a test webhook payload (Slack-compatible URL, etc).
    WebhookTest {
        url: String,
        #[arg(long)]
        secret: Option<String>,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Retry-now a single failed push send (DLQ unstuck).
    PushRetry {
        #[arg(long = "project")]
        project_id: String,
        send_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Re-queue every failed push send for a project at once.
    PushRetryAll {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Recent push sends with status / retry / error (triage / DLQ).
    PushSends {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        status: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Queue a real test push for a known device_token (uses
    /// the configured vendor + credentials). Session-gated.
    PushTest {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long = "device-token-id")]
        device_token_id: String,
        #[arg(long, default_value = "Sentori test")]
        title: String,
        #[arg(long = "body", default_value = "hello from sentori-cli")]
        body_text: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Send a synthetic test event (requires public token).
    IngestTest {
        #[arg(long, default_value = "TypeError")]
        error_type: String,
        #[arg(long, default_value = "x is undefined (cli test)")]
        message: String,
        #[arg(long, default_value = "cli-test@0.0.1")]
        release: String,
        #[arg(long, default_value = "development")]
        environment: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// List endpoint probes for a project.
    Probe {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Create a new endpoint probe.
    ProbeCreate {
        #[arg(long = "project")]
        project_id: String,
        target_url: String,
        #[arg(long, default_value = "GET")]
        method: String,
        #[arg(long, default_value_t = 60)]
        interval_sec: i32,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Interactive bootstrap wizard — probe server, login,
    /// list projects, print next-step commands.
    Init {
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Email + password login; prints session token for export.
    Login {
        email: String,
        password: String,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Server-side session logout.
    Logout {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// List push credentials for a project.
    PushCred {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Upsert a push credential.
    PushCredUpsert {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        provider: String,
        /// Config JSON. Example for webpush:
        ///   '{"subject":"mailto:a@b.com","vapidPublicKey":"BMqS..."}'
        #[arg(long)]
        config: String,
        /// Path to a file containing the secret (PEM, server key, etc).
        #[arg(long = "secret-file")]
        secret_path: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Delete a push credential by kind.
    PushCredDelete {
        #[arg(long = "project")]
        project_id: String,
        kind: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// List current user's active sessions.
    Sessions {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Revoke a specific session by id_hash_hex.
    SessionRevoke {
        id_hash_hex: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// List watchers of an issue.
    Watchers {
        issue_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Unwatch an issue.
    Unwatch {
        issue_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Subscribe (watch) an issue for the current session user.
    Watch {
        issue_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Quick LIKE-search across issues + events.
    Search {
        #[arg(long = "project")]
        project_id: String,
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: u32,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Per-project 24h lens counts via /v1/projects/<id>/stats.
    Stats {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum CertKind {
    /// List observed certs for a project.
    List {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Add a domain to the watch list.
    Watch {
        #[arg(long = "project")]
        project_id: String,
        domain: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Stop watching a domain.
    Unwatch {
        #[arg(long = "project")]
        project_id: String,
        domain: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum AlertKind {
    /// List workspace alert rules.
    List {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Delete an alert rule.
    Delete {
        alert_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Show alert rule detail (raw JSON).
    Show {
        alert_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum ViewKind {
    /// List saved views for a target.
    List {
        /// `issues` / `events` / `spans` / `replays` / `metrics`.
        #[arg(long, default_value = "issues")]
        target: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Delete a saved view.
    Delete {
        view_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum MemberKind {
    /// List workspace members.
    List {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Remove a member from the workspace.
    Remove {
        user_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum InviteKind {
    /// List pending + accepted invites.
    List {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Mint a new invite token.
    Mint {
        email: String,
        /// `admin` or `user` (default `user`).
        #[arg(long, default_value = "user")]
        role: String,
        /// Inviter user_id (your UUID).
        #[arg(long = "invited-by")]
        invited_by: String,
        /// Days until expiry.
        #[arg(long, default_value_t = 7)]
        expires_in_days: i64,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum ProjectKind {
    /// List all projects in the workspace.
    List {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Create a new project (name + slug).
    Create {
        name: String,
        slug: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Delete a project (cascades events / issues / spans).
    Delete {
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum TokenKind {
    /// List ingest tokens for a project.
    List {
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Mint a new ingest token. Plaintext shown ONCE.
    Mint {
        #[arg(long = "project")]
        project_id: String,
        /// Display label, e.g. "production iOS".
        #[arg(long)]
        label: Option<String>,
        /// `public` (default — SDK ingest) or `admin`.
        #[arg(long, default_value = "public")]
        kind: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Revoke an ingest token by id.
    Revoke {
        token_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum IssueKind {
    /// List issues for a project.
    List {
        /// Project UUID.
        #[arg(long = "project")]
        project_id: String,
        /// Filter by issue status. Defaults to `active`. `all` to skip.
        #[arg(long, default_value = "active")]
        status: String,
        /// Filter by release tag.
        #[arg(long)]
        release: Option<String>,
        /// Max rows returned.
        #[arg(long, default_value_t = 20)]
        limit: u32,
        /// Print the server JSON as-is instead of the dense table.
        #[arg(long)]
        json: bool,
        /// Admin / user-session token. Defaults to `SENTORI_ADMIN_TOKEN`
        /// then `SENTORI_TOKEN`.
        #[arg(long)]
        token: Option<String>,
        /// Admin API base. Same fallback chain as `upload dsym`.
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Mark an issue resolved (optionally tag the release that fixed it).
    Resolve {
        /// Issue UUID.
        issue_id: String,
        /// Project UUID.
        #[arg(long = "project")]
        project_id: String,
        /// Release tag where the fix landed, e.g. `myapp@1.2.4+457`.
        #[arg(long = "in-release")]
        in_release: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
    /// Silence an issue (no notifications; events still ingested).
    Silence {
        issue_id: String,
        #[arg(long = "project")]
        project_id: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "api-url")]
        api_url: Option<String>,
    },
}

#[derive(Subcommand)]
enum UploadKind {
    /// Upload `.js` and `.js.map` files for a release.
    Sourcemap {
        /// Release name, e.g. `myapp@1.2.3+456`
        #[arg(long)]
        release: String,
        /// Bearer token. Defaults to `SENTORI_TOKEN` env var.
        #[arg(long)]
        token: Option<String>,
        /// Ingest URL base. Defaults to `SENTORI_INGEST_URL` env var,
        /// or `https://ingest.sentori.golia.jp`.
        #[arg(long = "ingest-url")]
        ingest_url: Option<String>,
        /// Files or directories. Directories are walked recursively for
        /// `.js` and `.js.map`.
        files: Vec<PathBuf>,
    },
    /// Upload an iOS / macOS `.dSYM` bundle. Walks fat binaries,
    /// posting one slice per (debug_id, arch).
    Dsym {
        /// Project UUID. Required — dSYMs are scoped per project,
        /// not per release.
        #[arg(long = "project")]
        project_id: String,
        /// Release name, e.g. `myapp@1.2.3+456`. Optional but
        /// recommended — used by the dashboard release detail page.
        #[arg(long)]
        release: Option<String>,
        /// Admin or user-session token. Defaults to `SENTORI_ADMIN_TOKEN`
        /// then `SENTORI_TOKEN`.
        #[arg(long)]
        token: Option<String>,
        /// Admin API base, e.g. `https://sentori.golia.jp`.
        /// Defaults to `SENTORI_ADMIN_URL`, then derives from the
        /// public `SENTORI_INGEST_URL` by replacing `ingest.` →
        /// `api.`, then falls back to `https://sentori.golia.jp`.
        #[arg(long = "api-url")]
        api_url: Option<String>,
        /// `.dSYM` bundle paths or directories containing them.
        /// Directories are walked for any `*.dSYM` matches.
        paths: Vec<PathBuf>,
    },
    /// Upload an Android ProGuard / R8 mapping.txt. The server sniffs
    /// `# pg_map_id:` from the mapping header for the debug-id;
    /// `--release` lets the retracer match by release name when the
    /// mapping has no embedded id.
    Mapping {
        /// Project UUID.
        #[arg(long = "project")]
        project_id: String,
        /// Release name, e.g. `myapp@1.2.3+456`.
        #[arg(long)]
        release: Option<String>,
        /// Admin token. Same env-fallback chain as `dsym`.
        #[arg(long)]
        token: Option<String>,
        /// Admin API base. Same fallback chain as `dsym`.
        #[arg(long = "api-url")]
        api_url: Option<String>,
        /// `mapping.txt` path.
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Upload { kind } => match kind {
            UploadKind::Sourcemap {
                release,
                token,
                ingest_url,
                files,
            } => upload_sourcemap(release, token, ingest_url, files).await,
            UploadKind::Dsym {
                project_id,
                release,
                token,
                api_url,
                paths,
            } => upload_dsym(project_id, release, token, api_url, paths).await,
            UploadKind::Mapping {
                project_id,
                release,
                token,
                api_url,
                path,
            } => upload_mapping(project_id, release, token, api_url, path).await,
        },
        Command::Issue { kind } => match kind {
            IssueKind::List {
                project_id,
                status,
                release,
                limit,
                json,
                token,
                api_url,
            } => issue::list(project_id, status, release, limit, json, token, api_url).await,
            IssueKind::Resolve {
                issue_id,
                project_id,
                in_release,
                token,
                api_url,
            } => issue::resolve(issue_id, project_id, in_release, token, api_url).await,
            IssueKind::Silence {
                issue_id,
                project_id,
                token,
                api_url,
            } => issue::silence(issue_id, project_id, token, api_url).await,
        },
        Command::Project { kind } => match kind {
            ProjectKind::List {
                token,
                api_url,
                json,
            } => admin::project_list(token, api_url, json).await,
            ProjectKind::Create {
                name,
                slug,
                token,
                api_url,
            } => admin::project_create(name, slug, token, api_url).await,
            ProjectKind::Delete {
                project_id,
                token,
                api_url,
            } => admin::project_delete(project_id, token, api_url).await,
        },
        Command::Token { kind } => match kind {
            TokenKind::List {
                project_id,
                token,
                api_url,
                json,
            } => admin::token_list(project_id, token, api_url, json).await,
            TokenKind::Mint {
                project_id,
                label,
                kind: tk,
                token,
                api_url,
            } => admin::token_mint(project_id, label, tk, token, api_url).await,
            TokenKind::Revoke {
                token_id,
                token,
                api_url,
            } => admin::token_revoke(token_id, token, api_url).await,
        },
        Command::Audit {
            project_id,
            actor,
            action,
            limit,
            token,
            api_url,
            json,
        } => {
            admin::audit_list(project_id, actor, action, limit, token, api_url, json)
                .await
        }
        Command::Member { kind } => match kind {
            MemberKind::List {
                token,
                api_url,
                json,
            } => admin::member_list(token, api_url, json).await,
            MemberKind::Remove {
                user_id,
                token,
                api_url,
            } => admin::member_remove(user_id, token, api_url).await,
        },
        Command::Invite { kind } => match kind {
            InviteKind::List {
                token,
                api_url,
                json,
            } => admin::invite_list(token, api_url, json).await,
            InviteKind::Mint {
                email,
                role,
                invited_by,
                expires_in_days,
                token,
                api_url,
            } => {
                admin::invite_mint(
                    email,
                    role,
                    invited_by,
                    expires_in_days,
                    token,
                    api_url,
                )
                .await
            }
        },
        Command::Alert { kind } => match kind {
            AlertKind::List {
                token,
                api_url,
                json,
            } => admin::alert_list(token, api_url, json).await,
            AlertKind::Delete {
                alert_id,
                token,
                api_url,
            } => admin::alert_delete(alert_id, token, api_url).await,
            AlertKind::Show {
                alert_id,
                token,
                api_url,
            } => admin::alert_show(alert_id, token, api_url).await,
        },
        Command::View { kind } => match kind {
            ViewKind::List {
                target,
                token,
                api_url,
                json,
            } => admin::view_list(target, token, api_url, json).await,
            ViewKind::Delete {
                view_id,
                token,
                api_url,
            } => admin::view_delete(view_id, token, api_url).await,
        },
        Command::Cert { kind } => match kind {
            CertKind::List {
                project_id,
                token,
                api_url,
                json,
            } => admin::cert_list(project_id, token, api_url, json).await,
            CertKind::Watch {
                project_id,
                domain,
                token,
                api_url,
            } => admin::cert_watch(project_id, domain, token, api_url).await,
            CertKind::Unwatch {
                project_id,
                domain,
                token,
                api_url,
            } => admin::cert_unwatch(project_id, domain, token, api_url).await,
        },
        Command::Usage {
            token,
            api_url,
            json,
        } => admin::usage_show(token, api_url, json).await,
        Command::Trace {
            project_id,
            limit,
            token,
            api_url,
            json,
        } => admin::trace_list(project_id, limit, token, api_url, json).await,
        Command::Replay {
            project_id,
            limit,
            token,
            api_url,
            json,
        } => admin::replay_list(project_id, limit, token, api_url, json).await,
        Command::ReplayGet {
            project_id,
            replay_id,
            token,
            api_url,
        } => admin::replay_download(project_id, replay_id, token, api_url).await,
        Command::Metric {
            project_id,
            token,
            api_url,
            json,
        } => admin::metric_list(project_id, token, api_url, json).await,
        Command::Comment {
            issue_id,
            body_md,
            token,
            api_url,
        } => admin::comment_post(issue_id, body_md, token, api_url).await,
        Command::Comments {
            issue_id,
            token,
            api_url,
            json,
        } => admin::comment_list(issue_id, token, api_url, json).await,
        Command::Inbox {
            token,
            api_url,
            json,
        } => admin::notification_list(token, api_url, json).await,
        Command::InboxReadAll { token, api_url } => {
            admin::notification_read_all(token, api_url).await
        }
        Command::Describe { api_url, json } => admin::describe(api_url, json).await,
        Command::Health { api_url } => admin::health_check(api_url).await,
        Command::Ops { api_url } => admin::ops_report(api_url).await,
        Command::SelfTest { api_url } => admin::self_test(api_url).await,
        Command::Metrics { api_url } => admin::metrics_raw(api_url).await,
        Command::Me { token, api_url } => admin::me_show(token, api_url).await,
        Command::LiveTail { token, api_url } => admin::live_tail(token, api_url).await,
        Command::Release {
            project_id,
            token,
            api_url,
            json,
        } => admin::release_list(project_id, token, api_url, json).await,
        Command::ReleaseArtifacts {
            project_id,
            release_id,
            token,
            api_url,
            json,
        } => admin::release_artifacts(project_id, release_id, token, api_url, json).await,
        Command::PushSend {
            native_tokens,
            title,
            body_text,
            token,
            api_url,
        } => admin::push_send(native_tokens, title, body_text, token, api_url).await,
        Command::Watch {
            issue_id,
            token,
            api_url,
        } => admin::issue_watch(issue_id, token, api_url).await,
        Command::Alerts {
            token,
            api_url,
            json,
        } => admin::alerts_list(token, api_url, json).await,
        Command::AlertChannelAdd {
            alert_id,
            kind,
            url,
            secret,
            token,
            api_url,
        } => {
            admin::alert_channel_add(alert_id, kind, url, secret, token, api_url).await
        }
        Command::AlertFire {
            alert_id,
            token,
            api_url,
        } => admin::alert_fire_test(alert_id, token, api_url).await,
        Command::AlertPatch {
            alert_id,
            enabled,
            muted,
            throttle_minutes,
            token,
            api_url,
        } => {
            admin::alert_patch(
                alert_id,
                enabled,
                muted,
                throttle_minutes,
                token,
                api_url,
            )
            .await
        }
        Command::WebhookTest {
            url,
            secret,
            message,
            token,
            api_url,
        } => admin::webhook_test(url, secret, message, token, api_url).await,
        Command::PushRetry {
            project_id,
            send_id,
            token,
            api_url,
        } => admin::push_retry(project_id, send_id, token, api_url).await,
        Command::PushRetryAll {
            project_id,
            token,
            api_url,
        } => admin::push_retry_all_failed(project_id, token, api_url).await,
        Command::PushSends {
            project_id,
            status,
            limit,
            token,
            api_url,
            json,
        } => {
            admin::push_sends_list(project_id, status, limit, token, api_url, json).await
        }
        Command::PushTest {
            project_id,
            device_token_id,
            title,
            body_text,
            token,
            api_url,
        } => {
            admin::push_test(
                project_id,
                device_token_id,
                title,
                body_text,
                token,
                api_url,
            )
            .await
        }
        Command::IngestTest {
            error_type,
            message,
            release,
            environment,
            token,
            api_url,
        } => {
            admin::ingest_test(
                error_type,
                message,
                release,
                environment,
                token,
                api_url,
            )
            .await
        }
        Command::Probe {
            project_id,
            token,
            api_url,
            json,
        } => admin::probe_list(project_id, token, api_url, json).await,
        Command::ProbeCreate {
            project_id,
            target_url,
            method,
            interval_sec,
            token,
            api_url,
        } => admin::probe_create(project_id, target_url, method, interval_sec, token, api_url).await,
        Command::Init { api_url } => admin::init_wizard(api_url).await,
        Command::Login {
            email,
            password,
            api_url,
        } => admin::auth_login(email, password, api_url).await,
        Command::Logout { token, api_url } => admin::auth_logout(token, api_url).await,
        Command::PushCred {
            project_id,
            token,
            api_url,
            json,
        } => admin::push_cred_list(project_id, token, api_url, json).await,
        Command::PushCredUpsert {
            project_id,
            provider,
            config,
            secret_path,
            token,
            api_url,
        } => {
            admin::push_cred_upsert(
                project_id,
                provider,
                config,
                secret_path,
                token,
                api_url,
            )
            .await
        }
        Command::PushCredDelete {
            project_id,
            kind,
            token,
            api_url,
        } => admin::push_cred_delete(project_id, kind, token, api_url).await,
        Command::Sessions {
            token,
            api_url,
            json,
        } => admin::session_list(token, api_url, json).await,
        Command::SessionRevoke {
            id_hash_hex,
            token,
            api_url,
        } => admin::session_revoke(id_hash_hex, token, api_url).await,
        Command::Watchers {
            issue_id,
            token,
            api_url,
            json,
        } => admin::watcher_list(issue_id, token, api_url, json).await,
        Command::Unwatch {
            issue_id,
            token,
            api_url,
        } => admin::unwatch_issue(issue_id, token, api_url).await,
        Command::Search {
            project_id,
            query,
            limit,
            token,
            api_url,
            json,
        } => admin::search_project(project_id, query, limit, token, api_url, json).await,
        Command::Stats {
            project_id,
            token,
            api_url,
            json,
        } => admin::stats_show(project_id, token, api_url, json).await,
    }
}

async fn upload_mapping(
    project_id: String,
    release: Option<String>,
    token: Option<String>,
    api_url: Option<String>,
    path: PathBuf,
) -> Result<()> {
    let token = token
        .or_else(|| std::env::var("SENTORI_ADMIN_TOKEN").ok())
        .or_else(|| std::env::var("SENTORI_TOKEN").ok())
        .context("token: pass --token or set SENTORI_ADMIN_TOKEN / SENTORI_TOKEN")?;

    let base = api_url
        .or_else(|| std::env::var("SENTORI_ADMIN_URL").ok())
        .or_else(|| {
            std::env::var("SENTORI_INGEST_URL")
                .ok()
                .map(|s| s.replace("ingest.", "api."))
        })
        .unwrap_or_else(|| "https://sentori.golia.jp".to_string());

    if !path.is_file() {
        anyhow::bail!("mapping path is not a file: {}", path.display());
    }
    let bytes = tokio::fs::read(&path)
        .await
        .with_context(|| format!("reading {}", path.display()))?;

    let mut url = format!(
        "{}/admin/api/projects/{}/mappings",
        base.trim_end_matches('/'),
        project_id
    );
    if let Some(r) = release.as_deref() {
        url.push_str(&format!("?release={}", urlencoding(r)));
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .bearer_auth(&token)
        .header("content-type", "application/octet-stream")
        .body(bytes)
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("upload failed: {status} {body}");
    }
    println!("OK ({status}): {body}");
    Ok(())
}

async fn upload_dsym(
    project_id: String,
    release: Option<String>,
    token: Option<String>,
    api_url: Option<String>,
    paths: Vec<PathBuf>,
) -> Result<()> {
    let token = token
        .or_else(|| std::env::var("SENTORI_ADMIN_TOKEN").ok())
        .or_else(|| std::env::var("SENTORI_TOKEN").ok())
        .context("token: pass --token or set SENTORI_ADMIN_TOKEN / SENTORI_TOKEN")?;

    let base = api_url
        .or_else(|| std::env::var("SENTORI_ADMIN_URL").ok())
        .or_else(|| {
            std::env::var("SENTORI_INGEST_URL")
                .ok()
                .map(|s| s.replace("ingest.", "api."))
        })
        .unwrap_or_else(|| "https://sentori.golia.jp".to_string());

    let bundles = collect_dsym_bundles(&paths)?;
    if bundles.is_empty() {
        anyhow::bail!("no `.dSYM` bundles found in the given paths");
    }

    let mut slices: Vec<dsym::Slice> = Vec::new();
    for bundle in &bundles {
        let extracted = dsym::slices_from_bundle(bundle)
            .with_context(|| format!("parsing {}", bundle.display()))?;
        slices.extend(extracted);
    }
    if slices.is_empty() {
        anyhow::bail!("found `.dSYM` bundle(s) but no Mach-O slices inside");
    }

    println!(
        "Uploading {} slice(s) from {} bundle(s) to project {project_id} via {base}",
        slices.len(),
        bundles.len()
    );

    let client = reqwest::Client::new();
    let mut ok = 0;
    let total = slices.len();
    for s in slices {
        let mut url = format!(
            "{}/admin/api/projects/{}/dsyms",
            base.trim_end_matches('/'),
            project_id
        );
        let mut qs = Vec::new();
        if let Some(r) = release.as_deref() {
            qs.push(format!("release={}", urlencoding(r)));
        }
        if !s.object_name.is_empty() {
            qs.push(format!("objectName={}", urlencoding(&s.object_name)));
        }
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs.join("&"));
        }
        let resp = client
            .post(&url)
            .bearer_auth(&token)
            .header("content-type", "application/octet-stream")
            .header("x-sentori-debug-id", &s.debug_id)
            .header("x-sentori-arch", &s.arch)
            .body(s.data)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            eprintln!(
                "  ! {} ({} bytes) → {status} {body}",
                s.debug_id, s.size_bytes
            );
            continue;
        }
        println!(
            "  ✓ {} {} ({} bytes)",
            s.debug_id, s.arch, s.size_bytes
        );
        ok += 1;
    }
    println!("Done. {ok}/{total} ok.");
    Ok(())
}

fn collect_dsym_bundles(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        if !p.exists() {
            anyhow::bail!("path does not exist: {}", p.display());
        }
        if is_dsym_bundle(p) {
            out.push(p.clone());
            continue;
        }
        if p.is_dir() {
            for entry in WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
                if is_dsym_bundle(entry.path()) {
                    out.push(entry.path().to_path_buf());
                }
            }
        }
    }
    Ok(out)
}

fn is_dsym_bundle(p: &Path) -> bool {
    p.extension().map(|e| e == "dSYM").unwrap_or(false)
}

async fn upload_sourcemap(
    release: String,
    token: Option<String>,
    ingest_url: Option<String>,
    files: Vec<PathBuf>,
) -> Result<()> {
    let token = token
        .or_else(|| std::env::var("SENTORI_TOKEN").ok())
        .context("token: pass --token or set SENTORI_TOKEN")?;

    let base = ingest_url
        .or_else(|| std::env::var("SENTORI_INGEST_URL").ok())
        .unwrap_or_else(|| DEFAULT_INGEST_URL.to_string());

    let collected = collect_files(&files)?;
    if collected.is_empty() {
        anyhow::bail!("no .js or .js.map files found in the given paths");
    }

    println!(
        "Uploading {} file(s) to release {release} via {base}...",
        collected.len()
    );

    let url = format!(
        "{}/admin/api/releases/{}/sourcemaps",
        base.trim_end_matches('/'),
        urlencoding(&release)
    );

    let client = reqwest::Client::new();
    let mut form = reqwest::multipart::Form::new();
    for path in &collected {
        let data = tokio::fs::read(path)
            .await
            .with_context(|| format!("reading {}", path.display()))?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let part = reqwest::multipart::Part::bytes(data).file_name(name.clone());
        form = form.part(name, part);
    }

    let resp = client
        .post(&url)
        .bearer_auth(&token)
        .multipart(form)
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("upload failed: {status} {body}");
    }

    println!("OK ({status}): {body}");
    Ok(())
}

fn collect_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            for entry in WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let s = path.to_string_lossy();
                if s.ends_with(".js") || s.ends_with(".js.map") {
                    out.push(path.to_path_buf());
                }
            }
        } else if p.is_file() {
            out.push(p.clone());
        } else {
            anyhow::bail!("path does not exist or is not a file/dir: {}", p.display());
        }
    }
    Ok(out)
}

pub(crate) fn urlencoding(s: &str) -> String {
    // Just escape what we need for the release segment (`@`, `+`, `/`, ` `).
    s.chars()
        .flat_map(|c| match c {
            '@' => "%40".chars().collect::<Vec<_>>(),
            '+' => "%2B".chars().collect::<Vec<_>>(),
            '/' => "%2F".chars().collect::<Vec<_>>(),
            ' ' => "%20".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}
