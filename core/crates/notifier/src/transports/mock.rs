//! [`MockTransport`] — in-memory recorder for tests.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::error::TransportError;
use crate::model::{Channel, Notification};
use crate::transports::Notifier;

/// Shared inbox handle. Cloning yields another handle to the
/// same underlying `Vec<Notification>` — so tests can read
/// the inbox after the transport has consumed its own copy.
#[derive(Clone, Debug, Default)]
pub struct MockInbox {
    inner: Arc<Mutex<Vec<Notification>>>,
}

impl MockInbox {
    /// New empty inbox.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many notifications have been delivered.
    ///
    /// # Panics
    ///
    /// Inner mutex poison — shouldn't happen in practice.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().expect("mock inbox lock").len()
    }

    /// True when nothing's been delivered yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Snapshot the recorded notifications.
    ///
    /// # Panics
    ///
    /// Inner mutex poison — shouldn't happen in practice.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Notification> {
        self.inner.lock().expect("mock inbox lock").clone()
    }

    /// Reset the inbox to empty.
    ///
    /// # Panics
    ///
    /// Inner mutex poison — shouldn't happen in practice.
    pub fn clear(&self) {
        self.inner.lock().expect("mock inbox lock").clear();
    }
}

/// In-memory transport. Captures every sent notification into
/// a [`MockInbox`]; optional `fail_on_recipient` causes
/// matching deliveries to return [`TransportError::Mock`]
/// (drives the dispatch-failure code path in tests).
#[derive(Clone, Debug)]
pub struct MockTransport {
    inbox: MockInbox,
    fail_on_recipient: Option<String>,
}

impl MockTransport {
    /// Build with an empty inbox and no failure injection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inbox: MockInbox::new(),
            fail_on_recipient: None,
        }
    }

    /// Build from a pre-shared inbox so tests can read after
    /// the transport is wrapped in `Arc<dyn Notifier>`.
    #[must_use]
    pub const fn with_inbox(inbox: MockInbox) -> Self {
        Self {
            inbox,
            fail_on_recipient: None,
        }
    }

    /// Cause `send` to fail with [`TransportError::Mock`]
    /// when `n.recipient == recipient`. Idempotent — repeated
    /// calls overwrite.
    #[must_use]
    pub fn failing_for(mut self, recipient: impl Into<String>) -> Self {
        self.fail_on_recipient = Some(recipient.into());
        self
    }

    /// Borrow the inbox handle.
    #[must_use]
    pub const fn inbox(&self) -> &MockInbox {
        &self.inbox
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Notifier for MockTransport {
    fn channel(&self) -> Channel {
        Channel::Mock
    }

    async fn send(&self, n: &Notification) -> Result<(), TransportError> {
        if self
            .fail_on_recipient
            .as_deref()
            .is_some_and(|r| r == n.recipient)
        {
            return Err(TransportError::Mock(format!(
                "injected failure for recipient {}",
                n.recipient
            )));
        }
        self.inbox
            .inner
            .lock()
            .expect("mock inbox lock")
            .push(n.clone());
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use sentori_workspace_identity::WorkspaceId;

    use super::*;

    #[tokio::test]
    async fn mock_records_send() {
        let t = MockTransport::new();
        let n = Notification::new(WorkspaceId::new(), Channel::Mock, "x", "s", "b");
        t.send(&n).await.unwrap();
        assert_eq!(t.inbox().len(), 1);
        assert_eq!(t.inbox().snapshot()[0].subject, "s");
    }

    #[tokio::test]
    async fn mock_failure_injection() {
        let t = MockTransport::new().failing_for("bad");
        let n = Notification::new(WorkspaceId::new(), Channel::Mock, "bad", "s", "b");
        assert!(matches!(t.send(&n).await, Err(TransportError::Mock(_))));
    }

    #[tokio::test]
    async fn mock_passes_non_matching_recipient() {
        let t = MockTransport::new().failing_for("bad");
        let n = Notification::new(WorkspaceId::new(), Channel::Mock, "ok", "s", "b");
        t.send(&n).await.unwrap();
        assert_eq!(t.inbox().len(), 1);
    }

    #[test]
    fn clear_empties_inbox() {
        let inbox = MockInbox::new();
        let t = MockTransport::with_inbox(inbox.clone());
        // Add directly via Arc to avoid async fan-out in
        // this sync test.
        t.inbox.inner.lock().unwrap().push(Notification::new(
            WorkspaceId::new(),
            Channel::Mock,
            "x",
            "s",
            "b",
        ));
        assert_eq!(inbox.len(), 1);
        inbox.clear();
        assert!(inbox.is_empty());
    }
}
