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

    /// Embeds one bookmark in the background, so a page saved from the browser
    /// becomes searchable without anyone running a CLI command.
    ///
    /// This is spawned rather than awaited: the caller has already written the
    /// row and should return immediately. Embedding is a few milliseconds
    /// against an already-loaded model, but it is not worth making the save
    /// wait on it, and a save that succeeded must not be reported as failed
    /// because the vector could not be computed.
    ///
    /// A failure here is logged and dropped. The row stays in
    /// `needing_embedding`, so the next `torimemo embed` picks it up — the
    /// backfill is the safety net that makes this fire-and-forget safe.
    pub fn embed_in_background(&self, bookmark_id: i64) {
        let state = self.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = state.embed_now(bookmark_id) {
                eprintln!("could not embed bookmark {bookmark_id}: {error}");
            }
        });
    }

    /// Computes and stores one bookmark's vector.
    ///
    /// Separate from [`Self::embed_in_background`] so the work is testable
    /// without a runtime.
    pub fn embed_now(&self, bookmark_id: i64) -> torimemo_core::Result<()> {
        use torimemo_embed::{Embedder as _, embed_text, input_hash};

        let bookmark = {
            let store = self
                .store
                .lock()
                .map_err(|_| torimemo_core::Error::msg("store lock was poisoned"))?;
            store.bookmark(bookmark_id)?
        };
        let Some(bookmark) = bookmark else { return Ok(()) };

        // The lock is released across the embedding call: inference is the
        // slow part, and holding the store through it would block every other
        // request for no reason.
        let text = embed_text(&bookmark);
        let embedding = self.embedder.embed(&text)?;

        let store =
            self.store.lock().map_err(|_| torimemo_core::Error::msg("store lock was poisoned"))?;
        store.set_embedding(
            bookmark_id,
            self.embedder.model(),
            &embedding.vector,
            &input_hash(&text),
        )
    }
}
