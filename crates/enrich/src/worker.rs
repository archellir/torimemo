//! The enrichment pass.
//!
//! Walks the un-enriched bookmarks, fetches each one, and writes back what it
//! found. Two properties matter and both come from the store rather than from
//! bookkeeping here: it is **resumable**, because the work queue is defined by
//! the absence of a `fetch_state` row, and it is **idempotent**, because
//! re-running it against a fully enriched corpus does nothing.

use crate::fetch::{Fetcher, Outcome};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use torimemo_core::{Result, Store};

/// How the pass is run.
#[derive(Debug, Clone)]
pub struct Config {
    /// How many requests may be in flight at once, across all hosts.
    pub concurrency: usize,
    /// Minimum gap between two requests to the *same* host. Global
    /// concurrency alone would let a hundred Instagram URLs fire at once.
    pub per_host_delay: Duration,
    /// Per-request timeout.
    pub timeout: Duration,
    /// Give up on a URL after this many failed attempts.
    pub max_attempts: i64,
    /// Maximum bookmarks to process in this run.
    pub limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            concurrency: 16,
            per_host_delay: Duration::from_millis(500),
            timeout: Duration::from_secs(10),
            max_attempts: 3,
            limit: usize::MAX,
        }
    }
}

/// What a pass did.
#[derive(Debug, Default, Clone, Copy)]
pub struct Summary {
    /// Pages that yielded a title or description.
    pub enriched: usize,
    /// Pages fetched that carried nothing usable.
    pub no_metadata: usize,
    /// URLs that are gone.
    pub dead: usize,
    /// URLs that failed transiently and will be retried.
    pub failed: usize,
}

impl Summary {
    /// Total processed.
    #[must_use]
    pub fn total(&self) -> usize {
        self.enriched + self.no_metadata + self.dead + self.failed
    }
}

/// Runs one enrichment pass.
///
/// `progress` is called after each bookmark with the running summary.
pub async fn run(
    store: &Store,
    config: &Config,
    mut progress: impl FnMut(&Summary),
) -> Result<Summary> {
    let pending = store.needing_fetch(config.max_attempts, config.limit)?;
    if pending.is_empty() {
        return Ok(Summary::default());
    }

    let fetcher = Fetcher::new(config.timeout)?;
    let permits = Arc::new(Semaphore::new(config.concurrency));
    // Last-request time per host, so politeness is enforced per origin rather
    // than globally. A corpus this skewed — 268 Instagram URLs — would
    // otherwise spend its whole run queued behind one host's rate limit.
    let host_clocks: Arc<Mutex<HashMap<String, tokio::time::Instant>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let mut tasks = Vec::with_capacity(pending.len());

    for bookmark in pending {
        let permits = Arc::clone(&permits);
        let host_clocks = Arc::clone(&host_clocks);
        let fetcher = fetcher.clone();
        let delay = config.per_host_delay;

        tasks.push(tokio::spawn(async move {
            let _permit = permits.acquire_owned().await.ok()?;

            {
                let mut clocks = host_clocks.lock().await;
                let now = tokio::time::Instant::now();
                if let Some(last) = clocks.get(&bookmark.domain) {
                    let elapsed = now.saturating_duration_since(*last);
                    if elapsed < delay {
                        tokio::time::sleep(delay - elapsed).await;
                    }
                }
                clocks.insert(bookmark.domain.clone(), tokio::time::Instant::now());
            }

            let outcome = fetcher.fetch(&bookmark.canonical_url).await;
            Some((bookmark, outcome))
        }));
    }

    let mut summary = Summary::default();

    for task in tasks {
        let Ok(Some((bookmark, outcome))) = task.await else { continue };

        match outcome {
            Outcome::Enriched(metadata) => {
                store.set_metadata(
                    bookmark.id,
                    metadata.title.as_deref(),
                    metadata.description.as_deref(),
                )?;
                // The text this bookmark is embedded from just changed, so the
                // stored vector no longer describes it. Dropping it re-queues
                // the row for the next embedding pass rather than leaving a
                // vector that silently disagrees with the row it indexes.
                store.clear_embeddings(bookmark.id)?;
                store.set_fetch_state(bookmark.id, "enriched", None)?;
                summary.enriched += 1;
            }
            Outcome::NoMetadata => {
                store.set_fetch_state(bookmark.id, "no_metadata", None)?;
                summary.no_metadata += 1;
            }
            Outcome::Dead(detail) => {
                store.set_fetch_state(bookmark.id, "dead", Some(&detail))?;
                summary.dead += 1;
            }
            Outcome::Failed(detail) => {
                store.set_fetch_state(bookmark.id, "failed", Some(&detail))?;
                summary.failed += 1;
            }
        }

        progress(&summary);
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use torimemo_core::{NewCapture, Source};

    #[tokio::test]
    async fn an_empty_corpus_is_a_no_op() {
        let store = Store::open_in_memory().unwrap();
        let summary = run(&store, &Config::default(), |_| {}).await.unwrap();
        assert_eq!(summary.total(), 0);
    }

    #[test]
    fn the_queue_prefers_repeatedly_captured_links() {
        let mut store = Store::open_in_memory().unwrap();
        store.ingest(&NewCapture::new("https://once.com/a", Source::Api)).unwrap();
        store.ingest(&NewCapture::new("https://twice.com/b", Source::Api)).unwrap();
        store.ingest(&NewCapture::new("https://twice.com/b", Source::Telegram)).unwrap();

        let queue = store.needing_fetch(3, 10).unwrap();
        assert_eq!(queue[0].domain, "twice.com", "most-captured should be enriched first");
    }

    #[test]
    fn the_queue_skips_resolved_rows_and_gives_up_after_max_attempts() {
        let mut store = Store::open_in_memory().unwrap();
        let enriched =
            store.ingest(&NewCapture::new("https://a.com/x", Source::Api)).unwrap().bookmark_id();
        let dead =
            store.ingest(&NewCapture::new("https://b.com/y", Source::Api)).unwrap().bookmark_id();
        let flaky =
            store.ingest(&NewCapture::new("https://c.com/z", Source::Api)).unwrap().bookmark_id();

        store.set_fetch_state(enriched, "enriched", None).unwrap();
        store.set_fetch_state(dead, "dead", Some("HTTP 404")).unwrap();
        store.set_fetch_state(flaky, "failed", Some("timeout")).unwrap();

        // The flaky one is still retryable at one attempt.
        assert_eq!(store.needing_fetch(3, 10).unwrap().len(), 1);

        store.set_fetch_state(flaky, "failed", Some("timeout")).unwrap();
        store.set_fetch_state(flaky, "failed", Some("timeout")).unwrap();

        // Three attempts in, it is given up on.
        assert!(store.needing_fetch(3, 10).unwrap().is_empty());
    }
}
