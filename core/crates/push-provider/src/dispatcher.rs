//! [`PushDispatcher`] — composes tokens + credentials + provider
//! registry + S10 rate-limiter + DB quarantine.

use std::sync::Arc;

use sentori_rate_limiter::{Limiter, MemoryBackend, Policy, SystemClock};
use sentori_secrets_vault::Vault;
use sentori_workspace_identity::ProjectId;
use sqlx::PgPool;
use uuid::Uuid;

use crate::credentials::CredentialStore;
use crate::error::PushError;
use crate::model::{
    Credential, DeviceToken, NativeMessage, ProviderKind, ProviderResult, SendOutcome,
};
use crate::registry::ProviderRegistry;
use crate::tokens::DeviceTokenStore;

/// Rate-limit knobs for the dispatcher.
///
/// Defaults to a permissive 600 sends per minute per
/// (project, provider) so tests pass without coordinating
/// with the limiter; production wiring picks vendor-aware
/// values at [`PushDispatcher::new`] time.
#[derive(Debug, Clone, Copy)]
pub struct RateLimits {
    /// Max sends per minute per (project, provider) — the L1
    /// tier from legacy. L2 / L3 are follow-ups.
    pub sends_per_minute_per_project_provider: u32,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            sends_per_minute_per_project_provider: 600,
        }
    }
}

/// What [`PushDispatcher::dispatch`] targets.
#[derive(Debug, Clone)]
pub enum DispatchTarget {
    /// Send to one specific token (`push_tokens.id`). Errors
    /// with [`PushError::TokenNotFound`] if the row is missing
    /// or quarantined.
    SingleToken {
        /// The token row's id.
        token_id: Uuid,
    },
    /// Send to every live token for a `(project, kind)` pair.
    ProjectKind {
        /// The owning project.
        project_id: ProjectId,
        /// Which vendor.
        kind: ProviderKind,
    },
    /// Send to every live token for `(project, app_user_id)`
    /// across all providers (typically how a logged-in user is
    /// addressed — their iPhone + Android second device + web
    /// browser).
    ProjectUser {
        /// The owning project.
        project_id: ProjectId,
        /// App-side user id.
        app_user_id: String,
    },
}

/// Aggregate result of one `dispatch` call. The dispatcher
/// fans out across N tokens; this rolls up the per-token
/// outcomes for the caller's convenience.
#[derive(Debug, Clone)]
pub struct DispatchOutcome {
    /// Number of tokens targeted (pre-filter for quarantine /
    /// rate limit).
    pub targeted: usize,
    /// Number of `SendOutcome::Sent` results.
    pub sent: usize,
    /// Number of tokens skipped because they were quarantined
    /// (pre-dispatch).
    pub skipped_quarantined: usize,
    /// Number of tokens skipped because the L1 rate limiter
    /// said so.
    pub skipped_rate_limited: usize,
    /// Per-token results — `Ok((token_id, ProviderResult))`
    /// for an attempted send (regardless of outcome); `Err`
    /// for skipped tokens (rate-limited or pre-quarantined).
    pub per_token: Vec<PerTokenOutcome>,
}

impl DispatchOutcome {
    /// Convenience: how many tokens quarantined this call.
    #[must_use]
    pub fn newly_quarantined(&self) -> usize {
        self.per_token
            .iter()
            .filter(|o| {
                matches!(o, PerTokenOutcome::Sent { result, .. }
                if result.outcome.should_quarantine())
            })
            .count()
    }
}

/// Per-token outcome inside [`DispatchOutcome::per_token`].
#[derive(Debug, Clone)]
pub enum PerTokenOutcome {
    /// We invoked the provider's `send`. `result` is what
    /// came back.
    Sent {
        /// Which token row this attempt was for.
        token_id: Uuid,
        /// Provider's result (incl. outcome variant +
        /// vendor labels).
        result: ProviderResult,
    },
    /// Skipped because the row was pre-quarantined.
    SkippedQuarantined {
        /// Token row id.
        token_id: Uuid,
    },
    /// Skipped because the rate limiter said no.
    SkippedRateLimited {
        /// Token row id.
        token_id: Uuid,
    },
}

/// The public handle. Composes tokens + credentials +
/// registry + limiter + S12 vault.
#[derive(Clone)]
pub struct PushDispatcher {
    tokens: DeviceTokenStore,
    credentials: CredentialStore,
    registry: ProviderRegistry,
    limiter: Arc<Limiter<MemoryBackend, SystemClock>>,
}

impl std::fmt::Debug for PushDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PushDispatcher")
            .field("tokens", &self.tokens)
            .field("credentials", &self.credentials)
            .field("registry", &self.registry)
            .field("limiter", &"<Limiter>")
            .finish()
    }
}

impl PushDispatcher {
    /// Construct.
    ///
    /// `pool` powers both the token + credential stores; the
    /// `vault` is used for credential unseal at dispatch
    /// time. `limits` configures the L1 rate-limiter.
    ///
    /// # Panics
    ///
    /// If [`RateLimits::sends_per_minute_per_project_provider`]
    /// is `0`, the underlying [`sentori_rate_limiter::Policy::per_minute`]
    /// rejects construction and this constructor panics with
    /// a descriptive message. Pass a non-zero value.
    #[must_use]
    pub fn new(pool: PgPool, registry: ProviderRegistry, vault: Vault, limits: RateLimits) -> Self {
        let policy = Policy::per_minute(limits.sends_per_minute_per_project_provider)
            .expect("RateLimits.sends_per_minute_per_project_provider must be ≥ 1");
        let limiter = Arc::new(Limiter::new(MemoryBackend::new(), SystemClock, policy));
        Self {
            tokens: DeviceTokenStore::new(pool.clone()),
            credentials: CredentialStore::new(pool, vault),
            registry,
            limiter,
        }
    }

