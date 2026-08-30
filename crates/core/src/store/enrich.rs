//! What the enrichment pass learned about a URL, including that it learned
//! nothing.
//!
//! Recording failure explicitly is what stops the worker retrying a
//! decade-dead link on every pass.

use super::{Store, bookmark_from_row};
use crate::error::{Error, Result};
use crate::model::Bookmark;
use chrono::Utc;
use rusqlite::params;

impl Store {
    /// Records what enrichment found, including that it found nothing.
    ///
    /// `attempts` accumulates across passes so a repeatedly failing URL can be
    /// given up on, rather than being retried forever.
    pub fn set_fetch_state(
        &self,
        bookmark_id: i64,
        status: &str,
        detail: Option<&str>,
    ) -> Result<()> {
        self.connection.execute(
            "INSERT INTO fetch_state (bookmark_id, status, detail, attempts, last_attempt)
             VALUES (?1, ?2, ?3, 1, ?4)
             ON CONFLICT (bookmark_id) DO UPDATE
                SET status       = excluded.status,
                    detail       = excluded.detail,
                    attempts     = fetch_state.attempts + 1,
                    last_attempt = excluded.last_attempt",
            params![bookmark_id, status, detail, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Bookmarks that enrichment has not resolved yet.
    ///
    /// Excludes anything already enriched or known dead, and anything that has
    /// failed `max_attempts` times. Ordered by capture count so the links the
    /// user saved repeatedly are enriched first — if a run is interrupted, the
    /// most valuable rows are already done.
    /// Bookmarks that enrichment has not resolved yet.
    ///
    /// Excludes anything already enriched or known dead, and anything that has
    /// failed `max_attempts` times. Ordered by capture count so the links the
    /// user saved repeatedly are enriched first — if a run is interrupted, the
    /// most valuable rows are already done.
    pub fn needing_fetch(&self, max_attempts: i64, limit: usize) -> Result<Vec<Bookmark>> {
        let mut statement = self.connection.prepare(
            "SELECT b.id, b.canonical_url, b.domain, b.title, b.description,
                    b.first_captured_at, b.last_captured_at, b.capture_count
               FROM bookmarks b
               LEFT JOIN fetch_state f ON f.bookmark_id = b.id
              WHERE f.bookmark_id IS NULL
                 OR (f.status = 'failed' AND f.attempts < ?1)
              ORDER BY b.capture_count DESC, b.id
              LIMIT ?2",
        )?;
        let rows = statement.query_map(params![max_attempts, limit as i64], bookmark_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Counts of each enrichment outcome.
    /// Counts of each enrichment outcome.
    pub fn fetch_summary(&self) -> Result<Vec<(String, i64)>> {
        let mut statement = self.connection.prepare(
            "SELECT status, COUNT(*) FROM fetch_state GROUP BY status ORDER BY COUNT(*) DESC",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Marks a bookmark's stored vectors stale by deleting them.
    ///
    /// Called when a title arrives after the first embedding pass: the text
    /// the vector was computed from has changed, so the row re-enters the
    /// embedding queue on the next backfill.
    /// Marks a bookmark's stored vectors stale by deleting them.
    ///
    /// Called when a title arrives after the first embedding pass: the text
    /// the vector was computed from has changed, so the row re-enters the
    /// embedding queue on the next backfill.
    pub fn clear_embeddings(&self, bookmark_id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM embeddings WHERE bookmark_id = ?1", params![bookmark_id])?;
        Ok(())
    }
}
