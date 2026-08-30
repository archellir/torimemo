//! End-to-end tests for the capture pipeline.
//!
//! Every other test in this workspace lives inside a single crate and stubs
//! whatever sits on the other side of a boundary. These do not: they drive
//! real types across `core`, `embed`, `classify`, and `enrich`, which is the
//! only place a mismatch between them can surface.
//!
//! Nothing here touches the network. The embedding provider is the
//! deterministic one and the labeller is the rule-based one, so the suite runs
//! offline and produces the same result on every machine.

use torimemo_classify::{Config as TrainConfig, Example, train};
use torimemo_core::{NewCapture, Source, Store};
use torimemo_embed::{Embedder, Provider, backfill, rank_by_similarity};
use torimemo_enrich::label::{Labeller, RuleBased};

/// A store holding a small corpus, titled as the enrichment pass would leave it.
fn corpus() -> Store {
    let mut store = Store::open_in_memory().expect("in-memory store");
    let pages = [
        ("https://github.com/paperless-ngx/paperless-ngx", "Paperless document management"),
        ("https://caddyserver.com/", "Caddy - automatic HTTPS web server"),
        ("https://leetcode.com/problems/two-sum", "Two Sum - coding interview practice"),
        ("https://www.youtube.com/watch?v=abc", "Rust in 100 seconds"),
    ];
    for (url, title) in pages {
        let id = store.ingest(&NewCapture::new(url, Source::Api)).expect("ingest").bookmark_id();
        store.set_metadata(id, Some(title), None).expect("metadata");
    }
    store
}

#[test]
fn a_link_captured_twice_is_one_bookmark_and_stays_findable() {
    let mut store = Store::open_in_memory().unwrap();

    // The same resource, as it arrives from two different surfaces.
    let first =
        store.ingest(&NewCapture::new("https://example.com/article", Source::Telegram)).unwrap();
    let second = store
        .ingest(&NewCapture::new(
            "https://example.com/article?utm_source=newsletter&fbclid=xyz",
            Source::Browser,
        ))
        .unwrap();

    assert_eq!(first.bookmark_id(), second.bookmark_id(), "dedupe should collapse these");
    assert!(!second.is_new());

    let stats = store.stats().unwrap();
    assert_eq!(stats.bookmarks, 1);
    assert_eq!(stats.captures, 2, "both captures are kept as evidence it mattered twice");
}

#[test]
fn ingest_then_embed_then_recall_returns_the_right_bookmark() {
    let store = corpus();
    let embedder = Provider::deterministic();

    let embedded = backfill(&store, &embedder, 16, |_| {}).unwrap();
    assert_eq!(embedded, 4);

    let matches = rank_by_similarity(&store, &embedder, "document management", 3, 0.0).unwrap();
    assert!(!matches.is_empty(), "recall found nothing");
    assert!(
        matches[0].bookmark.canonical_url.contains("paperless"),
        "expected paperless first, got {}",
        matches[0].bookmark.canonical_url
    );
}

#[test]
fn metadata_arriving_late_invalidates_the_vector_it_contradicts() {
    // The ordering that actually happens: a link is captured and embedded from
    // its URL alone, then enrichment fetches a title. The stored vector no
    // longer describes the row, so it has to be recomputed rather than left to
    // silently disagree with the bookmark it indexes.
    let mut store = Store::open_in_memory().unwrap();
    let embedder = Provider::deterministic();

    let id =
        store.ingest(&NewCapture::new("https://example.com/x", Source::Api)).unwrap().bookmark_id();
    backfill(&store, &embedder, 16, |_| {}).unwrap();
    assert_eq!(store.stats().unwrap().embedded, 1);

    store.set_metadata(id, Some("A title that changes everything"), None).unwrap();
    store.clear_embeddings(id).unwrap();

    assert_eq!(store.stats().unwrap().embedded, 0, "the stale vector should be gone");
    assert_eq!(backfill(&store, &embedder, 16, |_| {}).unwrap(), 1, "and re-queued");
}

