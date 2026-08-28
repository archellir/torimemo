//! Fitting the classifier.
//!
//! Batch gradient descent on the logistic loss with L2 regularization, one
//! binary problem per tag. At this scale the whole fit is a few hundred
//! milliseconds, so there is no case for minibatching, and a closed-form
//! solver does not exist for logistic loss anyway.
//!
//! Two decisions matter more than the optimizer. **Rare labels are refused**
//! rather than fitted: a tag with four examples produces weights that describe
//! those four bookmarks and nothing else, and reporting a score for it would
//! be dishonest. And **evaluation is on data the model never saw**, split
//! before any fitting, because a training-set score on 1,248 examples measures
//! memorization.

use crate::model::Classifier;
use serde::Serialize;
use torimemo_core::{Error, Result};

/// One labelled example.
#[derive(Debug, Clone)]
pub struct Example {
    /// The feature vector, from the embedding model.
    pub vector: Vec<f32>,
    /// The tags the teacher assigned.
    pub labels: Vec<String>,
}

/// Training hyperparameters.
#[derive(Debug, Clone)]
pub struct Config {
    /// Gradient descent steps.
    pub epochs: usize,
    /// Step size.
    pub learning_rate: f32,
    /// L2 penalty. The dominant guard against overfitting here — 384 features
    /// against as few as 20 positive examples for a rare tag.
    pub l2: f32,
    /// A label needs at least this many positive examples to be fitted at all.
    pub min_support: usize,
    /// Fraction of examples held out for evaluation.
    pub holdout: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            epochs: 300,
            learning_rate: 0.5,
            l2: 0.01,
            // Twenty positives is roughly where a held-out estimate for one
            // label stops being noise at this corpus size.
            min_support: 20,
            holdout: 0.2,
        }
    }
}

/// How one label performed on held-out data.
#[derive(Debug, Clone, Serialize)]
pub struct LabelReport {
    /// The tag.
    pub label: String,
    /// Positive examples in the training split.
    pub support: usize,
    /// Of the ones predicted positive, how many were.
    pub precision: f32,
    /// Of the actual positives, how many were found.
    pub recall: f32,
    /// Their harmonic mean.
    pub f1: f32,
}

/// How training went.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Examples used to fit.
    pub trained_on: usize,
    /// Examples held out.
    pub evaluated_on: usize,
    /// Labels fitted.
    pub labels: usize,
    /// Labels refused for insufficient support, with their counts.
    pub skipped: Vec<(String, usize)>,
    /// Per-label held-out metrics, worst F1 first — the failures are what a
    /// reader needs to see.
    pub per_label: Vec<LabelReport>,
    /// F1 averaged over labels, each weighted equally.
    ///
    /// Macro rather than micro: micro-F1 on this corpus would be dominated by
    /// `video` and `programming` and would hide every rare tag doing badly.
    pub macro_f1: f32,
}

/// Fits a classifier and evaluates it on held-out data.
pub fn train(
    examples: &[Example],
    embedding_model: &str,
    teacher: &str,
    config: &Config,
) -> Result<(Classifier, Report)> {
    if examples.is_empty() {
        return Err(Error::msg("no training examples"));
    }

    let features = examples[0].vector.len();
    if examples.iter().any(|example| example.vector.len() != features) {
        return Err(Error::msg("training examples have inconsistent vector widths"));
    }

    // Deterministic split, so two runs are comparable and a reported score can
    // be reproduced. Deliberately *not* a plain stride: a fixed period aliases
    // with any periodic structure in the input, and when it does, a label can
    // land entirely in one split and vanish. Hashing the index scatters the
    // holdout without a random seed, keeping the split reproducible while
    // breaking that correlation.
    let holdout = config.holdout.clamp(0.05, 0.5);
    let cutoff = (holdout * u32::MAX as f32) as u32;
    type IndexedExamples<'a> = Vec<(usize, &'a Example)>;
    let (train_pairs, eval_pairs): (IndexedExamples<'_>, IndexedExamples<'_>) =
        examples.iter().enumerate().partition(|(index, _)| scatter(*index) >= cutoff);
    let train_set: Vec<&Example> = train_pairs.into_iter().map(|(_, example)| example).collect();
    let eval_set: Vec<&Example> = eval_pairs.into_iter().map(|(_, example)| example).collect();

    if train_set.is_empty() || eval_set.is_empty() {
        return Err(Error::msg("not enough examples to split into train and holdout"));
    }

    let mut vocabulary: Vec<String> =
        train_set.iter().flat_map(|example| example.labels.iter().cloned()).collect();
    vocabulary.sort_unstable();
    vocabulary.dedup();

    let mut labels = Vec::new();
    let mut weights = Vec::new();
    let mut biases = Vec::new();
    let mut skipped = Vec::new();
    let mut per_label = Vec::new();

    for label in &vocabulary {
        let targets: Vec<f32> = train_set
            .iter()
            .map(|example| f32::from(u8::from(example.labels.contains(label))))
            .collect();
        let support = targets.iter().filter(|target| **target > 0.5).count();

        if support < config.min_support {
            skipped.push((label.clone(), support));
            continue;
        }

        let (row, bias) = fit_one(&train_set, &targets, features, config);
        per_label.push(evaluate(label, &row, bias, &eval_set, support));

        labels.push(label.clone());
        weights.push(row);
        biases.push(bias);
    }

    if labels.is_empty() {
        return Err(Error::msg(format!(
            "no label had the {} positive examples required to fit",
            config.min_support
        )));
    }

    per_label.sort_by(|left, right| left.f1.total_cmp(&right.f1));
    let macro_f1 = per_label.iter().map(|report| report.f1).sum::<f32>() / per_label.len() as f32;

    let classifier = Classifier {
        labels,
        weights,
        biases,
        features,
        embedding_model: embedding_model.to_string(),
        teacher: teacher.to_string(),
        trained_at: chrono::Utc::now().to_rfc3339(),
        examples: train_set.len(),
    };

    let report = Report {
        trained_on: train_set.len(),
        evaluated_on: eval_set.len(),
        labels: classifier.labels.len(),
        skipped,
        per_label,
        macro_f1,
    };

    Ok((classifier, report))
}

