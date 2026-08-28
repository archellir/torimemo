//! How brute-force recall scales.
//!
//! `recall.rs` claims a full scan is the right choice "at a few thousand"
//! bookmarks and names itself as the module to revisit two orders of magnitude
//! later. That was an assertion in a comment. This measures it, so the
//! decision to add an approximate index is made against a number rather than a
//! feeling — and so a change that quietly makes search superlinear is visible.
//!
//! Vectors are synthetic and the provider is deterministic: the point is the
//! shape of the curve, not the quality of the results.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use torimemo_core::{NewCapture, Source, Store};
use torimemo_embed::{Embedder, Provider, backfill, rank_by_similarity};

/// Corpus sizes spanning the current archive and two orders of magnitude past
/// it — the range over which the "brute force is fine" claim has to hold.
const SIZES: &[usize] = &[100, 1_000, 2_000, 10_000, 50_000];

/// Builds a store with `count` embedded bookmarks.
fn corpus(count: usize, embedder: &Provider) -> Store {
    let mut store = Store::open_in_memory().expect("in-memory store");

    for index in 0..count {
        // Distinct hosts and paths so canonicalization cannot collapse them
        // into one bookmark, which would silently shrink the corpus.
        let url = format!("https://host{}.example.com/page/{index}", index % 997);
        let id = store.ingest(&NewCapture::new(url, Source::Api)).expect("ingest").bookmark_id();
        store
            .set_metadata(
                id,
                Some(&format!("document number {index} about topic {}", index % 40)),
                None,
            )
            .expect("metadata");
    }

    backfill(&store, embedder, 512, |_| {}).expect("backfill");
    store
}

fn recall(criterion: &mut Criterion) {
    // The deterministic provider: a benchmark must not download a model or
    // measure ONNX inference, which would swamp the scan being measured.
    let embedder = Provider::deterministic();

    let mut group = criterion.benchmark_group("recall");
    // Cosine over the whole corpus is the unit of work, so throughput is
    // reported per vector scanned rather than per query.
    for &size in SIZES {
        let store = corpus(size, &embedder);
        group.throughput(criterion::Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |bencher, _| {
            bencher.iter(|| {
                let matches = rank_by_similarity(
                    black_box(&store),
                    black_box(&embedder),
                    black_box("a document about a topic"),
                    10,
                    0.0,
                )
                .expect("recall");
                black_box(matches.len())
            });
        });
    }
    group.finish();
}

/// Embedding one text, for comparison.
///
/// Worth measuring beside recall because it sets the floor: a query cannot be
/// faster than embedding the query string, and if that dominates then
/// optimizing the scan is wasted effort.
fn embed_query(criterion: &mut Criterion) {
    let embedder = Provider::deterministic();
    criterion.bench_function("embed_query", |bencher| {
        bencher.iter(|| {
            black_box(embedder.embed(black_box("a document about a topic")).expect("embed"))
        });
    });
}

criterion_group!(benches, recall, embed_query);
criterion_main!(benches);
