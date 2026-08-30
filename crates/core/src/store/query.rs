//! Read-only queries: search, counts, and the interaction log.

use super::{Stats, Store, bookmark_from_row};
use crate::error::{Error, Result};
use crate::model::Bookmark;
use chrono::Utc;
use rusqlite::params;

impl Store {
    /// Lexical search over URL, title, and description.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Bookmark>> {
        let mut statement = self.connection.prepare(
            "SELECT b.id, b.canonical_url, b.domain, b.title, b.description,
                    b.first_captured_at, b.last_captured_at, b.capture_count
               FROM bookmarks_fts f
               JOIN bookmarks b ON b.id = f.rowid
              WHERE bookmarks_fts MATCH ?1
              ORDER BY rank
              LIMIT ?2",
        )?;
        let rows = statement.query_map(params![query, limit as i64], bookmark_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Bookmarks with no vector for `model`, oldest first.
    ///
    /// Drives the embedding backfill: the worker asks for a batch, embeds it,
    /// writes the vectors, and asks again until this returns nothing.
    /// Records an interaction, the training signal for ranking.
    pub fn record_event(
        &self,
        bookmark_id: i64,
        kind: &str,
        query: Option<&str>,
        position: Option<i64>,
    ) -> Result<()> {
        self.connection.execute(
            "INSERT INTO events (bookmark_id, kind, query, position, occurred_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![bookmark_id, kind, query, position, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Every bookmark paired with what enrichment found, for classification.
    ///
    /// Returns the fetch status alongside the bookmark because the rules need
    /// both: a page that 404s and a page that yielded no title are different
    /// facts, and only the fetch record distinguishes them.
    /// Corpus counts, for the CLI and for sanity-checking a backfill.
    pub fn stats(&self) -> Result<Stats> {
        let scalar = |sql: &str| -> Result<i64> {
            self.connection.query_row(sql, [], |row| row.get(0)).map_err(Error::from)
        };
        Ok(Stats {
            bookmarks: scalar("SELECT COUNT(*) FROM bookmarks")?,
            captures: scalar("SELECT COUNT(*) FROM captures")?,
            domains: scalar("SELECT COUNT(DISTINCT domain) FROM bookmarks")?,
            embedded: scalar("SELECT COUNT(DISTINCT bookmark_id) FROM embeddings")?,
            with_title: scalar("SELECT COUNT(*) FROM bookmarks WHERE title IS NOT NULL")?,
            events: scalar("SELECT COUNT(*) FROM events")?,
        })
    }

    /// The most-captured bookmarks — the strongest available proxy for
    /// "this actually mattered to you".
    /// The most-captured bookmarks — the strongest available proxy for
    /// "this actually mattered to you".
    pub fn most_captured(&self, limit: usize) -> Result<Vec<Bookmark>> {
        let mut statement = self.connection.prepare(
            "SELECT id, canonical_url, domain, title, description,
                    first_captured_at, last_captured_at, capture_count
               FROM bookmarks
              WHERE capture_count > 1
              ORDER BY capture_count DESC, last_captured_at DESC
              LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], bookmark_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Bookmark counts by domain, most first.
    /// Bookmark counts by domain, most first.
    pub fn top_domains(&self, limit: usize) -> Result<Vec<(String, i64)>> {
        let mut statement = self.connection.prepare(
            "SELECT domain, COUNT(*) AS n FROM bookmarks
              GROUP BY domain ORDER BY n DESC LIMIT ?1",
        )?;
        let rows =
            statement.query_map(params![limit as i64], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }
}
