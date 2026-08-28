//! Property tests for URL canonicalization.
//!
//! The example tests beside `normalize.rs` cover the cases I thought of. These
//! cover the ones I did not: canonicalization is the identity function for the
//! whole store — every dedupe decision, every bookmark's primary key, and the
//! correctness of the capture-count ranking signal rest on it — so the
//! invariants are worth stating in a form that generates its own
//! counterexamples.

use proptest::prelude::*;
use torimemo_core::canonicalize;

/// Hosts that survive canonicalization unchanged, so a generated URL's
/// identity is predictable. Deliberately excludes the alias table's inputs —
/// those are tested for the collapse they are supposed to perform.
fn host() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "example.com".to_string(),
        "a.example.org".to_string(),
        "sub.domain.co.uk".to_string(),
        "localhost".to_string(),
    ])
}

/// Path segments made of characters that need no percent-encoding, so a
/// round-trip through the URL parser is lossless by construction.
fn path() -> impl Strategy<Value = String> {
    prop::collection::vec("[a-z0-9]{1,8}", 0..4).prop_map(|segments| {
        if segments.is_empty() { String::new() } else { format!("/{}", segments.join("/")) }
    })
}

/// A tracking parameter the normalizer is supposed to strip.
fn tracking_param() -> impl Strategy<Value = (String, String)> {
    (
        prop::sample::select(vec![
            "utm_source".to_string(),
            "utm_medium".to_string(),
            "utm_campaign".to_string(),
            "fbclid".to_string(),
            "gclid".to_string(),
            "igshid".to_string(),
            "spm".to_string(),
            "algo_pvid".to_string(),
        ]),
        "[a-zA-Z0-9]{1,12}".prop_map(String::from),
    )
}

/// A parameter that identifies the resource and must survive.
fn meaningful_param() -> impl Strategy<Value = (String, String)> {
    (
        prop::sample::select(vec![
            "id".to_string(),
            "page".to_string(),
            "q".to_string(),
            "v".to_string(),
        ]),
        "[a-zA-Z0-9]{1,12}".prop_map(String::from),
    )
}

fn query_string(pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return String::new();
    }
    let joined: Vec<String> = pairs.iter().map(|(key, value)| format!("{key}={value}")).collect();
    format!("?{}", joined.join("&"))
}

proptest! {
    /// Canonicalizing twice must equal canonicalizing once.
    ///
    /// The store's identity column holds an already-canonical URL, and
    /// `bookmark_by_url` canonicalizes its argument before looking it up. If
    /// the function were not idempotent, a bookmark could become unreachable
    /// by the very URL it was stored under.
    #[test]
    fn canonicalization_is_idempotent(
        host in host(),
        path in path(),
        tracking in prop::collection::vec(tracking_param(), 0..4),
        meaningful in prop::collection::vec(meaningful_param(), 0..3),
    ) {
        let mut pairs = tracking;
        pairs.extend(meaningful);
        let raw = format!("https://{host}{path}{}", query_string(&pairs));

        if let Ok(once) = canonicalize(&raw) {
            let twice = canonicalize(&once.url).expect("a canonical URL must re-canonicalize");
            prop_assert_eq!(once.url, twice.url);
        }
    }

    /// Tracking parameters never survive, however many there are and whatever
    /// they are mixed with.
    #[test]
    fn tracking_parameters_never_survive(
        host in host(),
        path in path(),
        tracking in prop::collection::vec(tracking_param(), 1..5),
    ) {
        let raw = format!("https://{host}{path}{}", query_string(&tracking));

        if let Ok(canonical) = canonicalize(&raw) {
            for (key, _) in &tracking {
                prop_assert!(
                    !canonical.url.contains(key.as_str()),
                    "{} survived in {}", key, canonical.url
                );
            }
        }
    }

    /// Parameters that identify the resource always survive.
    ///
    /// The dangerous failure mode is a normalizer that strips too much:
    /// dropping `?v=` would collapse every YouTube video into one bookmark,
    /// which is silent, irreversible data loss.
    #[test]
    fn meaningful_parameters_always_survive(
        host in host(),
        path in path(),
        meaningful in prop::collection::vec(meaningful_param(), 1..4),
    ) {
        let raw = format!("https://{host}{path}{}", query_string(&meaningful));

        if let Ok(canonical) = canonicalize(&raw) {
            for (key, value) in &meaningful {
                prop_assert!(
                    canonical.url.contains(key.as_str()),
                    "{} was dropped from {}", key, canonical.url
                );
                prop_assert!(
                    canonical.url.contains(value.as_str()),
                    "value of {} was dropped from {}", key, canonical.url
                );
            }
        }
    }

    /// Parameter order never changes a URL's identity.
    ///
    /// The same link shared twice can arrive with its parameters in either
    /// order; without this the two would be separate bookmarks.
    #[test]
    fn parameter_order_does_not_change_identity(
        host in host(),
        path in path(),
        mut pairs in prop::collection::vec(meaningful_param(), 2..4),
    ) {
        // Distinct keys only: `?a=1&a=2` is genuinely order-dependent, and
        // sorting it is not expected to be a no-op.
        pairs.sort();
        pairs.dedup_by(|left, right| left.0 == right.0);
        prop_assume!(pairs.len() >= 2);

        let forward = format!("https://{host}{path}{}", query_string(&pairs));
        let mut reversed = pairs.clone();
        reversed.reverse();
        let backward = format!("https://{host}{path}{}", query_string(&reversed));

        if let (Ok(first), Ok(second)) = (canonicalize(&forward), canonicalize(&backward)) {
            prop_assert_eq!(first.url, second.url);
        }
    }

    /// The scheme is always `https`, so `http` and `https` for one resource
    /// cannot become two bookmarks.
    #[test]
    fn every_canonical_url_is_https(
        scheme in prop::sample::select(vec!["http", "https"]),
        host in host(),
        path in path(),
    ) {
        let raw = format!("{scheme}://{host}{path}");
        if let Ok(canonical) = canonicalize(&raw) {
            prop_assert!(canonical.url.starts_with("https://"), "got {}", canonical.url);
        }
    }

    /// The reported domain is always a suffix-consistent part of the URL.
    ///
    /// Domain drives grouping and the top-domains report; a domain that does
    /// not belong to its own URL would mis-file a bookmark silently.
    #[test]
    fn the_domain_belongs_to_its_url(host in host(), path in path()) {
        let raw = format!("https://{host}{path}");
        if let Ok(canonical) = canonicalize(&raw) {
            prop_assert!(
                canonical.url.contains(&canonical.domain),
                "domain {} is not in {}", canonical.domain, canonical.url
            );
            prop_assert_eq!(canonical.domain.to_lowercase(), canonical.domain);
        }
    }

    /// Canonicalization never panics, whatever it is handed.
    ///
    /// Bulk imports feed it scraped markdown and agents feed it whatever a
    /// user typed. An error is a fine answer; a panic would take down the
    /// import or the request.
    #[test]
    fn arbitrary_input_never_panics(raw in ".*") {
        let _ = canonicalize(&raw);
    }

    /// Neither does anything shaped like a URL.
    #[test]
    fn url_shaped_input_never_panics(
        scheme in "[a-z]{1,10}",
        rest in ".*",
    ) {
        let _ = canonicalize(&format!("{scheme}://{rest}"));
    }
}
