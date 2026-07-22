//! Concrete transports — wire bytes to a destination.
//!
//! The [`Notifier`] trait is `async_trait`-powered so the
//! registry on [`crate::NotifierService`] can hold
//! `Arc<dyn Notifier>` (mixing concrete transports per
//! channel). Same rationale as K7's `PushProvider`.

mod email;
mod mock;
mod webhook;

use async_trait::async_trait;

use crate::error::TransportError;
use crate::model::{Channel, Notification};

pub use email::{EmailConfig, EmailTransport, SmtpTls};
pub use mock::{MockInbox, MockTransport};
pub use webhook::WebhookTransport;

/// Anything that can push a [`Notification`] over its
/// channel. `dyn`-safe via `async_trait`.
#[async_trait]
pub trait Notifier: Send + Sync + std::fmt::Debug {
    /// Which channel this transport serves. The
    /// [`crate::NotifierService`] uses this as the registry
    /// key.
    fn channel(&self) -> Channel;

    /// Push the notification. Implementations must NOT
    /// touch the `delivery_log` table — that's
    /// [`crate::NotifierService::dispatch`]'s job.
    ///
    /// # Errors
    ///
    /// Per-transport [`TransportError`] variant.
    async fn send(&self, n: &Notification) -> Result<(), TransportError>;
}
