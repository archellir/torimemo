//! Canonical URL derivation.
//!
//! A bookmark's identity is its canonical URL, so two captures of the same page
//! from different channels collapse into one bookmark with two capture events.
//! That only works if normalization is aggressive enough to see through the
//! tracking parameters real links carry: the vault's saved links include an
//! `AliExpress` URL with six of them and a Buenos Aires city link whose `fbclid`
//! is longer than the rest of the URL combined.
//!
//! Normalization is deliberately lossy in one direction only — the raw URL is
//! always retained on the capture event, so nothing here destroys information.

use crate::error::{Error, Result};
use url::{Host, Url};

/// Query parameters that identify a campaign or a referrer rather than the
/// resource. Prefix matches (`utm_`) are handled separately in [`is_tracking`].
const TRACKING_PARAMS: &[&str] = &[
    // Facebook / Meta, Google, Microsoft, Yandex, Mailchimp, HubSpot
    "fbclid",
    "gclid",
    "dclid",
    "gbraid",
    "wbraid",
    "msclkid",
    "yclid",
    "mc_cid",
    "mc_eid",
    "_hsenc",
    "_hsmi",
    "hsctatracking",
    // Twitter/X, Instagram, TikTok, Reddit, LinkedIn
    "igshid",
    "igsh",
    "twclid",
    "ttclid",
    "rdt_cid",
    "li_fat_id",
    "trk",
    "trkinfo",
    // AliExpress / Taobao and other marketplace tracking, seen throughout the
    // vault's Telegram dump.
    "spm",
    "algo_pvid",
    "algo_expid",
    "btsid",
    "ws_ab_test",
    "pvid",
    "scm",
    "aff_platform",
    "aff_trace_key",
    "aff_request_id",
    "afref",
    "af",
    "terminal_id",
    "cn",
    "cv",
    "dp",
    "sk",
    "curpageloglogid",
    "srcsns",
    "businesstype",
    // Generic referrer and session noise
    "ref",
    "ref_src",
    "ref_url",
    "referrer",
    "source",
    "share_id",
    "si",
    "feature",
    "app",
    "_branch_match_id",
    "cmpid",
    "ncid",
    "sr_share",
];

/// Hosts whose mobile or short variants address the same resource as a
/// canonical host, mapped to that canonical host.
const HOST_ALIASES: &[(&str, &str)] = &[
    ("m.youtube.com", "www.youtube.com"),
    ("youtu.be", "www.youtube.com"),
    ("mobile.twitter.com", "x.com"),
    ("twitter.com", "x.com"),
    ("www.twitter.com", "x.com"),
    ("m.facebook.com", "www.facebook.com"),
    ("old.reddit.com", "www.reddit.com"),
    ("np.reddit.com", "www.reddit.com"),
];

/// Whether `name` is a tracking parameter rather than part of the resource.
fn is_tracking(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    lowered.starts_with("utm_")
        || lowered.starts_with("_ga")
        || lowered.starts_with("pk_")
        || lowered.starts_with("piwik_")
        || TRACKING_PARAMS.contains(&lowered.as_str())
}

/// A URL reduced to the form used as a bookmark's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canonical {
    /// The normalized URL, used for dedupe and as the bookmark's identity.
    pub url: String,
    /// The registrable host, lowercased and stripped of a leading `www.`.
    pub domain: String,
}

/// Derives the canonical form of `raw`.
///
/// Returns an error for input that does not parse as an absolute `http(s)`
/// URL. Callers importing bulk dumps should treat that as "skip this line"
/// rather than as a failure of the import.
pub fn canonicalize(raw: &str) -> Result<Canonical> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Error::msg("empty URL"));
    }

    let mut url = Url::parse(trimmed)?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::msg(format!("unsupported scheme: {}", url.scheme())));
    }

    // `http` and `https` for the same host and path are the same resource in
    // every case that matters here, and the vault's older dumps are full of
    // `http` links to sites that have since moved. Collapsing to `https` keeps
    // those from becoming a second bookmark.
    url.set_scheme("https").map_err(|()| Error::msg("could not normalize scheme to https"))?;

    // A default port carries no information once the scheme is fixed.
    let _ = url.set_port(None);

    // Credentials in a bookmark are either accidental or a secret that should
    // not be persisted. Either way they are not part of the resource's identity.
    let _ = url.set_username("");
    let _ = url.set_password(None);

    normalize_host(&mut url)?;

    // A fragment usually addresses a position within the resource, not a
    // different resource. The exception is a hash route (`#/path`), which is
    // the whole address for some single-page apps, so those are kept.
    let keep_fragment = url.fragment().is_some_and(|fragment| fragment.starts_with('/'));
    if !keep_fragment {
        url.set_fragment(None);
    }

    strip_tracking_params(&mut url);
    normalize_path(&mut url);

    let domain = match url.host() {
        Some(Host::Domain(domain)) => domain.trim_start_matches("www.").to_string(),
        Some(host) => host.to_string(),
        None => return Err(Error::msg("URL has no host")),
    };

    Ok(Canonical { url: url.to_string(), domain })
}

