//! SQLite-backed storage.
//!
//! One file holds everything: relational rows, the FTS5 index, and the
//! embedding vectors. Keeping the vector store in the same database as the
//! bookmarks is what makes a ranked query a single statement instead of a join
//! across two systems, and at this corpus size an approximate index would buy
//! back less time than it costs in moving parts.

use crate::error::{Error, Result};
use crate::model::{Bookmark, Capture, Ingested, NewCapture, Source};
use crate::normalize;
use crate::token::{self, Issued, Principal, Scope, TokenInfo};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::path::Path;
use std::str::FromStr as _;

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
        connection.execute_batch(include_str!("schema.sql"))?;
        Ok(Self { connection })
    }

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
    pub fn clear_embeddings(&self, bookmark_id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM embeddings WHERE bookmark_id = ?1", params![bookmark_id])?;
        Ok(())
    }

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

    /// Mints a service token.
    ///
    /// The secret is returned once and never stored in the clear. Losing it
    /// means minting a replacement, which is the intended failure mode.
    pub fn issue_token(&self, name: &str, scope: Scope) -> Result<Issued> {
        let secret = token::generate()?;
        let id = token::hash(&secret)[..16].to_string();

        self.connection.execute(
            "INSERT INTO service_tokens (id, name, token_hash, scope, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, token::hash(&secret), scope.as_str(), Utc::now().to_rfc3339()],
        )?;

        Ok(Issued { id, name: name.to_string(), scope, token: secret })
    }

    /// Validates a presented token.
    ///
    /// Lookup is **by hash**, so the comparison happens inside `SQLite`'s index
    /// on a value that reveals nothing, and no code path here ever holds a
    /// stored secret to compare against — there is none.
    ///
    /// Returns `None` for every failure — unknown, malformed, revoked —
    /// because the caller answers one generic 401 regardless. Distinguishing
    /// them would let a caller probe for which tokens exist.
    pub fn authenticate(&self, presented: &str) -> Result<Option<Principal>> {
        if !token::looks_valid(presented) {
            return Ok(None);
        }

        let found: Option<(String, String, String, Option<String>)> = self
            .connection
            .query_row(
                "SELECT id, name, scope, revoked_at FROM service_tokens WHERE token_hash = ?1",
                params![token::hash(presented)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        let Some((id, name, scope, revoked_at)) = found else { return Ok(None) };
        if revoked_at.is_some() {
            return Ok(None);
        }

        Ok(Some(Principal { id, name, scope: scope.parse()? }))
    }

    /// Revokes a token by id.
    ///
    /// A flag rather than a delete, so the record of what a machine could
    /// reach survives the revocation. Returns whether a live token was
    /// revoked, so revoking twice is visible rather than silent.
    pub fn revoke_token(&self, id: &str) -> Result<bool> {
        let affected = self.connection.execute(
            "UPDATE service_tokens SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            params![id, Utc::now().to_rfc3339()],
        )?;
        Ok(affected > 0)
    }

    /// Every token, newest first, including revoked ones.
    pub fn list_tokens(&self) -> Result<Vec<TokenInfo>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, scope, created_at, revoked_at
               FROM service_tokens ORDER BY created_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            let scope: String = row.get(2)?;
            Ok(TokenInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                scope: scope.parse().unwrap_or(Scope::Read),
                created_at: row.get(3)?,
                revoked_at: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Whether any usable token exists.
    ///
    /// The API uses this to decide whether the registry is open or closed: a
    /// store with no tokens has not been configured for agent access yet, and
    /// refusing every call would be a confusing first run.
    pub fn has_tokens(&self) -> Result<bool> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM service_tokens WHERE revoked_at IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

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
    pub fn needing_embedding(&self, model: &str, limit: usize) -> Result<Vec<Bookmark>> {
        let mut statement = self.connection.prepare(
            "SELECT b.id, b.canonical_url, b.domain, b.title, b.description,
                    b.first_captured_at, b.last_captured_at, b.capture_count
               FROM bookmarks b
               LEFT JOIN embeddings e ON e.bookmark_id = b.id AND e.model = ?1
              WHERE e.bookmark_id IS NULL
              ORDER BY b.id
              LIMIT ?2",
        )?;
        let rows = statement.query_map(params![model, limit as i64], bookmark_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Stores a vector, replacing any previous one from the same model.
    pub fn set_embedding(
        &self,
        bookmark_id: i64,
        model: &str,
        vector: &[f32],
        input_hash: &str,
    ) -> Result<()> {
        let mut blob = Vec::with_capacity(vector.len() * 4);
        for value in vector {
            blob.extend_from_slice(&value.to_le_bytes());
        }
        self.connection.execute(
            "INSERT INTO embeddings
                 (bookmark_id, model, dimensions, vector, input_hash, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (bookmark_id, model) DO UPDATE
                SET dimensions  = excluded.dimensions,
                    vector      = excluded.vector,
                    input_hash  = excluded.input_hash,
                    computed_at = excluded.computed_at",
            params![
                bookmark_id,
                model,
                vector.len() as i64,
                blob,
                input_hash,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Every stored vector for `model`, as `(bookmark_id, vector)`.
    pub fn embeddings(&self, model: &str) -> Result<Vec<(i64, Vec<f32>)>> {
        let mut statement = self
            .connection
            .prepare("SELECT bookmark_id, vector FROM embeddings WHERE model = ?1")?;
        let rows = statement.query_map(params![model], |row| {
            let id: i64 = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((id, decode_vector(&blob)))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

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
