//! In-memory adapter for tests.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::error::IntegrationError;
use crate::model::{ConnectMode, ExternalRef, IssueContext, IssueLifecycleEvent};
use crate::traits::IntegrationAdapter;

/// One recorded adapter invocation.
#[derive(Debug, Clone, PartialEq)]
pub enum RecordedCall {
    /// `accept_manual_config(form)`.
    AcceptManualConfig(Value),
    /// `exchange_code(code, redirect_uri)`.
    ExchangeCode {
        /// Code value.
        code: String,
        /// Redirect URI value.
        redirect_uri: String,
    },
    /// `create_issue(config, ctx.issue_id)`.
    CreateIssue {
        /// Issue id from the [`IssueContext`].
        issue_id: uuid::Uuid,
    },
    /// `update_status(config, external_id, event)`.
    UpdateStatus {
        /// Same `external_id` the service passed in.
        external_id: String,
        /// Which lifecycle event.
        event: IssueLifecycleEvent,
    },
}

/// Shared history handle. Cloning yields another reader of
/// the same underlying `Vec<RecordedCall>`.
#[derive(Clone, Debug, Default)]
pub struct MockHistory {
    inner: Arc<Mutex<Vec<RecordedCall>>>,
}

impl MockHistory {
    /// New empty history.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many calls recorded.
    ///
    /// # Panics
    ///
    /// Inner mutex poison — shouldn't happen.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().expect("mock history lock").len()
    }

    /// True when nothing recorded yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Snapshot of recorded calls.
    ///
    /// # Panics
    ///
    /// Inner mutex poison — shouldn't happen.
    #[must_use]
    pub fn snapshot(&self) -> Vec<RecordedCall> {
        self.inner.lock().expect("mock history lock").clone()
    }
}

/// Deterministic test adapter — records every call, can be
/// configured to fail on specific operations or return a
/// canned `ExternalRef`.
#[derive(Clone, Debug)]
pub struct MockAdapter {
    kind: &'static str,
    history: MockHistory,
    fail_on: Option<MockFailMode>,
    canned_ref: ExternalRef,
}

/// What to fail on (drives the error path in service tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockFailMode {
    /// `create_issue` returns `Upstream`.
    Create,
    /// `update_status` returns `Upstream`.
    Update,
}

impl MockAdapter {
    /// Build with the default kind `"mock"` and an empty
    /// history. Use [`Self::with_kind`] if a test needs
    /// multiple distinct mock adapters registered at once.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kind: "mock",
            history: MockHistory::new(),
            fail_on: None,
            canned_ref: ExternalRef {
                external_id: "mock-id".into(),
                external_url: "https://mock.example/item".into(),
            },
        }
    }

    /// Override the kind (static — caller passes a `&'static
    /// str` literal).
    #[must_use]
    pub fn with_kind(mut self, kind: &'static str) -> Self {
        self.kind = kind;
        self
    }

    /// Re-use an existing history handle so the test can
    /// read after wrapping in `Arc<dyn IntegrationAdapter>`.
    #[must_use]
    pub fn with_history(mut self, history: MockHistory) -> Self {
        self.history = history;
        self
    }

    /// Inject a deterministic failure mode.
    #[must_use]
    pub fn failing(mut self, mode: MockFailMode) -> Self {
        self.fail_on = Some(mode);
        self
    }

    /// Override the canned [`ExternalRef`] returned from
    /// `create_issue`.
    #[must_use]
    pub fn with_external(mut self, r: ExternalRef) -> Self {
        self.canned_ref = r;
        self
    }

    /// Borrow the history handle.
    #[must_use]
    pub const fn history(&self) -> &MockHistory {
        &self.history
    }

    fn record(&self, call: RecordedCall) {
        self.history
            .inner
            .lock()
            .expect("mock history lock")
            .push(call);
    }
}

impl Default for MockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IntegrationAdapter for MockAdapter {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn is_configured(&self) -> bool {
        true
    }

    fn connect_mode(&self) -> ConnectMode {
        ConnectMode::Manual
    }

    fn oauth_authorise_url(&self, state: &str, redirect_uri: &str) -> String {
        format!("https://mock.example/oauth?state={state}&redirect={redirect_uri}")
    }

    async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
    ) -> Result<Value, IntegrationError> {
        self.record(RecordedCall::ExchangeCode {
            code: code.to_string(),
            redirect_uri: redirect_uri.to_string(),
        });
        Ok(json!({"access_token": "mock-token"}))
    }

    async fn accept_manual_config(&self, form: Value) -> Result<Value, IntegrationError> {
        self.record(RecordedCall::AcceptManualConfig(form.clone()));
        Ok(form)
    }

    async fn create_issue(
        &self,
        _config: &Value,
        ctx: &IssueContext,
    ) -> Result<ExternalRef, IntegrationError> {
        self.record(RecordedCall::CreateIssue {
            issue_id: ctx.issue_id,
        });
        if self.fail_on == Some(MockFailMode::Create) {
            return Err(IntegrationError::Upstream(
                "mock create_issue failure".into(),
            ));
        }
        Ok(self.canned_ref.clone())
    }

    async fn update_status(
        &self,
        _config: &Value,
        external_id: &str,
        event: IssueLifecycleEvent,
    ) -> Result<(), IntegrationError> {
        self.record(RecordedCall::UpdateStatus {
            external_id: external_id.to_string(),
            event,
        });
        if self.fail_on == Some(MockFailMode::Update) {
            return Err(IntegrationError::Upstream(
                "mock update_status failure".into(),
            ));
        }
        Ok(())
    }
}