/// Lowercases the host and applies the alias table.
fn normalize_host(url: &mut Url) -> Result<()> {
    let Some(Host::Domain(host)) = url.host() else {
        // An IP literal is already canonical, and the vault has a few
        // (`192.168.0.106`, `127.0.0.1`) from local development.
        return Ok(());
    };

    let lowered = host.to_ascii_lowercase();
    let canonical = HOST_ALIASES
        .iter()
        .find_map(|(alias, target)| (*alias == lowered).then_some(*target))
        .unwrap_or(&lowered);

    // `youtu.be/ID` addresses the same video as `youtube.com/watch?v=ID`, so
    // the path has to move into a query parameter for the two to collapse.
    if lowered == "youtu.be" {
        let video_id = url.path().trim_start_matches('/').to_string();
        if !video_id.is_empty() {
            url.set_path("/watch");
            let existing: Vec<(String, String)> = url
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .filter(|(key, _)| key != "v")
                .collect();
            url.query_pairs_mut().clear().append_pair("v", &video_id).extend_pairs(existing);
        }
    }

    url.set_host(Some(canonical))
        .map_err(|error| Error::with_source("could not normalize host", error))
}

/// Removes tracking parameters and sorts what remains.
///
/// Sorting matters because the same link shared twice can arrive with its
/// parameters in different orders; without it those stay two bookmarks.
fn strip_tracking_params(url: &mut Url) {
    if url.query().is_none() {
        return;
    }

    let mut kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !is_tracking(key))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();

    if kept.is_empty() {
        url.set_query(None);
        return;
    }

    kept.sort();
    url.query_pairs_mut().clear().extend_pairs(kept);
}

/// Drops a trailing slash and unwraps AMP paths.
fn normalize_path(url: &mut Url) {
    let path = url.path().to_string();

    // Google's AMP viewer prefixes the real URL; the vault has a few of these
    // from mobile shares. The suffix form (`/article/amp`) is left alone —
    // stripping it guesses at a redirect that may not exist.
    if let Some(rest) = path.strip_prefix("/amp/s/")
        && let Ok(unwrapped) = Url::parse(&format!("https://{rest}"))
    {
        *url = unwrapped;
        return;
    }

    if path.len() > 1 && path.ends_with('/') {
        url.set_path(path.trim_end_matches('/'));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(raw: &str) -> String {
        canonicalize(raw).expect("should canonicalize").url
    }

    #[test]
    fn strips_utm_and_click_ids() {
        assert_eq!(
            canonical("https://example.com/a?utm_source=x&utm_medium=y&fbclid=abc"),
            "https://example.com/a"
        );
    }

    #[test]
    fn strips_marketplace_tracking_from_a_real_vault_link() {
        // Shortened from the AliExpress link in `Saved telegram.md`.
        let raw = "https://aliexpress.ru/item/4000344862592.html?spm=a2g0o.productlist.0.0.3ecb2c6\
                   &algo_pvid=6099bb1e&algo_expid=6099bb1e-0&btsid=0b8b036d&ws_ab_test=searchweb0_0";
        assert_eq!(canonical(raw), "https://aliexpress.ru/item/4000344862592.html");
    }

    #[test]
    fn strips_affiliate_referral_params() {
        // Shortened from an AliExpress affiliate link in `Saved telegram.md`;
        // the referral chain is longer than the product path it points at.
        let raw = "https://aliexpress.ru/item/4001081577255.html\
                   ?af=4014363&aff_request_id=e608eb6f&afref=https%3A%2F%2Fwww.youtube.com\
                   &cn=43qef1&cv=39083361&dp=v5_43qef1&sk=_dYAaMKG";
        assert_eq!(canonical(raw), "https://aliexpress.ru/item/4001081577255.html");
    }

    #[test]
    fn keeps_meaningful_query_params() {
        assert_eq!(
            canonical("https://www.youtube.com/watch?v=ngdoUQBvAjo&utm_source=x"),
            "https://www.youtube.com/watch?v=ngdoUQBvAjo"
        );
    }

    #[test]
    fn sorts_params_so_order_does_not_fork_identity() {
        assert_eq!(canonical("https://e.com/s?b=2&a=1"), canonical("https://e.com/s?a=1&b=2"));
    }

    #[test]
    fn collapses_http_and_https() {
        assert_eq!(canonical("http://example.com/a"), canonical("https://example.com/a"));
    }

    #[test]
    fn collapses_youtu_be_into_watch_url() {
        assert_eq!(
            canonical("https://youtu.be/ngdoUQBvAjo"),
            canonical("https://www.youtube.com/watch?v=ngdoUQBvAjo")
        );
    }

    #[test]
    fn collapses_twitter_and_x() {
        assert_eq!(canonical("https://twitter.com/user/status/1"), "https://x.com/user/status/1");
    }

    #[test]
    fn drops_trailing_slash_but_keeps_root() {
        assert_eq!(canonical("https://example.com/a/"), "https://example.com/a");
        assert_eq!(canonical("https://example.com/"), "https://example.com/");
    }

    #[test]
    fn drops_fragment_but_keeps_hash_route() {
        assert_eq!(canonical("https://example.com/a#section"), "https://example.com/a");
        assert_eq!(canonical("https://example.com/a#/route"), "https://example.com/a#/route");
    }

    #[test]
    fn extracts_domain_without_www() {
        assert_eq!(canonicalize("https://www.github.com/x").unwrap().domain, "github.com");
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert!(canonicalize("mailto:a@b.com").is_err());
        assert!(canonicalize("").is_err());
    }
}
