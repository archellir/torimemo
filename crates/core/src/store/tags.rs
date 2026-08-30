//! Tags, and the labelling queue that feeds them.
//!
//! `origin` separates a tag the user set from one a model proposed. A model
//! proposal never displaces a human tag, so re-running a labelling pass cannot
//! destroy a correction.

use super::{Store, bookmark_from_row};
use crate::error::{Error, Result};
use crate::model::Bookmark;
use chrono::Utc;
use rusqlite::params;

impl Store {
    /// Attaches tags to a bookmark.
    ///
    /// `origin` separates a tag the user set from one a model proposed. A
    /// model proposal never displaces a human tag — both rows coexist under
    /// the composite primary key — so re-running a labelling pass can never
    /// destroy a correction the user made.
    pub fn set_tags(
        &mut self,
        bookmark_id: i64,
        tags: &[String],
        origin: &str,
        confidence: Option<f64>,
        model: Option<&str>,
    ) -> Result<()> {
        let transaction = self.connection.transaction()?;
        let now = Utc::now().to_rfc3339();

        // Replace this origin's tags wholesale: a re-label is the model's new
        // opinion in full, not an addition to its old one.
        transaction.execute(
            "DELETE FROM bookmark_tags WHERE bookmark_id = ?1 AND origin = ?2",
            params![bookmark_id, origin],
        )?;

        for tag in tags {
            transaction.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", params![tag])?;
            let tag_id: i64 = transaction.query_row(
                "SELECT id FROM tags WHERE name = ?1",
                params![tag],
                |row| row.get(0),
            )?;
            transaction.execute(
                "INSERT INTO bookmark_tags
                     (bookmark_id, tag_id, origin, confidence, model, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![bookmark_id, tag_id, origin, confidence, model, now],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    /// A bookmark's tags, human-set ones first.
    /// A bookmark's tags, human-set ones first.
    pub fn tags(&self, bookmark_id: i64) -> Result<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT t.name FROM bookmark_tags bt
               JOIN tags t ON t.id = bt.tag_id
              WHERE bt.bookmark_id = ?1
              ORDER BY CASE bt.origin WHEN 'human' THEN 0 ELSE 1 END, t.name",
        )?;
        let rows = statement.query_map(params![bookmark_id], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Bookmarks with usable text that no model has labelled yet.
    ///
    /// Ordered by capture count so the links saved most often are labelled
    /// first: an interrupted run leaves the most valuable rows done.
    /// Bookmarks with usable text that no model has labelled yet.
    ///
    /// Ordered by capture count so the links saved most often are labelled
    /// first: an interrupted run leaves the most valuable rows done.
    pub fn needing_labels(&self, model: &str, limit: usize) -> Result<Vec<Bookmark>> {
        let mut statement = self.connection.prepare(
            "SELECT b.id, b.canonical_url, b.domain, b.title, b.description,
                    b.first_captured_at, b.last_captured_at, b.capture_count
               FROM bookmarks b
              WHERE (b.title IS NOT NULL OR b.description IS NOT NULL)
                AND NOT EXISTS (
                      SELECT 1 FROM bookmark_tags bt
                       WHERE bt.bookmark_id = b.id AND bt.model = ?1
                    )
              ORDER BY b.capture_count DESC, b.id
              LIMIT ?2",
        )?;
        let rows = statement.query_map(params![model, limit as i64], bookmark_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Tag counts across the corpus, most used first.
    /// Tag counts across the corpus, most used first.
    pub fn tag_counts(&self) -> Result<Vec<(String, i64)>> {
        let mut statement = self.connection.prepare(
            "SELECT t.name, COUNT(DISTINCT bt.bookmark_id) AS n
               FROM tags t JOIN bookmark_tags bt ON bt.tag_id = t.id
              GROUP BY t.name ORDER BY n DESC",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Every labelled bookmark with its tags, for exporting a training set.
    /// Every labelled bookmark with its tags, for exporting a training set.
    pub fn labelled(&self, model: &str) -> Result<Vec<(Bookmark, Vec<String>)>> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT b.id, b.canonical_url, b.domain, b.title, b.description,
                    b.first_captured_at, b.last_captured_at, b.capture_count
               FROM bookmarks b
               JOIN bookmark_tags bt ON bt.bookmark_id = b.id
              WHERE bt.model = ?1
              ORDER BY b.id",
        )?;
        let bookmarks = statement
            .query_map(params![model], bookmark_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut labelled = Vec::with_capacity(bookmarks.len());
        for bookmark in bookmarks {
            let tags = self.tags(bookmark.id)?;
            labelled.push((bookmark, tags));
        }
        Ok(labelled)
    }
}
