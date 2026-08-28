//! Local text embeddings and vector recall.
//!
//! Everything here runs in-process. There is no embedding service to reach and
//! no API key to hold: the ONNX model is fetched once into a cache directory
//! and then inference is local forever after, which is what keeps the serving
//! path deterministic and offline.
//!
//! The provider is an enum rather than a boxed trait so that the deterministic
//! fallback compiles even when the `local` feature is off — CI and the unit
//! tests run against it, and neither needs a model on disk.

pub mod provider;
pub mod recall;

pub use provider::{Embedder, Embedding, Provider};
pub use recall::{Match, backfill, cosine, embed_text, input_hash, rank_by_similarity};
