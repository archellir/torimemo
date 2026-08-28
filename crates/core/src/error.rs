//! The crate's error type.

use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

/// The result type returned throughout the crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error produced by any core operation.
///
/// Deliberately opaque: callers get a message and a source chain rather than a
/// variant to match on. The API layer reports these; nothing branches on kind.
#[derive(Debug)]
pub struct Error {
    message: String,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl Error {
    /// Builds an error from a message that has no underlying cause.
    pub fn msg(message: impl Into<String>) -> Self {
        Self { message: message.into(), source: None }
    }

    /// Builds an error that explains `message` and keeps `source` reachable.
    pub fn with_source(
        message: impl Into<String>,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self { message: message.into(), source: Some(Box::new(source)) }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_ref().map(|source| &**source as &(dyn StdError + 'static))
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::with_source(error.to_string(), error)
    }
}

impl From<url::ParseError> for Error {
    fn from(error: url::ParseError) -> Self {
        Self::with_source(error.to_string(), error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::with_source(error.to_string(), error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::with_source(error.to_string(), error)
    }
}
