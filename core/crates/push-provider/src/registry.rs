//! [`ProviderRegistry`] — `(kind → Arc<dyn PushProvider>)` map.

use std::collections::HashMap;
use std::sync::Arc;

use crate::model::ProviderKind;
use crate::provider::PushProvider;

/// Process-wide registry of vendor impls.
///
/// Built once at startup. Cheap to clone (each entry is
/// `Arc`-shared). Missing providers cause dispatch to surface
/// [`crate::PushError::ProviderNotRegistered`] — useful
/// during K7's iterative ship: register only the impls a
/// deployment actually uses.
#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: HashMap<ProviderKind, Arc<dyn PushProvider>>,
}

impl ProviderRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register `provider` under `kind`. Replaces any
    /// previously-registered impl for that kind (useful in
    /// tests where you swap [`crate::MockProvider`] flavours).
    pub fn register(&mut self, kind: ProviderKind, provider: Arc<dyn PushProvider>) {
        self.providers.insert(kind, provider);
    }

    /// Look up an impl. `None` when no provider is registered
    /// for this kind.
    #[must_use]
    pub fn get(&self, kind: ProviderKind) -> Option<&Arc<dyn PushProvider>> {
        self.providers.get(&kind)
    }

    /// True if no providers are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Number of registered providers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("kinds", &self.providers.keys().copied().collect::<Vec<_>>())
            .finish()
    }
}
