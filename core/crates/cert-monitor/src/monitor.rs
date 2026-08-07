//! [`CertMonitor`] — the public handle.

use std::time::Duration;

use sentori_workspace_identity::{ProjectId, UserId};
use serde::Deserialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::error::CertMonitorError;
use crate::model::{
    CertObservation, PollOutcome, WatchedDomain, row_to_observation, row_to_watched,
};

/// Default `base_url` for the public CT log.
pub const DEFAULT_BASE_URL: &str = "https://crt.sh";

/// Default per-call HTTP timeout. crt.sh occasionally takes
/// 30+ seconds on a popular domain; 20s plays "give up + retry
/// next tick" rather than block the loop indefinitely.
pub const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 20;

/// Max stored bytes of `name_value` (SANs). crt.sh sometimes
/// returns 8+ KB SAN blobs on multi-tenant CDNs.
const MAX_NAME_VALUE_BYTES: usize = 8000;

/// Public handle.
#[derive(Debug, Clone)]
pub struct CertMonitor {
    pool: PgPool,
    client: reqwest::Client,
    base_url: String,
}

impl CertMonitor {
    /// Construct with sensible defaults (crt.sh + 20s timeout).
    ///
    /// # Panics
    ///
    /// The underlying [`reqwest::Client::builder`] only fails
    /// to construct on system-level TLS misconfiguration
    /// (missing root certs etc.). v0.1 panics in that case
    /// rather than fail-soft — the cert monitor isn't useful
    /// without a working HTTPS client.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
            .user_agent(format!(
                "sentori-cert-monitor/{} (+https://sentori.golia.jp)",
                env!("CARGO_PKG_VERSION"),
            ))
            .build()
            .expect("reqwest client builder must succeed (system TLS)");
        Self {
            pool,
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// Swap the base URL (chained builder). Used in tests to
    /// inject a local mock crt.sh, OR in production to point
    /// at a self-hosted CT log mirror.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Swap the [`reqwest::Client`] (chained builder). Useful
    /// when the consumer crate already has a shared client
    /// pool tuned for its own settings.
    #[must_use]
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// Borrow the pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Borrow the base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // ── watch CRUD ──────────────────────────────────────────

    /// Add a watch. Idempotent on `(project, domain)` — re-adding
    /// returns the existing row id without modification.
    ///
    /// # Errors
    ///
    /// - [`CertMonitorError::ProjectNotFound`] on FK violation.
    /// - [`CertMonitorError::InvalidDomain`] for empty / >253
    ///   chars / non-ASCII.
    /// - [`CertMonitorError::Db`] on backend failure.
    pub async fn add_watch(
        &self,
        project_id: ProjectId,
        domain: &str,
        added_by: Option<UserId>,
    ) -> Result<Uuid, CertMonitorError> {
        validate_domain(domain)?;
        let normalised = domain.trim().to_ascii_lowercase();
        let new_id = Uuid::now_v7();
        // workspace_id is denormalized from projects via subquery
        // so the caller's PgPool can be either superuser (janitor)
        // or workspace-scoped (CRUD) without API change.
        let row: Option<(Uuid,)> = sqlx::query_as(
            "INSERT INTO cert_watch_domains (id, workspace_id, project_id, domain, added_by) \
             SELECT $1, p.workspace_id, $2, $3, $4 FROM projects p WHERE p.id = $2 \
             ON CONFLICT (project_id, domain) DO UPDATE SET added_by = COALESCE(EXCLUDED.added_by, cert_watch_domains.added_by) \
             RETURNING id",
        )
        .bind(new_id)
        .bind(project_id.into_uuid())
        .bind(&normalised)
        .bind(added_by.map(UserId::into_uuid))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| translate_fk(e, project_id))?;
        // An unknown project makes the driving SELECT match zero rows, so the
        // INSERT writes nothing and Postgres never raises the FK violation
        // `translate_fk` is waiting for. Absence of a RETURNING row is the
        // only signal we get.
        let row = row.ok_or_else(|| CertMonitorError::ProjectNotFound(project_id.into_uuid()))?;
        Ok(row.0)
    }

