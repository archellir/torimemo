//! Embedding providers.

use torimemo_core::{Error, Result};

/// A vector and the model that produced it.
///
/// The model name travels with the vector because it is written into the
/// store as part of the primary key: re-embedding with a newer model inserts
/// alongside the old vector rather than overwriting it, so a model upgrade
/// stays a diff.
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    /// The model identifier, as stored.
    pub model: String,
    /// The vector itself, L2-normalized.
    pub vector: Vec<f32>,
}

impl Embedding {
    /// The vector's width.
    #[must_use]
    pub fn dimensions(&self) -> usize {
        self.vector.len()
    }
}

/// Something that turns text into a vector.
pub trait Embedder {
    /// The model identifier written alongside stored vectors.
    fn model(&self) -> &str;
    /// The width of the vectors this produces.
    fn dimensions(&self) -> usize;
    /// Embeds a batch. Batching matters: ONNX inference amortizes badly over
    /// single inputs, and the backfill embeds thousands of rows.
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Embedding>>;

    /// Embeds one text.
    fn embed(&self, text: &str) -> Result<Embedding> {
        let mut embeddings = self.embed_batch(&[text.to_string()])?;
        embeddings.pop().ok_or_else(|| Error::msg("embedder returned no vector"))
    }
}

/// The configured provider.
#[derive(Debug)]
pub enum Provider {
    /// Hash-based vectors with no model behind them.
    Deterministic(Deterministic),
    /// In-process ONNX inference. Boxed because the ONNX session dwarfs the
    /// deterministic variant, and every `Provider` would otherwise carry that
    /// footprint whether or not a model is loaded.
    #[cfg(feature = "local")]
    Local(Box<Local>),
}

impl Provider {
    /// Builds the local ONNX provider, downloading the model on first use.
    #[cfg(feature = "local")]
    pub fn local(model: &str) -> Result<Self> {
        Ok(Self::Local(Box::new(Local::new(model)?)))
    }

    /// Builds the deterministic provider.
    #[must_use]
    pub fn deterministic() -> Self {
        Self::Deterministic(Deterministic::default())
    }
}

impl Embedder for Provider {
    fn model(&self) -> &str {
        match self {
            Self::Deterministic(provider) => provider.model(),
            #[cfg(feature = "local")]
            Self::Local(provider) => provider.model(),
        }
    }

