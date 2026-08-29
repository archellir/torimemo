//! Vector recall over the stored corpus.
//!
//! Brute force, deliberately — and measured rather than assumed. `cargo bench
//! -p torimemo-embed` scans at ~3.2M vectors/second: 0.6ms at 2,000 bookmarks,
//! 3.2ms at 10,000, 17ms at 50,000, linear with no cliff. Embedding the query
//! costs ~1.0µs, so the scan is what matters.
//!
//! An approximate index was measured against this rather than assumed better.
//! libSQL's `vector_top_k` is *slower* at this corpus size (1.4ms against
//! 0.6ms — the index has overhead a small scan does not amortise), and where
//! it does win at 50,000 it returned only 2 of the correct top 10. That is the
//! trade an ANN structure makes, and for a personal archive where a query
//! should surface the thing you actually saved, an exact 17ms beats an
//! approximate 4ms. Revisit at roughly 200,000 bookmarks, where the scan
//! crosses 60ms; nothing else has to change when it does.

use crate::provider::Embedder;
use torimemo_core::{Bookmark, Result, Store};

/// A bookmark and how closely it matched.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    /// The bookmark.
    pub bookmark: Bookmark,
    /// Cosine similarity in `[-1, 1]`; higher is closer.
    pub score: f32,
}

/// The text a bookmark is embedded from.
///
/// Falls back through what is actually present: enrichment fills in title and
/// description, but a freshly captured link has only its URL, and it should
/// still be recallable before the enrichment worker reaches it. The URL's own
/// path segments carry real signal — `/paperless-ngx/paperless-ngx` says a
/// good deal on its own.
#[must_use]
pub fn embed_text(bookmark: &Bookmark) -> String {
    let mut parts = Vec::new();

    if let Some(title) = &bookmark.title {
        parts.push(title.clone());
    }
    if let Some(description) = &bookmark.description {
        parts.push(description.clone());
    }

    parts.push(bookmark.domain.clone());

    if parts.len() < 3 {
        // Only mine the path when there is little else; once a real title
        // exists, URL slugs are noise against it.
        parts.push(readable_path(&bookmark.canonical_url));
    }

    parts.join(" ")
}