    /// Delete a watch. Idempotent (no error if missing).
    ///
    /// # Errors
    ///
    /// [`CertMonitorError::Db`] on backend failure.
    pub async fn remove_watch(
        &self,
        project_id: ProjectId,
        domain: &str,
    ) -> Result<(), CertMonitorError> {
        let normalised = domain.trim().to_ascii_lowercase();
        sqlx::query("DELETE FROM cert_watch_domains WHERE project_id = $1 AND domain = $2")
            .bind(project_id.into_uuid())
            .bind(&normalised)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// List all watched domains for a project.
    ///
    /// # Errors
    ///
    /// [`CertMonitorError::Db`] on backend failure.
    pub async fn list_watched(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<WatchedDomain>, CertMonitorError> {
        let rows = sqlx::query(
            r"
            SELECT id, project_id, domain, added_by, added_at, last_polled_at
            FROM cert_watch_domains
            WHERE project_id = $1
            ORDER BY added_at ASC
            ",
        )
        .bind(project_id.into_uuid())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_watched).collect()
    }

    // ── poll ────────────────────────────────────────────────

    /// Iterate every watched (project, domain) pair, fan-out
    /// poll, accumulate new observations.
    ///
    /// Per-domain failures are recorded in
    /// [`PollOutcome::per_domain_errors`] but do NOT abort
    /// the loop — one bad domain shouldn't block the rest.
    /// Caller logs the errors + decides retry policy.
    ///
    /// # Errors
    ///
    /// [`CertMonitorError::Db`] on the initial watch-domain
    /// fetch failure. Per-domain failures are captured in the
    /// returned outcome, not surfaced as `Err`.
    pub async fn poll_once(&self) -> Result<PollOutcome, CertMonitorError> {
        let watches: Vec<(Uuid, String)> =
            sqlx::query_as("SELECT project_id, domain FROM cert_watch_domains")
                .fetch_all(&self.pool)
                .await?;

        let mut outcome = PollOutcome {
            domains_polled: watches.len(),
            ..Default::default()
        };

        for (pid_raw, domain) in watches {
            let project_id = ProjectId::from_uuid(pid_raw);
            match self.poll_domain(project_id, &domain).await {
                Ok(new) => {
                    outcome.domains_ok += 1;
                    outcome.new_observations.extend(new);
                }
                Err(e) => {
                    let msg = e.to_string();
                    tracing::warn!(error = %e, %project_id, %domain, "cert poll failed");
                    outcome.per_domain_errors.push((domain, msg));
                }
            }
        }
        Ok(outcome)
    }

    /// Poll a single (project, domain) pair. Returns the new
    /// observations (i.e. ones the `ON CONFLICT DO NOTHING`
    /// actually inserted; dupes return zero rows).
    ///
    /// # Errors
    ///
    /// - [`CertMonitorError::HttpTransport`] on network /
    ///   timeout.
    /// - [`CertMonitorError::UpstreamStatus`] on non-2xx.
    /// - [`CertMonitorError::MalformedResponse`] on JSON
    ///   shape failure.
    /// - [`CertMonitorError::ProjectNotFound`] if the project
    ///   doesn't exist.
    /// - [`CertMonitorError::Db`] on backend failure.
    pub async fn poll_domain(
        &self,
        project_id: ProjectId,
        domain: &str,
    ) -> Result<Vec<CertObservation>, CertMonitorError> {
        validate_domain(domain)?;
        let normalised = domain.trim().to_ascii_lowercase();

        // `%25.<domain>` after URL-encoding = `%.<domain>` raw
        // = crt.sh wildcard-subdomain match.
        let url = format!(
            "{base}/?q=%25.{domain}&output=json",
            base = self.base_url.trim_end_matches('/'),
            domain = urlencoding::encode(&normalised),
        );
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(CertMonitorError::UpstreamStatus {
                status: resp.status().as_u16(),
                domain: normalised,
            });
        }
        let body = resp.bytes().await?;
        let parsed: Vec<CrtShCert> = serde_json::from_slice(&body)
            .map_err(|e| CertMonitorError::MalformedResponse(format!("json parse: {e}")))?;

        // The observation INSERT below derives workspace_id from a SELECT over
        // `projects`, so an unknown project matches zero rows and writes
        // nothing — indistinguishable from the `ON CONFLICT DO NOTHING` dupe
        // path, and never an FK violation. Probe once up front instead.
        let project_exists: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM projects WHERE id = $1")
                .bind(project_id.into_uuid())
                .fetch_optional(&self.pool)
                .await?;
        if project_exists.is_none() {
            return Err(CertMonitorError::ProjectNotFound(project_id.into_uuid()));
        }

        let mut new = Vec::with_capacity(parsed.len());
        for c in parsed {
            let not_before = parse_crt_sh_ts(&c.not_before)
                .map_err(|e| CertMonitorError::MalformedResponse(format!("not_before: {e}")))?;
            let not_after = parse_crt_sh_ts(&c.not_after)
                .map_err(|e| CertMonitorError::MalformedResponse(format!("not_after: {e}")))?;
            let id = Uuid::now_v7();
            let inserted = sqlx::query(
                r"
                INSERT INTO cert_observations
                    (id, workspace_id, project_id, domain, cert_id,
                     common_name, name_value, issuer_name,
                     not_before, not_after)
                SELECT $1, p.workspace_id, $2, $3, $4, $5, $6, $7, $8, $9
                FROM projects p WHERE p.id = $2
                ON CONFLICT (project_id, cert_id) DO NOTHING
                RETURNING id, project_id, domain, cert_id,
                          common_name, name_value, issuer_name,
                          not_before, not_after, observed_at
                ",
            )
            .bind(id)
            .bind(project_id.into_uuid())
            .bind(&normalised)
            .bind(c.id)
            .bind(c.common_name.as_deref())
            .bind(
                c.name_value
                    .as_deref()
                    .map(|s| truncate_utf8(s, MAX_NAME_VALUE_BYTES)),
            )
            .bind(&c.issuer_name)
            .bind(not_before)
            .bind(not_after)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| translate_fk(e, project_id))?;
            if let Some(row) = inserted {
                new.push(row_to_observation(&row)?);
            }
        }