#[test]
fn labelling_then_training_produces_a_classifier_that_predicts() {
    let mut store = corpus();
    let embedder = Provider::deterministic();
    let labeller = RuleBased;

    backfill(&store, &embedder, 16, |_| {}).unwrap();

    // Label with the offline baseline, exactly as the real pass would.
    for (bookmark, _) in store.all_with_fetch_status().unwrap() {
        let proposal = labeller.label(&bookmark).unwrap();
        if !proposal.tags.is_empty() {
            store
                .set_tags(
                    bookmark.id,
                    &proposal.tags,
                    "model",
                    Some(proposal.confidence),
                    Some(labeller.model()),
                )
                .unwrap();
        }
    }

    let vectors: std::collections::HashMap<i64, Vec<f32>> =
        store.embeddings(embedder.model()).unwrap().into_iter().collect();
    let examples: Vec<Example> = store
        .labelled(labeller.model())
        .unwrap()
        .into_iter()
        .filter_map(|(bookmark, labels)| {
            vectors.get(&bookmark.id).map(|vector| Example { vector: vector.clone(), labels })
        })
        .collect();

    assert!(!examples.is_empty(), "labelling produced no training examples");

    // Four bookmarks cannot clear the default support floor, and refusing is
    // the correct answer — a label fitted on two examples describes those two.
    let config = TrainConfig { min_support: 1, ..TrainConfig::default() };
    let (classifier, report) =
        train(&examples, embedder.model(), labeller.model(), &config).unwrap();

    assert!(report.labels > 0, "no label was fitted");
    assert_eq!(classifier.embedding_model, embedder.model(), "lineage must be recorded");

    let query = embedder.embed("document management").unwrap();
    assert!(classifier.predict(&query.vector, 0.0).is_ok());
}

#[test]
fn a_classifier_refuses_vectors_from_a_different_embedding_model() {
    // The failure this prevents: swapping the embedding model and scoring old
    // vectors against new weights, which produces confident nonsense rather
    // than an error.
    let examples: Vec<Example> = (0..40)
        .map(|index| Example {
            vector: vec![if index % 2 == 0 { 1.0 } else { -1.0 }, 0.5],
            labels: vec![if index % 2 == 0 { "yes".into() } else { "no".into() }],
        })
        .collect();

    let config = TrainConfig { min_support: 5, ..TrainConfig::default() };
    let (classifier, _) = train(&examples, "model-a", "rules", &config).unwrap();

    assert!(classifier.predict(&[1.0, 0.5], 0.5).is_ok());
    let wrong_width = classifier.predict(&[1.0, 0.5, 0.5], 0.5);
    assert!(wrong_width.is_err(), "a mismatched width must not be scored");
}

#[test]
fn pruning_removes_a_bookmark_and_everything_hanging_off_it() {
    let mut store = corpus();
    let embedder = Provider::deterministic();
    backfill(&store, &embedder, 16, |_| {}).unwrap();

    let victim = store.bookmark_by_url("https://caddyserver.com/").unwrap().unwrap();
    store.set_tags(victim.id, &["tool".into()], "model", Some(0.9), Some("rules")).unwrap();
    store.record_event(victim.id, "opened", None, None).unwrap();

    let before = store.stats().unwrap();
    assert_eq!(store.delete_bookmarks(&[victim.id]).unwrap(), 1);
    let after = store.stats().unwrap();

    assert_eq!(after.bookmarks, before.bookmarks - 1);
    assert_eq!(after.captures, before.captures - 1, "captures should cascade");
    assert_eq!(after.embedded, before.embedded - 1, "vectors should cascade");
    assert_eq!(after.events, 0, "events should cascade");

    // The rest of the corpus is untouched and still searchable.
    let matches = rank_by_similarity(&store, &embedder, "document management", 3, 0.0).unwrap();
    assert!(matches.iter().all(|found| found.bookmark.id != victim.id));
}

#[test]
fn a_token_gates_write_scope_independently_of_read() {
    use torimemo_core::Scope;

    let store = Store::open_in_memory().unwrap();
    let reader = store.issue_token("reader", Scope::Read).unwrap();
    let writer = store.issue_token("writer", Scope::ReadWrite).unwrap();

    assert!(!store.authenticate(&reader.token).unwrap().unwrap().scope.may_write());
    assert!(store.authenticate(&writer.token).unwrap().unwrap().scope.may_write());

    // Revoking one leaves the other working — the property that makes rotating
    // a single credential safe.
    store.revoke_token(&reader.id).unwrap();
    assert!(store.authenticate(&reader.token).unwrap().is_none());
    assert!(store.authenticate(&writer.token).unwrap().is_some());
}