/// Fits one binary logistic regression.
fn fit_one(
    examples: &[&Example],
    targets: &[f32],
    features: usize,
    config: &Config,
) -> (Vec<f32>, f32) {
    let mut weights = vec![0.0_f32; features];
    let mut bias = 0.0_f32;
    let count = examples.len() as f32;

    // Class weighting. Every tag is rare against the whole corpus — even
    // `video`, the most common, is a minority — so an unweighted fit converges
    // on predicting "no" for everything and scores well doing it. Scaling the
    // positive gradient by the imbalance ratio is what makes recall possible.
    let positives = targets.iter().filter(|target| **target > 0.5).count() as f32;
    let positive_weight = if positives > 0.0 { (count - positives) / positives } else { 1.0 };

    for _ in 0..config.epochs {
        let mut weight_gradient = vec![0.0_f32; features];
        let mut bias_gradient = 0.0_f32;

        for (example, target) in examples.iter().zip(targets) {
            let logit = dot(&weights, &example.vector) + bias;
            let predicted = 1.0 / (1.0 + (-logit.clamp(-30.0, 30.0)).exp());
            let error = predicted - target;
            let scale = if *target > 0.5 { positive_weight } else { 1.0 };

            for (gradient, feature) in weight_gradient.iter_mut().zip(&example.vector) {
                *gradient += scale * error * feature;
            }
            bias_gradient += scale * error;
        }

        for (weight, gradient) in weights.iter_mut().zip(&weight_gradient) {
            // The L2 term is applied to weights but not to the bias: penalizing
            // the intercept would bias the decision boundary toward the origin
            // for no principled reason.
            *weight -= config.learning_rate * (gradient / count + config.l2 * *weight);
        }
        bias -= config.learning_rate * bias_gradient / count;
    }

    (weights, bias)
}

/// Scores one label's weights against the held-out set.
fn evaluate(
    label: &str,
    weights: &[f32],
    bias: f32,
    eval_set: &[&Example],
    support: usize,
) -> LabelReport {
    let mut true_positives = 0.0_f32;
    let mut false_positives = 0.0_f32;
    let mut false_negatives = 0.0_f32;

    for example in eval_set {
        let logit = dot(weights, &example.vector) + bias;
        let predicted = logit > 0.0;
        let actual = example.labels.iter().any(|other| other == label);

        match (predicted, actual) {
            (true, true) => true_positives += 1.0,
            (true, false) => false_positives += 1.0,
            (false, true) => false_negatives += 1.0,
            (false, false) => {}
        }
    }

    // A label with no predictions and no actuals in the holdout scores zero
    // rather than one: there is no evidence it works, and reporting a perfect
    // score for an absence would be the wrong default.
    let precision = ratio(true_positives, true_positives + false_positives);
    let recall = ratio(true_positives, true_positives + false_negatives);
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    LabelReport { label: label.to_string(), support, precision, recall, f1 }
}

/// Maps an index to a well-distributed `u32`.
///
/// A finalizer-style integer hash: cheap, deterministic across runs and
/// platforms, and it decorrelates the split from any ordering in the corpus.
fn scatter(index: usize) -> u32 {
    let mut value = index as u32;
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    value
}

