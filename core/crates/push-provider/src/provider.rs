//! The [`PushProvider`] trait — what every vendor impl
//! satisfies. dyn-dispatchable via `async-trait` because the
//! registry holds heterogeneous concrete types behind
//! `Arc<dyn PushProvider>`.

use async_trait::async_trait;

use crate::model::{Credential, NativeMessage, ProviderKind, ProviderResult, ValidateOutcome};

/// What every vendor impl implements.
///
/// `send` is the hot path; `validate` is the cheap auth-
/// challenge the dashboard's "is this credential green?"
/// button runs.
#[async_trait]
pub trait PushProvider: Send + Sync {
    /// Which vendor this impl handles. Stable across the
    /// process lifetime; the registry indexes on it.
    fn kind(&self) -> ProviderKind;

    /// Send one message to one native token. Returns the
    /// vendor's outcome — see [`crate::SendOutcome`] for the
    /// five typed paths.
    ///
    /// `native_token` is the raw provider-native string
    /// (APNs hex device id, FCM registration id, web
    /// subscription endpoint JSON, etc.). `env` is the APNs
    /// `production`/`sandbox` selector (other vendors ignore).
    async fn send(
        &self,
        cred: Credential<'_>,
        native_token: &str,
        env: Option<&str>,
        msg: &NativeMessage,
    ) -> ProviderResult;

    /// Fast credential-validation challenge. Should complete
    /// in < 1 s on the happy path. Default impl returns
    /// [`ValidateOutcome::NotImplemented`] so vendor crates
    /// that don't have a cheap challenge don't need a
    /// placeholder.
    async fn validate(&self, _cred: Credential<'_>) -> ValidateOutcome {
        ValidateOutcome::NotImplemented
    }
}