/// Turns a URL's path into words a model can use.
fn readable_path(url: &str) -> String {
    let without_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let path = without_scheme.split_once('/').map_or("", |(_, rest)| rest);

    path.split(|character: char| !character.is_alphanumeric())
        .filter(|segment| segment.len() > 2 && !segment.chars().all(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A stable hash of the text a vector was computed from.
///
/// Stored beside the vector so enrichment arriving later — a title fetched
/// after the first embed — invalidates exactly the rows it affects instead of
/// forcing a full re-embed.
#[must_use]
pub fn input_hash(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

/// Ranks stored bookmarks against `query` by cosine similarity.
///
/// Returns at most `limit` matches above `floor`, closest first.
pub fn rank_by_similarity(
    store: &Store,
    embedder: &impl Embedder,
    query: &str,
    limit: usize,
    floor: f32,
) -> Result<Vec<Match>> {
    let query_embedding = embedder.embed(query)?;
    let stored = store.embeddings(embedder.model())?;

    let mut scored: Vec<(i64, f32)> = stored
        .into_iter()
        .filter_map(|(id, vector)| {
            // `dot_unit`, not `cosine`: both sides are unit vectors by
            // construction, and this runs once per bookmark per query.
            let score = dot_unit(&query_embedding.vector, &vector)?;
            (score >= floor).then_some((id, score))
        })
        .collect();

    // Descending by score. `total_cmp` rather than `partial_cmp` because a NaN
    // here would silently corrupt the ordering rather than failing loudly.
    scored.sort_by(|left, right| right.1.total_cmp(&left.1));
    scored.truncate(limit);

    let mut matches = Vec::with_capacity(scored.len());
    for (id, score) in scored {
        if let Some(bookmark) = store.bookmark(id)? {
            matches.push(Match { bookmark, score });
        }
    }
    Ok(matches)
}

/// Cosine similarity between two vectors, or `None` if their widths differ.
///
/// A width mismatch means two models' vectors are stored under one name, which
/// is a bug rather than a query-time condition — skipping the row keeps a bad
/// write from taking down every search.
///
/// General enough for vectors of any magnitude. The hot path uses [`dot_unit`]
/// instead, which is a third of the arithmetic; this stays for callers that
/// cannot promise normalized input.
#[must_use]
pub fn cosine(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }

    let mut dot = 0.0_f32;
    let mut left_magnitude = 0.0_f32;
    let mut right_magnitude = 0.0_f32;

    for (a, b) in left.iter().zip(right) {
        dot += a * b;
        left_magnitude += a * a;
        right_magnitude += b * b;
    }

    let denominator = (left_magnitude * right_magnitude).sqrt();
    if denominator <= f32::EPSILON { None } else { Some(dot / denominator) }
}

/// Accumulators the dot product splits across.
///
/// Floating-point addition is not associative, so one accumulator forces the
/// compiler to keep every add in source order: a single dependent chain with
/// one add in flight at a time. Eight independent chains let it use the whole
/// vector unit, and eight saturates NEON and AVX2 without spilling registers.
const LANES: usize = 8;

/// Dot product of two **unit** vectors, which is their cosine similarity.
///
/// Every vector this crate stores is L2-normalized at the provider before it
/// is written, so both magnitudes are 1 and the general [`cosine`] spends two
/// thirds of its arithmetic recomputing that. This is the hot path — it runs
/// once per stored bookmark on every query — so it is worth the specialisation.
#[must_use]
pub fn dot_unit(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }

    let mut sums = [0.0_f32; LANES];
    let mut left_chunks = left.chunks_exact(LANES);
    let mut right_chunks = right.chunks_exact(LANES);

    for (a, b) in left_chunks.by_ref().zip(right_chunks.by_ref()) {
        for lane in 0..LANES {
            sums[lane] = a[lane].mul_add(b[lane], sums[lane]);
        }
    }

    let mut total: f32 = sums.iter().sum();
    for (a, b) in left_chunks.remainder().iter().zip(right_chunks.remainder()) {
        total += a * b;
    }

    Some(total)
}

/// Embeds every bookmark that has no vector for this model.
///
/// Resumable by construction: the queue is defined by the absence of a row, so
/// an interrupted backfill continues where it stopped, and a new model version
/// re-queues the whole corpus without touching the old vectors.
pub fn backfill(
    store: &Store,
    embedder: &impl Embedder,
    batch_size: usize,
    mut progress: impl FnMut(usize),
) -> Result<usize> {
    let mut embedded = 0;

    loop {
        let batch = store.needing_embedding(embedder.model(), batch_size)?;
        if batch.is_empty() {
            break;
        }

        let texts: Vec<String> = batch.iter().map(embed_text).collect();
        let embeddings = embedder.embed_batch(&texts)?;

        for ((bookmark, embedding), text) in batch.iter().zip(&embeddings).zip(&texts) {
            store.set_embedding(
                bookmark.id,
                embedder.model(),
                &embedding.vector,
                &input_hash(text),
            )?;
        }

        embedded += batch.len();
        progress(embedded);
    }

    Ok(embedded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Deterministic;
    use torimemo_core::{NewCapture, Source};

    fn store_with(urls: &[(&str, Option<&str>)]) -> Store {
        let mut store = Store::open_in_memory().unwrap();
        for (url, title) in urls {
            let id = store.ingest(&NewCapture::new(*url, Source::Api)).unwrap().bookmark_id();
            if let Some(title) = title {
                store.set_metadata(id, Some(title), None).unwrap();
            }
        }
        store
    }

    #[test]
    fn cosine_of_identical_vectors_is_one() {
        let vector = [0.6, 0.8];
        assert!((cosine(&vector, &vector).unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_of_orthogonal_vectors_is_zero() {
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).unwrap().abs() < 1e-6);
    }

    #[test]
    fn dot_unit_agrees_with_cosine_on_unit_vectors() {
        // The specialisation is only valid because stored vectors are
        // normalized; this pins that equivalence.
        let mut left = vec![0.0_f32; 384];
        let mut right = vec![0.0_f32; 384];
        for index in 0..384 {
            left[index] = ((index % 7) as f32) - 3.0;
            right[index] = ((index % 11) as f32) - 5.0;
        }
        for vector in [&mut left, &mut right] {
            let magnitude = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
            for value in vector.iter_mut() {
                *value /= magnitude;
            }
        }

        let exact = cosine(&left, &right).unwrap();
        let fast = dot_unit(&left, &right).unwrap();
        assert!((exact - fast).abs() < 1e-5, "cosine {exact} vs dot_unit {fast}");
    }

    #[test]
    fn dot_unit_handles_widths_that_are_not_a_multiple_of_the_lane_count() {
        // 384 divides by 8, but another model's width might not; the
        // remainder loop has to be right.
        for width in [1_usize, 7, 8, 9, 15, 384, 385] {
            let unit = vec![1.0_f32 / (width as f32).sqrt(); width];
            let similarity = dot_unit(&unit, &unit).unwrap();
            assert!((similarity - 1.0).abs() < 1e-4, "width {width} gave {similarity}");
        }
    }

    #[test]
    fn dot_unit_rejects_mismatched_widths() {
        assert_eq!(dot_unit(&[1.0, 0.0], &[1.0]), None);
        assert_eq!(dot_unit(&[], &[]), None);
    }

    #[test]
    fn cosine_rejects_mismatched_widths() {
        assert_eq!(cosine(&[1.0, 0.0], &[1.0]), None);
        assert_eq!(cosine(&[], &[]), None);
    }

    #[test]
    fn embed_text_prefers_title_over_url_slugs() {
        let store =
            store_with(&[("https://github.com/paperless-ngx/paperless-ngx", Some("Paperless"))]);
        let bookmark = store.bookmark(1).unwrap().unwrap();
        let text = embed_text(&bookmark);
        assert!(text.contains("Paperless"));
        assert!(text.contains("github.com"));
    }

    #[test]
    fn embed_text_falls_back_to_url_words_when_untitled() {
        let store = store_with(&[("https://github.com/paperless-ngx/paperless-ngx", None)]);
        let bookmark = store.bookmark(1).unwrap().unwrap();
        assert!(embed_text(&bookmark).contains("paperless"));
    }

    #[test]
    fn backfill_embeds_everything_then_drains() {
        let store = store_with(&[("https://a.com/x", None), ("https://b.com/y", None)]);
        let embedder = Deterministic::default();

        let embedded = backfill(&store, &embedder, 10, |_| {}).unwrap();
        assert_eq!(embedded, 2);
        assert_eq!(store.stats().unwrap().embedded, 2);

        // Idempotent: a second pass has nothing to do.
        assert_eq!(backfill(&store, &embedder, 10, |_| {}).unwrap(), 0);
    }

    #[test]
    fn recall_ranks_the_lexically_closer_bookmark_first() {
        let store = store_with(&[
            ("https://a.com/x", Some("rust deterministic builds")),
            ("https://b.com/y", Some("italian pasta recipes")),
        ]);
        let embedder = Deterministic::default();
        backfill(&store, &embedder, 10, |_| {}).unwrap();

        let matches =
            rank_by_similarity(&store, &embedder, "deterministic builds", 5, 0.0).unwrap();
        assert!(!matches.is_empty());
        assert_eq!(matches[0].bookmark.canonical_url, "https://a.com/x");
    }
}
