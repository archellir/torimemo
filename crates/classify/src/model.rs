//! The classifier: weights, prediction, and serialization.
//!
//! Multi-label logistic regression over sentence embeddings, one independent
//! binary classifier per tag. A bookmark genuinely carries several tags at
//! once — a Rust tutorial video is `programming`, `tutorial`, and `video` —
//! so softmax over a single label would be the wrong shape. One-vs-rest says
//! exactly what the data says.
//!
//! At 1,248 examples and 384 features, regularized logistic regression is the
//! right capacity. A deeper model would fit the training set better and
//! generalize worse, and the honest constraint here is label count, not
//! model expressiveness.

use serde::{Deserialize, Serialize};
use torimemo_core::{Error, Result};

/// A trained tag classifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classifier {
    /// The tag each row of `weights` predicts, in order.
    pub labels: Vec<String>,
    /// One weight vector per label, each `features` long.
    pub weights: Vec<Vec<f32>>,
    /// One bias per label.
    pub biases: Vec<f32>,
    /// Input width; a vector of any other width is rejected.
    pub features: usize,
    /// The embedding model these weights were trained against.
    ///
    /// Weights are only meaningful in the vector space they were fitted in,
    /// so predicting with vectors from a different embedding model is a bug.
    /// Recording the name here is what lets [`Self::predict`] refuse to.
    pub embedding_model: String,
    /// The labeller whose output was the training target.
    pub teacher: String,
    /// When it was trained.
    pub trained_at: String,
    /// How many examples it was fitted on.
    pub examples: usize,
}

/// A predicted tag and its probability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    /// The tag.
    pub tag: String,
    /// Probability in `[0, 1]`.
    pub probability: f32,
}

impl Classifier {
    /// Predicts tags for one embedding, above `threshold`, highest first.
    ///
    /// Errors when the vector's width does not match the trained width, which
    /// means it came from a different embedding model.
    pub fn predict(&self, vector: &[f32], threshold: f32) -> Result<Vec<Prediction>> {
        if vector.len() != self.features {
            return Err(Error::msg(format!(
                "classifier expects {}-dimensional vectors, got {}; \
                 this vector is probably from a different embedding model",
                self.features,
                vector.len()
            )));
        }

        let mut predictions: Vec<Prediction> = self
            .labels
            .iter()
            .enumerate()
            .filter_map(|(index, tag)| {
                let probability = sigmoid(dot(&self.weights[index], vector) + self.biases[index]);
                (probability >= threshold).then(|| Prediction { tag: tag.clone(), probability })
            })
            .collect();

        predictions.sort_by(|left, right| right.probability.total_cmp(&left.probability));
        Ok(predictions)
    }

    /// Predicts at most `top_k` tags, ignoring the threshold.
    ///
    /// Useful when something must be assigned — a suggestion box that would
    /// otherwise be empty — where an uncertain tag beats none.
    pub fn predict_top(&self, vector: &[f32], top_k: usize) -> Result<Vec<Prediction>> {
        let mut predictions = self.predict(vector, 0.0)?;
        predictions.truncate(top_k);
        Ok(predictions)
    }

    /// Serializes to pretty JSON.
    ///
    /// JSON rather than a binary format: the whole model is a few hundred
    /// kilobytes, it belongs in git beside the code that produced it, and a
    /// weight matrix you can open and read is worth more than the bytes saved.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(Error::from)
    }

    /// Parses a serialized classifier, checking its internal consistency.
    pub fn from_json(json: &str) -> Result<Self> {
        let classifier: Self = serde_json::from_str(json)?;

        if classifier.labels.len() != classifier.weights.len()
            || classifier.labels.len() != classifier.biases.len()
        {
            return Err(Error::msg("classifier has mismatched labels, weights, and biases"));
        }
        if classifier.weights.iter().any(|row| row.len() != classifier.features) {
            return Err(Error::msg("a weight row does not match the declared feature width"));
        }

        Ok(classifier)
    }
}

/// The logistic function.
fn sigmoid(value: f32) -> f32 {
    // Clamped because `exp` of a large negative overflows to infinity, and a
    // saturated probability is the correct answer there anyway.
    1.0 / (1.0 + (-value.clamp(-30.0, 30.0)).exp())
}

/// Dot product of two equal-length vectors.
fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classifier() -> Classifier {
        Classifier {
            labels: vec!["yes".into(), "no".into()],
            // First label fires on a positive first feature, second on negative.
            weights: vec![vec![10.0, 0.0], vec![-10.0, 0.0]],
            biases: vec![0.0, 0.0],
            features: 2,
            embedding_model: "test".into(),
            teacher: "test".into(),
            trained_at: "2026-01-01T00:00:00Z".into(),
            examples: 2,
        }
    }

    #[test]
    fn predicts_the_label_whose_weights_match() {
        let predictions = classifier().predict(&[1.0, 0.0], 0.5).unwrap();
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].tag, "yes");
        assert!(predictions[0].probability > 0.99);
    }

    #[test]
    fn predictions_come_back_highest_first() {
        let predictions = classifier().predict(&[1.0, 0.0], 0.0).unwrap();
        assert!(predictions[0].probability >= predictions[1].probability);
    }

    #[test]
    fn a_threshold_filters_weak_predictions() {
        assert!(classifier().predict(&[0.0, 0.0], 0.9).unwrap().is_empty());
    }

    #[test]
    fn predict_top_returns_at_most_k() {
        assert_eq!(classifier().predict_top(&[1.0, 0.0], 1).unwrap().len(), 1);
    }

    #[test]
    fn a_wrong_width_vector_is_rejected_rather_than_silently_scored() {
        let error = classifier().predict(&[1.0], 0.5).unwrap_err();
        assert!(error.to_string().contains("different embedding model"));
    }

    #[test]
    fn round_trips_through_json() {
        let original = classifier();
        let parsed = Classifier::from_json(&original.to_json().unwrap()).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn rejects_a_model_with_mismatched_shapes() {
        let mut broken = classifier();
        broken.biases.pop();
        let json = serde_json::to_string(&broken).unwrap();
        assert!(Classifier::from_json(&json).is_err());
    }

    #[test]
    fn rejects_a_weight_row_of_the_wrong_width() {
        let mut broken = classifier();
        broken.weights[0] = vec![1.0];
        let json = serde_json::to_string(&broken).unwrap();
        assert!(Classifier::from_json(&json).is_err());
    }

    #[test]
    fn sigmoid_saturates_without_overflowing() {
        assert!(sigmoid(1000.0).is_finite());
        assert!(sigmoid(-1000.0).is_finite());
        assert!((sigmoid(0.0) - 0.5).abs() < f32::EPSILON);
    }
}