        // Stamp last_polled_at on the watch row so the
        // dashboard "last polled" indicator works.
        sqlx::query(
            "UPDATE cert_watch_domains SET last_polled_at = now() \
             WHERE project_id = $1 AND domain = $2",
        )
        .bind(project_id.into_uuid())
        .bind(&normalised)
        .execute(&self.pool)
        .await?;

        Ok(new)
    }

    // ── read ────────────────────────────────────────────────

    /// List observations for a project that landed after
    /// `since`. Sorted by `observed_at` descending.
    ///
    /// # Errors
    ///
    /// [`CertMonitorError::Db`] on backend failure.
    pub async fn list_observations(
        &self,
        project_id: ProjectId,
        since: OffsetDateTime,
    ) -> Result<Vec<CertObservation>, CertMonitorError> {
        let rows = sqlx::query(
            r"
            SELECT id, project_id, domain, cert_id,
                   common_name, name_value, issuer_name,
                   not_before, not_after, observed_at
            FROM cert_observations
            WHERE project_id = $1 AND observed_at >= $2
            ORDER BY observed_at DESC
            ",
        )
        .bind(project_id.into_uuid())
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_observation).collect()
    }

    /// List observations expiring within `within` of `now`,
    /// sorted by `not_after` ascending (soonest first).
    ///
    /// # Errors
    ///
    /// [`CertMonitorError::Db`] on backend failure.
    pub async fn list_expiring(
        &self,
        project_id: ProjectId,
        now: OffsetDateTime,
        within: time::Duration,
    ) -> Result<Vec<CertObservation>, CertMonitorError> {
        let cutoff = now + within;
        let rows = sqlx::query(
            r"
            SELECT id, project_id, domain, cert_id,
                   common_name, name_value, issuer_name,
                   not_before, not_after, observed_at
            FROM cert_observations
            WHERE project_id = $1 AND not_after <= $2 AND not_after >= $3
            ORDER BY not_after ASC
            ",
        )
        .bind(project_id.into_uuid())
        .bind(cutoff)
        .bind(now) // skip already-expired certs by default
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_observation).collect()
    }
}