    fn dimensions(&self) -> usize {
        match self {
            Self::Deterministic(provider) => provider.dimensions(),
            #[cfg(feature = "local")]
            Self::Local(provider) => provider.dimensions(),
        }
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Embedding>> {
        match self {
            Self::Deterministic(provider) => provider.embed_batch(texts),
            #[cfg(feature = "local")]
            Self::Local(provider) => provider.embed_batch(texts),
        }
    }
}

/// Token-hash vectors, with no model behind them.
///
/// This exists so the pipeline is testable and so the binary works with the
/// `local` feature off. It captures lexical overlap and nothing else — two
/// texts about the same idea in different words score zero — so it is a
/// fallback, never a substitute for real embeddings.
#[derive(Debug, Clone)]
pub struct Deterministic {
    dimensions: usize,
}

impl Default for Deterministic {
    fn default() -> Self {
        Self { dimensions: 384 }
    }
}

impl Embedder for Deterministic {
    fn model(&self) -> &str {
        "deterministic-v1"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Embedding>> {
        Ok(texts
            .iter()
            .map(|text| {
                let mut vector = vec![0.0_f32; self.dimensions];
                for token in text.split(|character: char| !character.is_alphanumeric()) {
                    if token.is_empty() {
                        continue;
                    }
                    let digest = blake3::hash(token.to_lowercase().as_bytes());
                    let bytes = digest.as_bytes();
                    let index =
                        usize::from(u16::from_le_bytes([bytes[0], bytes[1]])) % self.dimensions;
                    // Sign from a second byte so unrelated tokens cancel
                    // rather than accumulating into a single positive bias.
                    let sign = if bytes[2].is_multiple_of(2) { 1.0 } else { -1.0 };
                    vector[index] += sign;
                }
                normalize(&mut vector);
                Embedding { model: self.model().to_string(), vector }
            })
            .collect())
    }
}

/// In-process ONNX embeddings via fastembed.
///
/// The engine is built once and reused: model load dominates the cost of a
/// single embed, and the backfill would otherwise pay it thousands of times.
#[cfg(feature = "local")]
pub struct Local {
    name: String,
    dimensions: usize,
    // `TextEmbedding::embed` takes `&mut self`, but the provider is shared
    // across the API's request handlers, so the mutability is confined here.
    engine: std::sync::Mutex<fastembed::TextEmbedding>,
}

// `fastembed::TextEmbedding` holds an ONNX session and is not `Debug`, so the
// derive is written out by hand to keep the workspace's
// `missing_debug_implementations` lint satisfied without leaking the engine's
// internals into the output.
#[cfg(feature = "local")]
impl std::fmt::Debug for Local {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Local")
            .field("name", &self.name)
            .field("dimensions", &self.dimensions)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "local")]
impl Local {
    /// Loads `model`, downloading it on first use.
    pub fn new(model: &str) -> Result<Self> {
        let (embedding_model, dimensions) = resolve(model)?;
        let options =
            fastembed::TextInitOptions::new(embedding_model).with_show_download_progress(false);
        let engine = fastembed::TextEmbedding::try_new(options).map_err(|error| {
            Error::msg(format!("could not load embedding model {model}: {error}"))
        })?;
        Ok(Self { name: model.to_string(), dimensions, engine: std::sync::Mutex::new(engine) })
    }
}

#[cfg(feature = "local")]
impl Embedder for Local {
    fn model(&self) -> &str {
        &self.name
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Embedding>> {
        let mut engine =
            self.engine.lock().map_err(|_| Error::msg("embedding engine mutex was poisoned"))?;
        let vectors = engine
            .embed(texts, None)
            .map_err(|error| Error::msg(format!("embedding failed: {error}")))?;

        Ok(vectors
            .into_iter()
            .map(|mut vector| {
                normalize(&mut vector);
                Embedding { model: self.name.clone(), vector }
            })
            .collect())
    }
}

/// Maps a user-facing model name to fastembed's identifier and native width.
#[cfg(feature = "local")]
fn resolve(name: &str) -> Result<(fastembed::EmbeddingModel, usize)> {
    match name {
        "bge-small-en-v1.5" => Ok((fastembed::EmbeddingModel::BGESmallENV15, 384)),
        "bge-base-en-v1.5" => Ok((fastembed::EmbeddingModel::BGEBaseENV15, 768)),
        "all-minilm-l6-v2" => Ok((fastembed::EmbeddingModel::AllMiniLML6V2, 384)),
        // The vault's links are not all English — there is Russian and Spanish
        // throughout the Telegram dump — so a multilingual model is a
        // reasonable default for this corpus specifically.
        "multilingual-e5-small" => Ok((fastembed::EmbeddingModel::MultilingualE5Small, 384)),
        other => Err(Error::msg(format!("unknown embedding model: {other}"))),
    }
}

/// Scales a vector to unit length, so cosine similarity is a dot product.
fn normalize(vector: &mut [f32]) {
    let magnitude = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if magnitude > f32::EPSILON {
        for value in vector.iter_mut() {
            *value /= magnitude;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_vectors_are_unit_length() {
        let provider = Deterministic::default();
        let embedding = provider.embed("rust deterministic builds").unwrap();
        let magnitude: f32 = embedding.vector.iter().map(|value| value * value).sum();
        assert!((magnitude - 1.0).abs() < 1e-5, "magnitude was {magnitude}");
    }

    #[test]
    fn deterministic_provider_is_actually_deterministic() {
        let provider = Deterministic::default();
        assert_eq!(provider.embed("same text").unwrap(), provider.embed("same text").unwrap());
    }

    #[test]
    fn batch_and_single_agree() {
        let provider = Deterministic::default();
        let batch = provider.embed_batch(&["one".into(), "two".into()]).unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0], provider.embed("one").unwrap());
    }

    #[test]
    fn empty_text_yields_a_zero_vector_rather_than_nan() {
        let provider = Deterministic::default();
        let embedding = provider.embed("").unwrap();
        assert!(embedding.vector.iter().all(|value| value.abs() < f32::EPSILON));
    }
}
