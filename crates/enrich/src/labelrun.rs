//! The labelling pass and training-set export.
//!
//! Resumable and idempotent for the same reason the fetch pass is: the queue
//! is defined by the absence of a row for this model, so an interrupted run
//! continues where it stopped and a completed one does nothing. Switching
//! models re-queues the corpus without touching the previous model's labels,
//! which is what makes two labellers comparable on the same bookmarks.

use crate::label::Labeller;
use serde::Serialize;
use torimemo_core::{Result, Store};

/// What a labelling pass did.
#[derive(Debug, Default, Clone, Copy)]
pub struct Summary {
    /// Bookmarks that received at least one tag.
    pub labelled: usize,
    /// Bookmarks the labeller declined to tag — thin metadata, usually.
    pub skipped: usize,
    /// Bookmarks whose labelling call failed.
    pub failed: usize,
}

impl Summary {
    /// Total processed.
    #[must_use]
    pub fn total(&self) -> usize {
        self.labelled + self.skipped + self.failed
    }
}

/// Runs a labelling pass over bookmarks this model has not seen.
///
/// Sequential by design. The corpus is a few thousand rows labelled once, so
/// wall-clock is not the constraint; a single ordered stream keeps the run
/// trivially interruptible and stays well inside any rate limit without a
/// backoff policy to get wrong.
pub fn run(
    store: &mut Store,
    labeller: &impl Labeller,
    limit: usize,
    mut progress: impl FnMut(&Summary),
) -> Result<Summary> {
    let pending = store.needing_labels(labeller.model(), limit)?;
    let mut summary = Summary::default();

    for bookmark in pending {
        match labeller.label(&bookmark) {
            Ok(proposal) if proposal.tags.is_empty() => {
                summary.skipped += 1;
            }
            Ok(proposal) => {
                store.set_tags(
                    bookmark.id,
                    &proposal.tags,
                    "model",
                    Some(proposal.confidence),
                    Some(labeller.model()),
                )?;
                summary.labelled += 1;
            }
            Err(_) => {
                // A failed row simply stays in the queue: it has no label for
                // this model, so the next pass picks it up. Nothing to record.
                summary.failed += 1;
            }
        }
        progress(&summary);
    }

    Ok(summary)
}

/// One row of the exported training set.
#[derive(Debug, Clone, Serialize)]
pub struct Example {
    /// The bookmark's identity, so a row can be traced back.
    pub id: i64,
    /// The text a classifier sees — the same text the labeller saw, so the
    /// student is trained on the teacher's actual inputs.
    pub text: String,
    /// The teacher's labels.
    pub labels: Vec<String>,
    /// The domain, kept as a feature in its own right.
    pub domain: String,
    /// How many times the link was captured.
    pub capture_count: i64,
}

/// Exports every labelled bookmark as JSON Lines.
///
/// JSONL rather than a single array: it streams, it appends, and every tool
/// that will consume this — a training script, a spot check, `wc -l` — reads
/// it line at a time.
pub fn export_training_set(store: &Store, model: &str) -> Result<String> {
    let labelled = store.labelled(model)?;
    let mut lines = Vec::with_capacity(labelled.len());

    for (bookmark, labels) in labelled {
        let example = Example {
            id: bookmark.id,
            text: crate::label::label_text(&bookmark),
            labels,
            domain: bookmark.domain.clone(),
            capture_count: bookmark.capture_count,
        };
        lines.push(serde_json::to_string(&example)?);
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label::RuleBased;
    use torimemo_core::{NewCapture, Source};

    fn store_with_titled(urls: &[(&str, &str)]) -> Store {
        let mut store = Store::open_in_memory().unwrap();
        for (url, title) in urls {
            let id = store.ingest(&NewCapture::new(*url, Source::Api)).unwrap().bookmark_id();
            store.set_metadata(id, Some(title), None).unwrap();
        }
        store
    }

    #[test]
    fn labelling_writes_tags_and_drains_the_queue() {
        let mut store = store_with_titled(&[("https://github.com/a/b", "a repo")]);

        let summary = run(&mut store, &RuleBased, 100, |_| {}).unwrap();
        assert_eq!(summary.labelled, 1);
        assert!(store.tags(1).unwrap().contains(&"open-source".to_string()));

        // Idempotent: the row now has a label for this model.
        assert_eq!(run(&mut store, &RuleBased, 100, |_| {}).unwrap().total(), 0);
    }

    #[test]
    fn an_untitled_bookmark_is_never_queued() {
        let mut store = Store::open_in_memory().unwrap();
        store.ingest(&NewCapture::new("https://github.com/a/b", Source::Api)).unwrap();
        assert!(store.needing_labels("rules-v1", 10).unwrap().is_empty());
    }

    #[test]
    fn a_human_tag_survives_a_model_relabel() {
        let mut store = store_with_titled(&[("https://github.com/a/b", "a repo")]);
        store.set_tags(1, &["motorcycle".into()], "human", None, None).unwrap();

        run(&mut store, &RuleBased, 100, |_| {}).unwrap();

        // The correction the user made is still there alongside the model's.
        let tags = store.tags(1).unwrap();
        assert!(tags.contains(&"motorcycle".to_string()), "human tag was lost");
        assert!(tags.contains(&"open-source".to_string()));
    }

    #[test]
    fn a_relabel_replaces_the_model_tags_rather_than_accumulating() {
        let mut store = store_with_titled(&[("https://github.com/a/b", "a repo")]);
        store.set_tags(1, &["gaming".into()], "model", Some(0.1), Some("rules-v1")).unwrap();
        store.set_tags(1, &["design".into()], "model", Some(0.1), Some("rules-v1")).unwrap();

        assert_eq!(store.tags(1).unwrap(), vec!["design"]);
    }

    #[test]
    fn export_produces_one_json_line_per_labelled_bookmark() {
        let mut store = store_with_titled(&[
            ("https://github.com/a/b", "a repo"),
            ("https://leetcode.com/x", "two sum"),
        ]);
        run(&mut store, &RuleBased, 100, |_| {}).unwrap();

        let exported = export_training_set(&store, "rules-v1").unwrap();
        let lines: Vec<&str> = exported.lines().collect();
        assert_eq!(lines.len(), 2);

        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert!(first["labels"].as_array().is_some_and(|labels| !labels.is_empty()));
        assert!(first["text"].as_str().unwrap().contains("Site:"));
    }

    #[test]
    fn export_of_an_unlabelled_model_is_empty() {
        let store = store_with_titled(&[("https://github.com/a/b", "a repo")]);
        assert!(export_training_set(&store, "never-ran").unwrap().is_empty());
    }
}
