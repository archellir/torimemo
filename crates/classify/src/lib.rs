//! The distilled tag classifier.
//!
//! A capable model labels the corpus once, offline; this crate fits a small
//! local model to those labels and serves it. That is the whole teacher/student
//! split: the expensive model produces training data, and inference at request
//! time is a dot product against weights that live in the repository.
//!
//! Nothing here reaches the network. The crate depends only on `core`, so it
//! sits inside the serving path by construction.

pub mod model;
pub mod train;

pub use model::{Classifier, Prediction};
pub use train::{Config, Example, LabelReport, Report, train};
