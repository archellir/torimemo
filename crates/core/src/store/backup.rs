//! Making a copy of the archive.
//!
//! The notes attached to captures are the only part of this store that cannot
//! be reconstructed. URLs come back from a browser export and titles come back
//! from a fetch, but *why* something was worth keeping exists nowhere else, and
//! that is precisely what accumulates as the archive grows.
//!
//! `VACUUM INTO` is used rather than copying the file: it produces a
//! transactionally consistent snapshot from a live database, so a backup taken
//! while the server is mid-write is still a valid database rather than a torn
//! copy that happens to open.

use super::Store;
use crate::error::{Error, Result};
use chrono::Utc;
use rusqlite::params;
use std::path::{Path, PathBuf};

impl Store {
    /// Writes a consistent snapshot of the archive to `path`.
    ///
    /// Refuses to overwrite: a backup command that silently replaces an older
    /// backup can destroy the only good copy when run against an already
    /// damaged database.
    pub fn backup_to(&self, path: &Path) -> Result<u64> {
        if path.exists() {
            return Err(Error::msg(format!("{} already exists", path.display())));
        }

        // VACUUM INTO takes its destination as a string literal, and a path
        // containing a quote would otherwise break the statement.
        let destination = path.to_string_lossy().replace('\'', "''");
        self.connection.execute(&format!("VACUUM INTO '{destination}'"), params![]).map_err(
            |error| Error::with_source(format!("could not write {}", path.display()), error),
        )?;

        std::fs::metadata(path).map(|meta| meta.len()).map_err(Error::from)
    }

    /// A timestamped filename for a snapshot taken now.
    ///
    /// Sorts chronologically as text, so `ls` and a glob both give the
    /// backups in order without anything having to parse the name.
    #[must_use]
    pub fn backup_name() -> String {
        format!("torimemo-{}.db", Utc::now().format("%Y%m%dT%H%M%SZ"))
    }

    /// Existing snapshots in `directory`, oldest first.
    pub fn backups_in(directory: &Path) -> Result<Vec<PathBuf>> {
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut found: Vec<PathBuf> = std::fs::read_dir(directory)?
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("torimemo-") && name.ends_with(".db"))
            })
            .collect();
        found.sort();
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NewCapture, Source};

    fn populated() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        let id = store
            .ingest(&NewCapture::new("https://example.com/a", Source::Api).with_context("why"))
            .unwrap()
            .bookmark_id();
        store.set_metadata(id, Some("A title"), None).unwrap();
        store
    }

    #[test]
    fn a_backup_is_a_working_database_with_the_same_contents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(Store::backup_name());

        let store = populated();
        assert!(store.backup_to(&path).unwrap() > 0, "backup should not be empty");

        // The point of the exercise: the copy opens and holds the same rows,
        // including the note, which is the part nothing else can recover.
        let restored = Store::open(&path).unwrap();
        assert_eq!(restored.stats().unwrap().bookmarks, 1);
        let captures = restored.captures(1).unwrap();
        assert_eq!(captures[0].context.as_deref(), Some("why"));
    }

    #[test]
    fn an_existing_file_is_never_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("taken.db");
        std::fs::write(&path, b"not a database").unwrap();

        let error = populated().backup_to(&path).unwrap_err();
        assert!(error.to_string().contains("already exists"));
        // The bystander file is untouched, which is the property that matters.
        assert_eq!(std::fs::read(&path).unwrap(), b"not a database");
    }

    #[test]
    fn backup_names_sort_chronologically() {
        let first = "torimemo-20260101T000000Z.db";
        let second = "torimemo-20260102T000000Z.db";
        assert!(first < second);
        assert!(Store::backup_name().starts_with("torimemo-"));
        assert!(Store::backup_name().ends_with(".db"));
    }

    #[test]
    fn listing_finds_snapshots_and_ignores_everything_else() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["torimemo-20260101T000000Z.db", "torimemo-20260102T000000Z.db"] {
            std::fs::write(directory.path().join(name), b"x").unwrap();
        }
        std::fs::write(directory.path().join("notes.txt"), b"x").unwrap();

        let found = Store::backups_in(directory.path()).unwrap();
        assert_eq!(found.len(), 2, "only the snapshots should be listed");
        assert!(found[0] < found[1], "oldest first");
    }

    #[test]
    fn listing_a_missing_directory_is_empty_rather_than_an_error() {
        assert!(Store::backups_in(Path::new("/nonexistent/xyz")).unwrap().is_empty());
    }
}