fn ratio(numerator: f32, denominator: f32) -> f32 {
    if denominator > 0.0 { numerator / denominator } else { 0.0 }
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a separable problem: the first feature decides the label.
    fn separable(count: usize) -> Vec<Example> {
        (0..count)
            .map(|index| {
                let positive = index % 2 == 0;
                Example {
                    vector: vec![if positive { 1.0 } else { -1.0 }, 0.1],
                    labels: if positive { vec!["yes".into()] } else { vec!["no".into()] },
                }
            })
            .collect()
    }

    #[test]
    fn learns_a_separable_problem() {
        let (classifier, report) = train(&separable(200), "m", "t", &Config::default()).unwrap();

        assert_eq!(report.labels, 2);
        // A linearly separable problem should be solved essentially perfectly;
        // anything less means the optimizer is broken.
        assert!(report.macro_f1 > 0.95, "macro F1 was {}", report.macro_f1);

        let predictions = classifier.predict(&[1.0, 0.1], 0.5).unwrap();
        assert_eq!(predictions[0].tag, "yes");
    }

    #[test]
    fn refuses_labels_without_enough_support() {
        let mut examples = separable(200);
        // A handful of positives, spread so the split cannot swallow them all.
        for index in [3, 17, 41, 88, 150] {
            examples[index].labels.push("rare".into());
        }

        let (classifier, report) = train(&examples, "m", "t", &Config::default()).unwrap();
        assert!(!classifier.labels.contains(&"rare".to_string()));
        assert!(report.skipped.iter().any(|(label, _)| label == "rare"));
    }

    #[test]
    fn evaluation_uses_held_out_examples() {
        let (_, report) = train(&separable(200), "m", "t", &Config::default()).unwrap();
        assert!(report.evaluated_on > 0);
        assert_eq!(report.trained_on + report.evaluated_on, 200);
    }

    #[test]
    fn the_split_is_deterministic() {
        let first = train(&separable(200), "m", "t", &Config::default()).unwrap().1;
        let second = train(&separable(200), "m", "t", &Config::default()).unwrap().1;
        assert_eq!(first.trained_on, second.trained_on);
        assert!((first.macro_f1 - second.macro_f1).abs() < f32::EPSILON);
    }

    #[test]
    fn records_the_embedding_model_it_was_fitted_against() {
        let (classifier, _) =
            train(&separable(200), "bge-small-en-v1.5", "haiku", &Config::default()).unwrap();
        assert_eq!(classifier.embedding_model, "bge-small-en-v1.5");
        assert_eq!(classifier.teacher, "haiku");
    }

    #[test]
    fn per_label_metrics_are_worst_first() {
        let (_, report) = train(&separable(200), "m", "t", &Config::default()).unwrap();
        for pair in report.per_label.windows(2) {
            assert!(pair[0].f1 <= pair[1].f1);
        }
    }

    #[test]
    fn the_split_does_not_alias_with_periodic_labels() {
        // Every tenth example carries the label. A plain stride-of-5 holdout
        // would put every one of them in the same split; the scattered split
        // must not.
        let examples: Vec<Example> = (0..500)
            .map(|index| Example {
                vector: vec![if index % 10 == 0 { 1.0 } else { -1.0 }, 0.1],
                labels: if index % 10 == 0 {
                    vec!["periodic".into()]
                } else {
                    vec!["other".into()]
                },
            })
            .collect();

        let (classifier, _) = train(&examples, "m", "t", &Config::default()).unwrap();
        assert!(
            classifier.labels.contains(&"periodic".to_string()),
            "a periodic label was lost to the split"
        );
    }

    #[test]
    fn an_empty_training_set_is_an_error() {
        assert!(train(&[], "m", "t", &Config::default()).is_err());
    }

    #[test]
    fn inconsistent_vector_widths_are_rejected() {
        let examples = vec![
            Example { vector: vec![1.0, 0.0], labels: vec!["a".into()] },
            Example { vector: vec![1.0], labels: vec!["a".into()] },
        ];
        assert!(train(&examples, "m", "t", &Config::default()).is_err());
    }

    #[test]
    fn a_corpus_where_no_label_clears_support_is_an_error_not_an_empty_model() {
        let examples: Vec<Example> = (0..30)
            .map(|index| Example { vector: vec![1.0, 0.0], labels: vec![format!("tag{index}")] })
            .collect();
        assert!(train(&examples, "m", "t", &Config::default()).is_err());
    }

    #[test]
    fn class_weighting_lets_a_minority_label_be_learned() {
        // One positive in ten, which is roughly the real ratio for a mid-
        // frequency tag. Without positive-class weighting the fit collapses to
        // predicting "no" everywhere and recall goes to zero.
        let examples: Vec<Example> = (0..400)
            .map(|index| {
                let positive = index % 7 == 0;
                Example {
                    vector: vec![if positive { 1.0 } else { -1.0 }, 0.1],
                    labels: if positive {
                        vec!["rare".into(), "common".into()]
                    } else {
                        vec!["common".into()]
                    },
                }
            })
            .collect();

        let (_, report) = train(&examples, "m", "t", &Config::default()).unwrap();
        let rare = report.per_label.iter().find(|entry| entry.label == "rare").unwrap();
        assert!(rare.recall > 0.9, "recall on the minority label was {}", rare.recall);
    }
}
