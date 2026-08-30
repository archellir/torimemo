//! Capture, dedupe, and the bookmark rows themselves.
//!
//! This is the only write path for links: every capture surface funnels
//! through [`Store::ingest`], so normalization and dedupe cannot be bypassed
//! by adding a new one.

use super::{BatchOutcome, Store, bookmark_from_row, parse_timestamp};
use crate::error::{Error, Result};
use crate::model::{Bookmark, Capture, Ingested, NewCapture, Source};
use crate::normalize;
use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use std::str::FromStr as _;

impl Store {
    /// Ingests one raw URL.
    ///
    /// Normalizes, then either creates a bookmark or appends a capture to the
    /// existing one. This is the only write path for links: every capture
    /// surface funnels through it, so dedupe cannot be bypassed.
    pub fn ingest(&mut self, capture: &NewCapture) -> Result<Ingested> {
        let canonical = normalize::canonicalize(&capture.raw_url)?;
        let captured_at = capture.captured_at.unwrap_or_else(Utc::now);
        let timestamp = captured_at.to_rfc3339();

        let transaction = self.connection.transaction()?;

        let existing: Option<i64> = transaction
            .query_row(
                "SELECT id FROM bookmarks WHERE canonical_url = ?1",
                params![canonical.url],
                |row| row.get(0),
            )
            .optional()?;

        let (bookmark_id, created) = match existing {
            Some(id) => {
                // `MIN`/`MAX` rather than assignment because a backfill walks
                // the vault in file order, not chronological order, so an
                // older capture can arrive after a newer one.
                transaction.execute(
                    "UPDATE bookmarks
                        SET capture_count     = capture_count + 1,
                            last_captured_at  = MAX(last_captured_at, ?2),
                            first_captured_at = MIN(first_captured_at, ?2)
                      WHERE id = ?1",
                    params![id, timestamp],
                )?;
                (id, false)
            }
            None => {
                transaction.execute(
                    "INSERT INTO bookmarks
                         (canonical_url, domain, first_captured_at, last_captured_at, capture_count)
                     VALUES (?1, ?2, ?3, ?3, 0)",
                    params![canonical.url, canonical.domain, timestamp],
                )?;
                let id = transaction.last_insert_rowid();
                transaction
                    .execute("UPDATE bookmarks SET capture_count = 1 WHERE id = ?1", params![id])?;
                (id, true)
            }
        };

        transaction.execute(
            "INSERT INTO captures (bookmark_id, raw_url, source, context, captured_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                bookmark_id,
                capture.raw_url,
                capture.source.as_str(),
                capture.context,
                timestamp
            ],
        )?;

        transaction.commit()?;

