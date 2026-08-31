//! SQLite-backed storage.
//!
//! One database file holds everything: relational rows, the FTS5 index, and
//! the embedding vectors. Keeping vectors beside the bookmarks is what makes a
//! ranked query a single statement rather than a join across two systems.
//!
//! The [`Store`] type is one struct with one connection; its methods are split
//! across sibling modules by concern, because thirty-odd methods spanning
//! captures, enrichment, tags, credentials, and vectors is more than one file
//! can be read as. Each module is an `impl Store` block:
//!
//! - [`bookmarks`] — ingest, dedupe, and the bookmark rows
//! - [`enrich`] — what the fetch pass learned, including that it learned nothing
//! - [`tags`] — tags and the labelling queue
//! - [`tokens`] — bearer credentials for non-browser callers
//! - [`embed`] — vector storage, keyed by model
//! - [`query`] — search, counts, and the interaction log
//!
//! Shared row decoding lives here, since every module needs it.

use crate::error::Result;
use crate::model::Bookmark;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, Row};
use std::path::Path;

mod backup;
mod bookmarks;
mod embed;
mod enrich;
mod query;
mod tags;
mod tokens;

/// The bookmark store.
#[derive(Debug)]
pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens the store at `path`, creating and migrating it if needed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    /// Opens an in-memory store, for tests.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        // WAL keeps the capture surfaces writing while the enrichment worker
        // reads; `foreign_keys` is off by default in SQLite and the cascade
        // deletes in the schema depend on it.
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.execute_batch(include_str!("../schema.sql"))?;
        Ok(Self { connection })
    }
}

/// What a batch ingest did.
#[derive(Debug, Default, Clone)]
pub struct BatchOutcome {
    /// New bookmarks created.
    pub created: usize,
    /// Captures appended to bookmarks that already existed.
    pub merged: usize,
    /// URLs that could not be normalized, with the reason.
    pub skipped: Vec<(String, String)>,
}

/// Corpus counts.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Stats {
    /// Distinct canonical URLs.
    pub bookmarks: i64,
    /// Capture events across all sources.
    pub captures: i64,
    /// Distinct domains.
    pub domains: i64,
    /// Bookmarks with at least one vector.
    pub embedded: i64,
    /// Bookmarks whose metadata has been fetched.
    pub with_title: i64,
    /// Recorded interactions.
    pub events: i64,
}

