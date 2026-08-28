//! Shared handler state.

use std::sync::{Arc, Mutex};
use torimemo_core::Store;
use torimemo_embed::Provider;

/// What every handler can reach.
///
/// The store is behind a `Mutex` rather than a pool: `SQLite` in WAL mode
/// serializes writers anyway, the workload is one user's bookmarks, and a
/// single connection keeps the `FTS` triggers and the embedding table trivially
/// consistent. A pool would be complexity bought for contention that does not
/// exist here.
#[derive(Clone)]
pub struct AppState {
    /// The bookmark store.
    pub store: Arc<Mutex<Store>>,
    /// The embedding provider used for semantic recall.
    pub embedder: Arc<Provider>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}

impl AppState {
    /// Builds the state from an open store and a provider.
    #[must_use]
    pub fn new(store: Store, embedder: Provider) -> Self {
        Self { store: Arc::new(Mutex::new(store)), embedder: Arc::new(embedder) }
    }
}