        Ok(if created { Ingested::Created(bookmark_id) } else { Ingested::Merged(bookmark_id) })
    }

    /// Ingests many URLs, skipping those that fail to normalize.
    ///
    /// Returns the per-URL outcomes alongside the raw URLs that were skipped.
    /// Bulk imports of scraped markdown always contain some unparseable lines,
    /// and failing the whole import over one of them would be useless.
    /// Ingests many URLs, skipping those that fail to normalize.
    ///
    /// Returns the per-URL outcomes alongside the raw URLs that were skipped.
    /// Bulk imports of scraped markdown always contain some unparseable lines,
    /// and failing the whole import over one of them would be useless.
    pub fn ingest_batch(&mut self, captures: &[NewCapture]) -> Result<BatchOutcome> {
        let mut outcome = BatchOutcome::default();
        for capture in captures {
            match self.ingest(capture) {
                Ok(ingested) if ingested.is_new() => outcome.created += 1,
                Ok(_) => outcome.merged += 1,
                Err(error) => outcome.skipped.push((capture.raw_url.clone(), error.to_string())),
            }
        }
        Ok(outcome)
    }

    /// Fetches a bookmark by id.
    /// Fetches a bookmark by id.
    pub fn bookmark(&self, id: i64) -> Result<Option<Bookmark>> {
        self.connection
            .query_row(
                "SELECT id, canonical_url, domain, title, description,
                        first_captured_at, last_captured_at, capture_count
                   FROM bookmarks WHERE id = ?1",
                params![id],
                bookmark_from_row,
            )
            .optional()
            .map_err(Error::from)
    }

    /// Fetches a bookmark by its canonical URL.
    ///
    /// The argument is normalized first, so a caller can pass a raw URL.
    /// Fetches a bookmark by its canonical URL.
    ///
    /// The argument is normalized first, so a caller can pass a raw URL.
    pub fn bookmark_by_url(&self, raw_url: &str) -> Result<Option<Bookmark>> {
        let canonical = normalize::canonicalize(raw_url)?;
        self.connection
            .query_row(
                "SELECT id, canonical_url, domain, title, description,
                        first_captured_at, last_captured_at, capture_count
                   FROM bookmarks WHERE canonical_url = ?1",
                params![canonical.url],
                bookmark_from_row,
            )
            .optional()
            .map_err(Error::from)
    }

    /// Every capture recorded for a bookmark, oldest first.
    /// Every capture recorded for a bookmark, oldest first.
    pub fn captures(&self, bookmark_id: i64) -> Result<Vec<Capture>> {
        let mut statement = self.connection.prepare(
            "SELECT id, bookmark_id, raw_url, source, context, captured_at
               FROM captures WHERE bookmark_id = ?1 ORDER BY captured_at",
        )?;
        let rows = statement.query_map(params![bookmark_id], |row| {
            let source: String = row.get(3)?;
            Ok(Capture {
                id: row.get(0)?,
                bookmark_id: row.get(1)?,
                raw_url: row.get(2)?,
                // A row written by a newer version with an unknown source still
                // reads back; the capture matters more than its label.
                source: Source::from_str(&source).unwrap_or(Source::Api),
                context: row.get(4)?,
                captured_at: parse_timestamp(row, 5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Records a title and description discovered by enrichment.
    /// Records a title and description discovered by enrichment.
    pub fn set_metadata(
        &self,
        bookmark_id: i64,
        title: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        self.connection.execute(
            "UPDATE bookmarks SET title = ?2, description = ?3 WHERE id = ?1",
            params![bookmark_id, title, description],
        )?;
        Ok(())
    }

    /// Records what enrichment found, including that it found nothing.
    ///
    /// `attempts` accumulates across passes so a repeatedly failing URL can be
    /// given up on, rather than being retried forever.
    /// Every bookmark paired with what enrichment found, for classification.
    ///
    /// Returns the fetch status alongside the bookmark because the rules need
    /// both: a page that 404s and a page that yielded no title are different
    /// facts, and only the fetch record distinguishes them.
    pub fn all_with_fetch_status(&self) -> Result<Vec<(Bookmark, Option<String>)>> {
        let mut statement = self.connection.prepare(
            "SELECT b.id, b.canonical_url, b.domain, b.title, b.description,
                    b.first_captured_at, b.last_captured_at, b.capture_count, f.status
               FROM bookmarks b
               LEFT JOIN fetch_state f ON f.bookmark_id = b.id
              ORDER BY b.id",
        )?;
        let rows = statement
            .query_map([], |row| Ok((bookmark_from_row(row)?, row.get::<_, Option<String>>(8)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Titles held by more than one bookmark, keeping the oldest of each.
    ///
    /// Returns the ids that would be removed. The oldest is kept rather than
    /// the newest because `first_captured_at` is the closest thing this corpus
    /// has to a real save date.
    /// Titles held by more than one bookmark, keeping the oldest of each.
    ///
    /// Returns the ids that would be removed. The oldest is kept rather than
    /// the newest because `first_captured_at` is the closest thing this corpus
    /// has to a real save date.
    pub fn duplicate_title_ids(&self) -> Result<Vec<i64>> {
        let mut statement = self.connection.prepare(
            "SELECT id FROM bookmarks
              WHERE title IS NOT NULL
                AND id NOT IN (SELECT MIN(id) FROM bookmarks WHERE title IS NOT NULL GROUP BY title)
                AND title IN (SELECT title FROM bookmarks WHERE title IS NOT NULL
                               GROUP BY title HAVING COUNT(*) > 1)",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Deletes bookmarks by id.
    ///
    /// Captures, embeddings, tags, events, and fetch state cascade, so a
    /// deleted bookmark leaves nothing behind. This is the only delete path in
    /// the crate and it takes an explicit list — there is deliberately no
    /// "delete everything matching a query", so a caller must have enumerated
    /// and been able to review exactly what it is removing.
    /// Deletes bookmarks by id.
    ///
    /// Captures, embeddings, tags, events, and fetch state cascade, so a
    /// deleted bookmark leaves nothing behind. This is the only delete path in
    /// the crate and it takes an explicit list — there is deliberately no
    /// "delete everything matching a query", so a caller must have enumerated
    /// and been able to review exactly what it is removing.
    pub fn delete_bookmarks(&mut self, ids: &[i64]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let transaction = self.connection.transaction()?;
        let mut deleted = 0;
        {
            let mut statement = transaction.prepare("DELETE FROM bookmarks WHERE id = ?1")?;
            for id in ids {
                deleted += statement.execute(params![id])?;
            }
        }
        transaction.commit()?;
        Ok(deleted)
    }
}
