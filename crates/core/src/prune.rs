//! Rules for deciding which bookmarks are no longer worth keeping.
//!
//! A decade of saving produces a lot that was never worth keeping and a lot
//! more that stopped being worth keeping: pages that 404, login walls that
//! reveal nothing, job postings for roles long filled, marketplace listings
//! for things long sold. This module names those categories so a cull is a
//! reviewable list of reasons rather than a judgement call per row.
//!
//! Two properties make that safe. Every rule states **why** it matched, so a
//! delete list can be read and argued with. And no rule is applied
//! automatically — the caller chooses which categories to enable, because the
//! boundary between "expired" and "still interesting" is the user's to draw,
//! not this module's.
//!
//! The rules were derived from a real 2,101-bookmark archive, where they
//! identified roughly three quarters as unreachable, unreadable, or spent.

use crate::model::Bookmark;
use serde::Serialize;

/// Why a bookmark was proposed for removal.
///
/// Ordered from most objective to most subjective. Everything above
/// [`Self::Ephemeral`] is a measurable fact about the link; everything below
/// is a judgement about the content, and a caller should think harder before
/// enabling those.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The server said it is gone: a 404, a 410, or a host that no longer
    /// resolves. Not a guess.
    Dead,
    /// Fetched successfully but carried no metadata — a login wall. The URL
    /// is unreadable and unsearchable, so it occupies space without being
    /// recoverable.
    Walled,
    /// No title from any source. All that remains is a URL with no record of
    /// what it pointed at.
    Untitled,
    /// A file rather than a page: an image, a PDF, an installer.
    Asset,
    /// A login form, a personal dashboard, or a host on a private network.
    /// Bookmarking one saves the door, not the room.
    Session,
    /// A shortener that no longer resolves. `goo.gl` is shut down; the rest
    /// are opaque redirects to nothing.
    DeadShortlink,
    /// A job posting. Roles are filled and postings are taken down; this is
    /// the most reliably expired category on the open web.
    JobPosting,
    /// A marketplace listing. Sold or delisted.
    ShoppingListing,
    /// A link that only meant something in a session — a dating redirect, a
    /// match page, a one-off tool result.
    Ephemeral,
    /// A title that describes the site rather than the page: "Home",
    /// "Features", "Welcome". Search cannot use it and neither can you.
    Contentless,
    /// Two bookmarks with the same title; the older one is kept.
    Duplicate,
}

impl Reason {
    /// The stable name used on the command line and in output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dead => "dead",
            Self::Walled => "walled",
            Self::Untitled => "untitled",
            Self::Asset => "asset",
            Self::Session => "session",
            Self::DeadShortlink => "dead-shortlink",
            Self::JobPosting => "job-posting",
            Self::ShoppingListing => "shopping-listing",
            Self::Ephemeral => "ephemeral",
            Self::Contentless => "contentless",
            Self::Duplicate => "duplicate",
        }
    }

    /// A sentence explaining the category, for the dry run's summary.
    #[must_use]
    pub fn explanation(self) -> &'static str {
        match self {
            Self::Dead => "the server returned 404/410 or the host no longer resolves",
            Self::Walled => "fetched, but the page was a login wall with no metadata",
            Self::Untitled => "no title from any source; only a bare URL remains",
            Self::Asset => "a file rather than a page (image, PDF, installer)",
            Self::Session => "a login form, personal dashboard, or private-network host",
            Self::DeadShortlink => "a URL shortener that no longer resolves",
            Self::JobPosting => "a job posting; the role is long since filled",
            Self::ShoppingListing => "a marketplace listing; sold or delisted",
            Self::Ephemeral => "only meaningful during the session that produced it",
            Self::Contentless => "the title names the site, not the page",
            Self::Duplicate => "another bookmark has the same title; the older is kept",
        }
    }

    /// Every category, in order.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Dead,
            Self::Walled,
            Self::Untitled,
            Self::Asset,
            Self::Session,
            Self::DeadShortlink,
            Self::JobPosting,
            Self::ShoppingListing,
            Self::Ephemeral,
            Self::Contentless,
            Self::Duplicate,
        ]
    }

    /// Parses a category name.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::all().iter().copied().find(|reason| reason.as_str() == value)
    }

    /// Whether this rule rests on a measurable fact rather than a judgement
    /// about the content.
    ///
    /// The distinction matters for defaults: an unreachable link is gone
    /// whatever anyone thinks, while "this job posting is stale" is an
    /// inference that is usually but not always right.
    #[must_use]
    pub fn is_objective(self) -> bool {
        matches!(self, Self::Dead | Self::Walled | Self::Untitled | Self::Asset | Self::Session)
    }
}

