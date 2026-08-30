//! Vector storage.
//!
//! Vectors live beside the bookmark rather than in it, keyed by model, so
//! re-embedding with a newer model is an insert alongside the old vector
//! rather than a destructive update.

use super::{Store, bookmark_from_row, decode_vector};
use crate::error::{Error, Result};
use crate::model::Bookmark;
use chrono::Utc;
use rusqlite::params;

impl Store {
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
}