    /// Borrow the token store (consumer crates use this to
    /// register / list tokens before dispatching).
    #[must_use]
    pub const fn tokens(&self) -> &DeviceTokenStore {
        &self.tokens
    }

    /// Borrow the credential store.
    #[must_use]
    pub const fn credentials(&self) -> &CredentialStore {
        &self.credentials
    }

    /// Borrow the registry.
    #[must_use]
    pub const fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    /// Borrow the L1 rate limiter (for telemetry, manual
    /// reset, etc.).
    #[must_use]
    pub fn limiter(&self) -> &Limiter<MemoryBackend, SystemClock> {
        &self.limiter
    }

    /// Dispatch a message.
    ///
    /// 1. Resolve `target` → set of live `DeviceToken`s.
    /// 2. For each token: load credentials (per
    ///    `(project, kind)` — cached per call), check L1 rate
    ///    limit, invoke `PushProvider::send`, on
    ///    `PermanentlyInvalidToken` outcome stamp
    ///    `push_tokens.quarantined_at`.
    /// 3. Aggregate into [`DispatchOutcome`].
    ///
    /// # Errors
    ///
    /// - [`PushError::TokenNotFound`] for `SingleToken` with
    ///   no matching live row.
    /// - [`PushError::CredentialsMissing`] if no
    ///   `push_credentials` row for any of the dispatched
    ///   `(project, kind)` pairs.
    /// - [`PushError::ProviderNotRegistered`] if no impl in
    ///   the registry handles the requested kind.
    /// - [`PushError::CredentialUnseal`] on vault failure.
    /// - [`PushError::Db`] on DB failure.
    pub async fn dispatch(
        &self,
        target: DispatchTarget,
        msg: NativeMessage,
    ) -> Result<DispatchOutcome, PushError> {
        let tokens = self.resolve_target(&target).await?;
        let targeted = tokens.len();
        let mut per_token = Vec::with_capacity(targeted);
        let mut sent = 0usize;
        let mut skipped_quarantined = 0usize;
        let mut skipped_rate_limited = 0usize;

        for token in tokens {
            // Quarantined → skip (live-token filter at
            // resolve time should make this dead code, but
            // SingleToken bypasses the filter so keep the
            // guard).
            if token.is_quarantined() {
                skipped_quarantined += 1;
                per_token.push(PerTokenOutcome::SkippedQuarantined { token_id: token.id });
                continue;
            }

            // Rate-limit key = "<project>:<kind>".
            let rate_key = rate_key(token.project_id, token.kind);
            let verdict = self.limiter.check(&rate_key);
            if verdict.is_limited() {
                skipped_rate_limited += 1;
                per_token.push(PerTokenOutcome::SkippedRateLimited { token_id: token.id });
                continue;
            }

            // Resolve provider impl.
            let provider = self.registry.get(token.kind).ok_or_else(|| {
                PushError::ProviderNotRegistered(token.kind.as_db_str().to_string())
            })?;

            // Load + unseal credentials for this (project, kind).
            // NOTE: This is per-token; for multi-token dispatches
            // to the same (project, kind) we re-load + re-unseal
            // every iteration. v0.1 keeps it simple; the K7
            // follow-up adding a per-call credential cache lands
            // alongside the first real provider impl when the
            // crypto cost actually matters.
            let cred = self
                .credentials
                .load(token.project_id, token.kind)
                .await?
                .ok_or_else(|| PushError::CredentialsMissing {
                    project_id: token.project_id.into_uuid(),
                    kind: token.kind.as_db_str().to_string(),
                })?;
            let credential = Credential {
                config: &cred.config,
                secret_payload: &cred.secret_payload,
            };

            // Send.
            let result = provider
                .send(credential, &token.native_token, token.env.as_deref(), &msg)
                .await;

            // Quarantine on PermanentlyInvalidToken outcome.
            if result.outcome.should_quarantine() {
                let reason = match &result.outcome {
                    SendOutcome::PermanentlyInvalidToken => {
                        format!(
                            "PermanentlyInvalidToken via {} (label={})",
                            token.kind.as_db_str(),
                            result.provider_outcome_label
                        )
                    }
                    _ => "unknown".to_string(),
                };
                if let Err(e) = self.tokens.quarantine(token.id, &reason).await {
                    tracing::warn!(error = %e, token_id = %token.id, "quarantine write failed");
                }
            }

            if matches!(result.outcome, SendOutcome::Sent) {
                sent += 1;
            }
            per_token.push(PerTokenOutcome::Sent {
                token_id: token.id,
                result,
            });
        }

        Ok(DispatchOutcome {
            targeted,
            sent,
            skipped_quarantined,
            skipped_rate_limited,
            per_token,
        })
    }

    async fn resolve_target(&self, target: &DispatchTarget) -> Result<Vec<DeviceToken>, PushError> {
        match target {
            DispatchTarget::SingleToken { token_id } => {
                let tok = self
                    .tokens
                    .find(*token_id)
                    .await?
                    .ok_or(PushError::TokenNotFound(*token_id))?;
                if tok.is_quarantined() {
                    return Err(PushError::TokenNotFound(*token_id));
                }
                Ok(vec![tok])
            }
            DispatchTarget::ProjectKind { project_id, kind } => {
                self.tokens.list_live(*project_id, *kind).await
            }
            DispatchTarget::ProjectUser {
                project_id,
                app_user_id,
            } => self.tokens.list_for_user(*project_id, app_user_id).await,
        }
    }
}

fn rate_key(project_id: ProjectId, kind: ProviderKind) -> String {
    format!("{}:{}", project_id.into_uuid(), kind.as_db_str())
}