/// One bookmark proposed for removal, with the reason.
#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    /// Which bookmark.
    pub id: i64,
    /// Its canonical URL, so the list can be read without a second lookup.
    pub url: String,
    /// Its title, if it had one.
    pub title: Option<String>,
    /// Why it matched.
    pub reason: Reason,
}

/// Hosts whose unauthenticated pages carry no usable metadata.
const WALLED_HOSTS: &[&str] = &["instagram.com", "threads.com", "threads.net"];

/// Shorteners that are dead or opaque. `goo.gl` shut down in 2025; the others
/// were measured as unresolvable in this corpus.
const DEAD_SHORTENERS: &[&str] =
    &["goo.gl", "gooo.im", "cutt.ly", "amp.gs", "search.app", "clck.ru", "bit.ly"];

/// Substrings that identify a job board or an applicant-tracking host.
const JOB_HOSTS: &[&str] = &[
    "lever.co",
    "greenhouse.io",
    "recruitee.com",
    "workable.com",
    "hh.ru",
    "connecticum.de",
    "toughbyte.com",
    "freelancer.com",
    "upwork.com",
    "underdog.io",
];

/// Substrings that identify a marketplace.
const SHOPPING_HOSTS: &[&str] = &[
    "mercadolibre",
    "aliexpress",
    "market.yandex",
    "amazon.",
    "wildberries",
    "ozon.",
    "zonaprop",
    "avito.",
    "dns-shop",
    "citilink",
];

/// Hosts whose links only mean something inside a session.
const EPHEMERAL_HOSTS: &[&str] = &["tinder", "op.gg", "badoo", "buffstream", "flixhq"];

/// File extensions that mean the link is an asset, not a page.
const ASSET_EXTENSIONS: &[&str] =
    &[".jpg", ".jpeg", ".png", ".gif", ".webp", ".pdf", ".apk", ".zip", ".mp4", ".webm"];

/// URL fragments that mark a login form, a dashboard, or a private host.
const SESSION_MARKERS: &[&str] = &[
    "/login",
    "signin",
    "sign_in",
    "/auth/",
    "backoffice",
    "dashboard",
    "localhost",
    "127.0.0.1",
    "192.168.",
    "__cf_chl",
];

/// Titles that name the site rather than the page.
///
/// Matched against the whole lowercased title, never as a substring: a page
/// genuinely titled "Features of Rust's Borrow Checker" must survive.
const CONTENTLESS_TITLES: &[&str] = &[
    "home",
    "homepage",
    "features",
    "welcome",
    "pricing",
    "about",
    "docs",
    "documentation",
    "overview",
    "dashboard",
    "blog",
    "contact",
    "client challenge",
    "browse all training - training",
    "choose a starting point",
    "build something amazing.",
];

