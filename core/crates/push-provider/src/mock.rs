//! [`MockProvider`] — for tests + consumer integration suites.

use std::sync::{Mutex, PoisonError};

use async_trait::async_trait;

use crate::model::{
    Credential, NativeMessage, ProviderKind, ProviderResult, SendOutcome, ValidateOutcome,
};
use crate::provider::PushProvider;

/// Test double. Holds a fixed [`SendOutcome`] / [`ValidateOutcome`]
/// the caller picks, and counts every `send` / `validate` call.
///
/// Pattern:
///
/// ```
/// use std::sync::Arc;
/// use sentori_push_provider::{MockProvider, ProviderKind, SendOutcome, PushProvider};
///
/// let mock = Arc::new(MockProvider::new(
///     ProviderKind::Apns,
///     SendOutcome::Sent,
///     "MOCK_OK",
/// ));
/// // … register into ProviderRegistry, drive the dispatcher …
/// assert_eq!(mock.send_calls(), 0);
/// ```
pub struct MockProvider {
    kind: ProviderKind,
    outcome: Mutex<SendOutcome>,
    label: Mutex<String>,
    validate: Mutex<ValidateOutcome>,
    send_calls: Mutex<u64>,
    validate_calls: Mutex<u64>,
    duration_ms: i32,
}

impl MockProvider {
    /// Build a mock that always returns `outcome` from `send`,
    /// tagged with `label` for the audit row, and
    /// [`ValidateOutcome::Ok`] from `validate`.
    #[must_use]
    pub fn new(kind: ProviderKind, outcome: SendOutcome, label: impl Into<String>) -> Self {
        Self {
            kind,
            outcome: Mutex::new(outcome),
            label: Mutex::new(label.into()),
            validate: Mutex::new(ValidateOutcome::Ok),
            send_calls: Mutex::new(0),
            validate_calls: Mutex::new(0),
            duration_ms: 1,
        }
    }

    /// Convenience: "always succeed" mock with kind defaulting
    /// to APNs and label `"MOCK_OK"`.
    #[must_use]
    pub fn always(outcome: SendOutcome) -> Self {
        Self::new(ProviderKind::Apns, outcome, "MOCK_OK")
    }

    /// Swap the `send` outcome (allows tests to drive a mock
    /// through multiple phases without rebuilding).
    pub fn set_outcome(&self, outcome: SendOutcome, label: impl Into<String>) {
        *self.outcome.lock().unwrap_or_else(PoisonError::into_inner) = outcome;
        *self.label.lock().unwrap_or_else(PoisonError::into_inner) = label.into();
    }

    /// Swap the `validate` outcome.
    pub fn set_validate(&self, outcome: ValidateOutcome) {
        *self.validate.lock().unwrap_or_else(PoisonError::into_inner) = outcome;
    }

    /// Number of `send` calls made so far.
    #[must_use]
    pub fn send_calls(&self) -> u64 {
        *self
            .send_calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Number of `validate` calls made so far.
    #[must_use]
    pub fn validate_calls(&self) -> u64 {
        *self
            .validate_calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl PushProvider for MockProvider {
    fn kind(&self) -> ProviderKind {
        self.kind
    }

    async fn send(
        &self,
        _cred: Credential<'_>,
        _native_token: &str,
        _env: Option<&str>,
        _msg: &NativeMessage,
    ) -> ProviderResult {
        *self
            .send_calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner) += 1;
        let outcome = self
            .outcome
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let label = self
            .label
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        ProviderResult {
            outcome,
            provider_outcome_label: label,
            provider_status: Some(200),
            provider_body: None,
            duration_ms: self.duration_ms,
        }
    }

    async fn validate(&self, _cred: Credential<'_>) -> ValidateOutcome {
        *self
            .validate_calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner) += 1;
        self.validate
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}
