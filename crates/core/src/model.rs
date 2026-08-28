//! The types the deterministic core stores.
//!
//! The central modelling decision is that a **capture** and a **bookmark** are
//! different things. A capture is an immutable record that a link arrived —
//! from Telegram, from the browser, from a vault backfill — and is never
//! rewritten. A bookmark is the deduplicated resource those captures point at.
//!
//! Sending yourself the same link on Telegram and on `WhatsApp` therefore
//! produces one bookmark and two captures. The duplicate disappears from view
//! while the fact that it was saved twice is retained, and that repeat-capture
//! count turns out to be the single strongest available signal that a link
//! actually matters.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Where a capture came from.
///
/// Stored as a string rather than an integer so a new surface can be added
/// without a migration, and so rows stay readable in a `SQLite` shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Forwarded to the Telegram bot.
    Telegram,
    /// Saved from the browser extension.
    Browser,
    /// Shared from the iOS share sheet.
    Shortcut,
    /// Imported from an Obsidian vault markdown dump.
    Vault,
    /// Imported from a browser's own bookmark store.
    BrowserImport,
    /// Created directly through the API.
    Api,
}

impl Source {
    /// The stored representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Telegram => "telegram",
            Self::Browser => "browser",
            Self::Shortcut => "shortcut",
            Self::Vault => "vault",
            Self::BrowserImport => "browser_import",
            Self::Api => "api",
        }
    }
}

impl std::str::FromStr for Source {
    type Err = crate::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "telegram" => Ok(Self::Telegram),
            "browser" => Ok(Self::Browser),
            "shortcut" => Ok(Self::Shortcut),
            "vault" => Ok(Self::Vault),
            "browser_import" => Ok(Self::BrowserImport),
            "api" => Ok(Self::Api),
            other => Err(crate::Error::msg(format!("unknown capture source: {other}"))),
        }
    }
}

/// An immutable record that a link arrived from somewhere.
///
/// Captures are append-only. Nothing in the pipeline rewrites one, which is
/// what makes the whole store reproducible: re-running normalization against a
/// newer ruleset rebuilds every bookmark from captures without data loss.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capture {
    /// Database identity.
    pub id: i64,
    /// The bookmark this capture resolved to.
    pub bookmark_id: i64,
    /// Exactly the URL text that arrived, before any normalization.
    pub raw_url: String,
    /// Where it arrived from.
    pub source: Source,
    /// Any text that accompanied the link — a Telegram message body, the
    /// vault file it was found in, the note typed into the share sheet.
    pub context: Option<String>,
    /// When it arrived.
    pub captured_at: DateTime<Utc>,
}

/// A deduplicated resource, identified by its canonical URL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    /// Database identity.
    pub id: i64,
    /// The canonical URL; unique across the store.
    pub canonical_url: String,
    /// The registrable domain, for cheap grouping without reparsing.
    pub domain: String,
    /// Page title, filled by enrichment.
    pub title: Option<String>,
    /// Page description, filled by enrichment.
    pub description: Option<String>,
    /// When this resource was first captured, from any source.
    pub first_captured_at: DateTime<Utc>,
    /// When it was most recently captured.
    pub last_captured_at: DateTime<Utc>,
    /// How many times it has been captured. Denormalized from the capture
    /// table because it is read on every ranked query and only ever
    /// incremented on write.
    pub capture_count: i64,
}

/// The outcome of ingesting one raw URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ingested {
    /// The canonical URL had not been seen; a bookmark was created.
    Created(i64),
    /// The canonical URL already existed; a capture was appended to it.
    Merged(i64),
}

impl Ingested {
    /// The bookmark's identity, however it was reached.
    #[must_use]
    pub fn bookmark_id(self) -> i64 {
        match self {
            Self::Created(id) | Self::Merged(id) => id,
        }
    }

    /// Whether this ingest created a new bookmark.
    #[must_use]
    pub fn is_new(self) -> bool {
        matches!(self, Self::Created(_))
    }
}

/// A link to ingest, before normalization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewCapture {
    /// The URL as it arrived.
    pub raw_url: String,
    /// Where it arrived from.
    pub source: Source,
    /// Any accompanying text.
    pub context: Option<String>,
    /// When it arrived. Defaults to now when absent, but a backfill should
    /// pass the real timestamp so recency features stay honest.
    pub captured_at: Option<DateTime<Utc>>,
}

impl NewCapture {
    /// Builds a capture with no context, arriving now.
    #[must_use]
    pub fn new(raw_url: impl Into<String>, source: Source) -> Self {
        Self { raw_url: raw_url.into(), source, context: None, captured_at: None }
    }

    /// Attaches accompanying text.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Sets the arrival time explicitly.
    #[must_use]
    pub fn at(mut self, captured_at: DateTime<Utc>) -> Self {
        self.captured_at = Some(captured_at);
        self
    }
}