/// Classifies one bookmark, given what enrichment found.
///
/// `fetch_status` is the recorded outcome, absent when the bookmark has never
/// been fetched. Returns the **first** matching reason, most objective first,
/// so a dead job posting is reported as dead — the stronger fact.
#[must_use]
pub fn classify(bookmark: &Bookmark, fetch_status: Option<&str>) -> Option<Reason> {
    let url = bookmark.canonical_url.to_lowercase();
    let domain = bookmark.domain.as_str();

    if fetch_status == Some("dead") {
        return Some(Reason::Dead);
    }

    if fetch_status == Some("no_metadata") && WALLED_HOSTS.contains(&domain) {
        return Some(Reason::Walled);
    }

    if ASSET_EXTENSIONS.iter().any(|extension| url.ends_with(extension)) {
        return Some(Reason::Asset);
    }

    if SESSION_MARKERS.iter().any(|marker| url.contains(marker)) {
        return Some(Reason::Session);
    }

    if bookmark.title.is_none() {
        return Some(Reason::Untitled);
    }

    if DEAD_SHORTENERS.contains(&domain) || domain.starts_with("maps.app") {
        return Some(Reason::DeadShortlink);
    }

    if JOB_HOSTS.iter().any(|host| domain.contains(host)) {
        return Some(Reason::JobPosting);
    }

    if SHOPPING_HOSTS.iter().any(|host| domain.contains(host)) {
        return Some(Reason::ShoppingListing);
    }

    if EPHEMERAL_HOSTS.iter().any(|host| domain.contains(host)) {
        return Some(Reason::Ephemeral);
    }

    if let Some(title) = &bookmark.title {
        let normalized = title.trim().to_lowercase();
        if CONTENTLESS_TITLES.contains(&normalized.as_str()) {
            return Some(Reason::Contentless);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn bookmark(url: &str, title: Option<&str>) -> Bookmark {
        let now = Utc::now();
        let domain = url
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or("")
            .trim_start_matches("www.")
            .to_string();
        Bookmark {
            id: 1,
            canonical_url: url.to_string(),
            domain,
            title: title.map(str::to_string),
            description: None,
            first_captured_at: now,
            last_captured_at: now,
            capture_count: 1,
        }
    }

    #[test]
    fn a_dead_fetch_is_reported_as_dead() {
        let mark = bookmark("https://example.com/a", Some("Something"));
        assert_eq!(classify(&mark, Some("dead")), Some(Reason::Dead));
    }

    #[test]
    fn a_walled_host_with_no_metadata_is_walled() {
        let mark = bookmark("https://instagram.com/p/abc", None);
        assert_eq!(classify(&mark, Some("no_metadata")), Some(Reason::Walled));
    }

    #[test]
    fn a_non_walled_host_with_no_metadata_is_merely_untitled() {
        // The distinction matters: a login wall is unrecoverable, while an
        // ordinary page that failed to yield a title might be refetched.
        let mark = bookmark("https://example.com/a", None);
        assert_eq!(classify(&mark, Some("no_metadata")), Some(Reason::Untitled));
    }

    #[test]
    fn assets_are_recognised_by_extension() {
        for url in ["https://a.com/x.jpg", "https://a.com/x.pdf", "https://a.com/x.apk"] {
            assert_eq!(classify(&bookmark(url, Some("t")), None), Some(Reason::Asset));
        }
    }

    #[test]
    fn session_urls_are_recognised() {
        for url in [
            "https://a.com/login",
            "https://dashboard.heroku.com/apps",
            "https://localhost:8080/x",
            "https://192.168.0.1/admin",
        ] {
            assert_eq!(classify(&bookmark(url, Some("t")), None), Some(Reason::Session));
        }
    }

    #[test]
    fn a_dead_link_outranks_its_other_categories() {
        // A dead job posting is reported as dead: the stronger, measured fact
        // rather than the inference.
        let mark = bookmark("https://jobs.lever.co/x", Some("Engineer"));
        assert_eq!(classify(&mark, Some("dead")), Some(Reason::Dead));
    }

    #[test]
    fn job_and_shopping_hosts_are_recognised() {
        assert_eq!(
            classify(&bookmark("https://jobs.lever.co/acme/1", Some("Engineer")), None),
            Some(Reason::JobPosting)
        );
        assert_eq!(
            classify(&bookmark("https://aliexpress.ru/item/1.html", Some("Thing")), None),
            Some(Reason::ShoppingListing)
        );
    }

    #[test]
    fn a_contentless_title_is_matched_whole_not_as_a_substring() {
        assert_eq!(
            classify(&bookmark("https://a.com/x", Some("Features")), None),
            Some(Reason::Contentless)
        );
        // The substring trap: this must survive.
        assert_eq!(
            classify(&bookmark("https://a.com/x", Some("Features of Rust's borrow checker")), None),
            None
        );
    }

    #[test]
    fn a_real_bookmark_matches_nothing() {
        let mark = bookmark("https://caddyserver.com/", Some("Caddy - The Ultimate Server"));
        assert_eq!(classify(&mark, Some("enriched")), None);
    }

    #[test]
    fn short_tool_names_are_not_treated_as_contentless() {
        // An earlier draft used a title-length test and would have deleted
        // these; they are real tools whose names are simply short.
        for title in ["Carbon", "Lobsters", "ASCIIFlow", "Markwhen", "OpenMoji"] {
            assert_eq!(
                classify(&bookmark("https://example.com/", Some(title)), Some("enriched")),
                None,
                "{title} should have been kept"
            );
        }
    }

    #[test]
    fn every_reason_round_trips_through_its_name() {
        for reason in Reason::all() {
            assert_eq!(Reason::parse(reason.as_str()), Some(*reason));
        }
        assert_eq!(Reason::parse("nonsense"), None);
    }

    #[test]
    fn objective_reasons_are_the_measurable_ones() {
        assert!(Reason::Dead.is_objective());
        assert!(Reason::Walled.is_objective());
        // An inference, however reliable.
        assert!(!Reason::JobPosting.is_objective());
        assert!(!Reason::Contentless.is_objective());
    }
}