// ── crt.sh wire ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CrtShCert {
    id: i64,
    #[serde(default)]
    common_name: Option<String>,
    #[serde(default)]
    name_value: Option<String>,
    issuer_name: String,
    /// crt.sh returns naive datetimes without zone; treat as
    /// UTC. Format: `2024-01-01T00:00:00`.
    not_before: String,
    not_after: String,
}

/// crt.sh emits zoneless ISO 8601 (`2024-01-01T00:00:00`).
/// Append `Z` then parse via RFC 3339. Exposed via test
/// proptests; called only here in production.
pub(crate) fn parse_crt_sh_ts(s: &str) -> Result<OffsetDateTime, time::error::Parse> {
    let with_z = if s.ends_with('Z') {
        s.to_string()
    } else {
        format!("{s}Z")
    };
    OffsetDateTime::parse(&with_z, &Rfc3339)
}

/// Truncate `s` at `max` bytes without slicing inside a UTF-8
/// codepoint.
pub(crate) fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn validate_domain(domain: &str) -> Result<(), CertMonitorError> {
    let trimmed = domain.trim();
    if trimmed.is_empty() {
        return Err(CertMonitorError::InvalidDomain(
            "domain must not be empty".into(),
        ));
    }
    if trimmed.len() > 253 {
        return Err(CertMonitorError::InvalidDomain(format!(
            "domain too long: {}",
            trimmed.len()
        )));
    }
    if !trimmed.is_ascii() {
        return Err(CertMonitorError::InvalidDomain(
            "domain must be ASCII (use punycode for IDN)".into(),
        ));
    }
    if trimmed.starts_with('.') || trimmed.ends_with('.') {
        return Err(CertMonitorError::InvalidDomain(
            "domain must not start or end with a dot".into(),
        ));
    }
    Ok(())
}

fn translate_fk(err: sqlx::Error, project_id: ProjectId) -> CertMonitorError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23503")
    {
        return CertMonitorError::ProjectNotFound(project_id.into_uuid());
    }
    CertMonitorError::Db(err)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_crt_sh_ts_accepts_zoneless() {
        let ts = parse_crt_sh_ts("2024-01-15T03:04:05").unwrap();
        assert_eq!(ts.year(), 2024);
    }

    #[test]
    fn parse_crt_sh_ts_accepts_z_suffix() {
        let ts = parse_crt_sh_ts("2024-01-15T03:04:05Z").unwrap();
        assert_eq!(ts.year(), 2024);
    }

    #[test]
    fn truncate_utf8_handles_boundary() {
        // "é" is 2 bytes in UTF-8.
        let s = "abcé";
        // Asking for 4 bytes would slice mid-codepoint; should
        // back off to 3.
        let out = truncate_utf8(s, 4);
        assert!(out.is_char_boundary(out.len()));
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn truncate_utf8_passes_through_short_string() {
        assert_eq!(truncate_utf8("ab", 8), "ab");
    }

    #[test]
    fn validate_domain_accepts_apex() {
        assert!(validate_domain("example.com").is_ok());
    }

    #[test]
    fn validate_domain_rejects_empty() {
        assert!(matches!(
            validate_domain("   "),
            Err(CertMonitorError::InvalidDomain(_))
        ));
    }

    #[test]
    fn validate_domain_rejects_non_ascii() {
        assert!(matches!(
            validate_domain("café.com"),
            Err(CertMonitorError::InvalidDomain(_))
        ));
    }

    #[test]
    fn validate_domain_rejects_leading_dot() {
        assert!(matches!(
            validate_domain(".example.com"),
            Err(CertMonitorError::InvalidDomain(_))
        ));
    }
}