fn decode_vector(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn parse_timestamp(row: &Row<'_>, index: usize) -> rusqlite::Result<DateTime<Utc>> {
    let raw: String = row.get(index)?;
    DateTime::parse_from_rfc3339(&raw).map(|value| value.with_timezone(&Utc)).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn bookmark_from_row(row: &Row<'_>) -> rusqlite::Result<Bookmark> {
    Ok(Bookmark {
        id: row.get(0)?,
        canonical_url: row.get(1)?,
        domain: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        first_captured_at: parse_timestamp(row, 5)?,
        last_captured_at: parse_timestamp(row, 6)?,
        capture_count: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    // Imported here rather than at file scope: the tests exercise every
    // module's methods, while `mod.rs` itself only needs `Bookmark`.
    use crate::model::{NewCapture, Source};
    use crate::token::Scope;
    use chrono::TimeZone as _;

    fn store() -> Store {
        Store::open_in_memory().expect("in-memory store should open")
    }

    #[test]
    fn ingest_creates_a_bookmark() {
        let mut store = store();
        let outcome = store.ingest(&NewCapture::new("https://example.com/a", Source::Api)).unwrap();
        assert!(outcome.is_new());
        assert_eq!(store.stats().unwrap().bookmarks, 1);
    }

    #[test]
    fn the_same_link_from_two_channels_is_one_bookmark_and_two_captures() {
        let mut store = store();
        store.ingest(&NewCapture::new("https://example.com/a", Source::Telegram)).unwrap();
        let second = store
            .ingest(&NewCapture::new("https://example.com/a?utm_source=whatsapp", Source::Api))
            .unwrap();

        assert!(!second.is_new());
        let stats = store.stats().unwrap();
        assert_eq!(stats.bookmarks, 1);
        assert_eq!(stats.captures, 2);
        assert_eq!(store.bookmark(second.bookmark_id()).unwrap().unwrap().capture_count, 2);
    }

    #[test]
    fn first_captured_survives_out_of_order_backfill() {
        let mut store = store();
        let recent = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let older = Utc.with_ymd_and_hms(2022, 1, 1, 0, 0, 0).unwrap();

        store.ingest(&NewCapture::new("https://example.com/a", Source::Api).at(recent)).unwrap();
        let id = store
            .ingest(&NewCapture::new("https://example.com/a", Source::Vault).at(older))
            .unwrap()
            .bookmark_id();

        let bookmark = store.bookmark(id).unwrap().unwrap();
        assert_eq!(bookmark.first_captured_at, older);
        assert_eq!(bookmark.last_captured_at, recent);
    }

    #[test]
    fn batch_skips_unparseable_urls_without_failing() {
        let mut store = store();
        let captures = vec![
            NewCapture::new("https://example.com/a", Source::Vault),
            NewCapture::new("not a url", Source::Vault),
            NewCapture::new("mailto:a@b.com", Source::Vault),
        ];
        let outcome = store.ingest_batch(&captures).unwrap();
        assert_eq!(outcome.created, 1);
        assert_eq!(outcome.skipped.len(), 2);
    }

    #[test]
    fn fts_finds_a_bookmark_by_title() {
        let mut store = store();
        let id = store
            .ingest(&NewCapture::new("https://example.com/a", Source::Api))
            .unwrap()
            .bookmark_id();
        store.set_metadata(id, Some("Deterministic builds in Rust"), None).unwrap();

        let found = store.search("deterministic", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, id);
    }

    #[test]
    fn deleting_a_bookmark_takes_everything_that_hangs_off_it() {
        let mut store = store();
        let id = store
            .ingest(&NewCapture::new("https://example.com/a", Source::Api))
            .unwrap()
            .bookmark_id();
        store.set_metadata(id, Some("Title"), None).unwrap();
        store.set_embedding(id, "m", &[0.5, 0.5], "hash").unwrap();
        store.set_tags(id, &["programming".into()], "model", Some(0.9), Some("m")).unwrap();
        store.record_event(id, "opened", None, None).unwrap();
        store.set_fetch_state(id, "enriched", None).unwrap();

        assert_eq!(store.delete_bookmarks(&[id]).unwrap(), 1);

        let stats = store.stats().unwrap();
        assert_eq!(stats.bookmarks, 0);
        assert_eq!(stats.captures, 0, "captures should cascade");
        assert_eq!(stats.embedded, 0, "embeddings should cascade");
        assert_eq!(stats.events, 0, "events should cascade");
        assert!(store.fetch_summary().unwrap().is_empty(), "fetch state should cascade");
    }

    #[test]
    fn deleting_nothing_is_a_no_op() {
        let mut store = store();
        store.ingest(&NewCapture::new("https://example.com/a", Source::Api)).unwrap();
        assert_eq!(store.delete_bookmarks(&[]).unwrap(), 0);
        assert_eq!(store.stats().unwrap().bookmarks, 1);
    }

    #[test]
    fn deleting_an_unknown_id_removes_nothing() {
        let mut store = store();
        store.ingest(&NewCapture::new("https://example.com/a", Source::Api)).unwrap();
        assert_eq!(store.delete_bookmarks(&[999]).unwrap(), 0);
        assert_eq!(store.stats().unwrap().bookmarks, 1);
    }

    #[test]
    fn duplicate_titles_keep_the_oldest() {
        let mut store = store();
        let first =
            store.ingest(&NewCapture::new("https://a.com/x", Source::Api)).unwrap().bookmark_id();
        let second =
            store.ingest(&NewCapture::new("https://b.com/y", Source::Api)).unwrap().bookmark_id();
        store.set_metadata(first, Some("Same"), None).unwrap();
        store.set_metadata(second, Some("Same"), None).unwrap();

        assert_eq!(store.duplicate_title_ids().unwrap(), vec![second]);
    }

    #[test]
    fn a_unique_title_is_never_a_duplicate() {
        let mut store = store();
        let id =
            store.ingest(&NewCapture::new("https://a.com/x", Source::Api)).unwrap().bookmark_id();
        store.set_metadata(id, Some("Unique"), None).unwrap();
        assert!(store.duplicate_title_ids().unwrap().is_empty());
    }

    #[test]
    fn a_minted_token_authenticates_once_and_only_as_itself() {
        let store = store();
        let issued = store.issue_token("odin", Scope::ReadWrite).unwrap();

        let principal = store.authenticate(&issued.token).unwrap().unwrap();
        assert_eq!(principal.name, "odin");
        assert!(principal.scope.may_write());

        // A different token, even a well-formed one, is not this one.
        let other = store.issue_token("other", Scope::Read).unwrap();
        assert_eq!(store.authenticate(&other.token).unwrap().unwrap().name, "other");
    }

    #[test]
    fn the_secret_is_never_stored_in_the_clear() {
        let store = store();
        let issued = store.issue_token("odin", Scope::Read).unwrap();

        let stored: String = store
            .connection
            .query_row("SELECT token_hash FROM service_tokens", [], |row| row.get(0))
            .unwrap();
        assert_ne!(stored, issued.token);
        assert!(!stored.contains(&issued.token));
    }

    #[test]
    fn an_unknown_or_malformed_token_is_refused() {
        let store = store();
        store.issue_token("odin", Scope::Read).unwrap();

        assert!(store.authenticate("tmk_deadbeef").unwrap().is_none());
        assert!(store.authenticate("").unwrap().is_none());
        assert!(store.authenticate("not-a-token").unwrap().is_none());
    }

    #[test]
    fn a_revoked_token_stops_working_but_stays_listed() {
        let store = store();
        let issued = store.issue_token("odin", Scope::Read).unwrap();

        assert!(store.revoke_token(&issued.id).unwrap());
        assert!(store.authenticate(&issued.token).unwrap().is_none());

        let listed = store.list_tokens().unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].revoked_at.is_some());
    }

    #[test]
    fn revoking_twice_reports_that_nothing_changed() {
        let store = store();
        let issued = store.issue_token("odin", Scope::Read).unwrap();

        assert!(store.revoke_token(&issued.id).unwrap());
        assert!(!store.revoke_token(&issued.id).unwrap());
        assert!(!store.revoke_token("no-such-id").unwrap());
    }

    #[test]
    fn scope_is_carried_through_authentication() {
        let store = store();
        let read = store.issue_token("reader", Scope::Read).unwrap();
        assert!(!store.authenticate(&read.token).unwrap().unwrap().scope.may_write());
    }

    #[test]
    fn has_tokens_ignores_revoked_ones() {
        let store = store();
        assert!(!store.has_tokens().unwrap());

        let issued = store.issue_token("odin", Scope::Read).unwrap();
        assert!(store.has_tokens().unwrap());

        store.revoke_token(&issued.id).unwrap();
        assert!(!store.has_tokens().unwrap());
    }

    #[test]
    fn embeddings_round_trip_and_backfill_queue_drains() {
        let mut store = store();
        let id = store
            .ingest(&NewCapture::new("https://example.com/a", Source::Api))
            .unwrap()
            .bookmark_id();

        assert_eq!(store.needing_embedding("m1", 10).unwrap().len(), 1);
        store.set_embedding(id, "m1", &[0.5, -0.25, 0.75], "hash").unwrap();
        assert!(store.needing_embedding("m1", 10).unwrap().is_empty());

        let stored = store.embeddings("m1").unwrap();
        assert_eq!(stored, vec![(id, vec![0.5, -0.25, 0.75])]);
        // A new model version leaves the old vector alone and re-queues the row.
        assert_eq!(store.needing_embedding("m2", 10).unwrap().len(), 1);
    }
}
