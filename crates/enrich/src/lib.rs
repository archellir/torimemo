//! Offline enrichment: page metadata now, model-backed labelling next.
//!
//! This is the only crate permitted to reach the network, and nothing in the
//! serving path depends on it. That is the akunaki boundary expressed as a
//! dependency graph: `api` cannot call an LLM because it cannot see one, and
//! the product works fully with this crate absent — bookmarks are captured,
//! deduplicated, embedded, and recalled without it ever running.

pub mod extract;
pub mod fetch;
pub mod label;
pub mod labelrun;
pub mod stars;
pub mod taxonomy;
pub mod worker;

pub use extract::Metadata;
pub use fetch::{Fetcher, Outcome};
pub use label::{AnthropicLabeller, Labeller, Proposal, RuleBased};
pub use labelrun::{Example, export_training_set};
pub use worker::{Config, Summary, run};
