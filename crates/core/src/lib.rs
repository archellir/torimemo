//! Torimemo's deterministic core: capture, normalize, dedupe, store.
//!
//! This crate holds everything that must be reproducible. Given the same
//! captures it produces the same bookmarks, and it depends on no model, no
//! network, and no clock beyond the timestamps its callers hand it. The
//! model-backed work — embedding, tagging, ranking — reads from here and
//! writes back through narrow, versioned interfaces, so a model change is
//! always visible as a diff rather than as a silent rewrite.

pub mod error;
pub mod import;
pub mod model;
pub mod normalize;
pub mod store;
pub mod token;

pub use error::{Error, Result};
pub use model::{Bookmark, Capture, Ingested, NewCapture, Source};
pub use normalize::{Canonical, canonicalize};
pub use store::{BatchOutcome, Stats, Store};
pub use token::{Issued, Principal, Scope, TokenInfo};
