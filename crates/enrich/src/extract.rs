//! Turning a fetched HTML page into a title and a description.
//!
//! The hard part is not parsing — it is knowing when a page said nothing. A
//! quarter of this corpus is behind a login wall, and Instagram, X, Threads,
//! and `LinkedIn` all answer an unauthenticated request with a valid page whose
//! title is just the product's name. Storing "Instagram" as the title of 268
//! bookmarks would be worse than storing nothing: recall would cluster them
//! all together and the URL slugs that currently carry real signal would be
//! displaced by a word that distinguishes nothing.
//!
//! So extraction can return "there was no metadata here", and that is a
//! first-class outcome rather than an error.

use scraper::{Html, Selector};

/// What a page yielded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// The page title, if it carried a real one.
    pub title: Option<String>,
    /// The page description, if any.
    pub description: Option<String>,
}

impl Metadata {
    /// Whether anything usable was found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.description.is_none()
    }
}

/// Site names that these hosts return in place of a real title when the
/// request is not authenticated.
///
/// Matched against the whole trimmed title, never as a substring: a genuine
/// article legitimately titled "Instagram is changing" must survive. Entries
/// carry no trailing period — [`is_placeholder`] strips those before matching,
/// so "Just a moment..." and "Just a moment" are one entry.
const PLACEHOLDER_TITLES: &[&str] = &[
    "instagram",
    "x",
    "twitter",
    "threads",
    "linkedin",
    "facebook",
    "log in",
    "login",
    "sign in",
    "just a moment",
    "attention required! | cloudflare",
    "access denied",
    "are you a robot?",
    "page not found",
    "404 not found",
    "403 forbidden",
    "error",
    "redirecting",
    "loading",
    // Generic landing pages a site serves when the real resource needs auth or
    // no longer exists. Each of these appeared many times over in this corpus
    // (Reddit 21x, Tinder 15x) against completely different URLs, which is the
    // tell: a title that describes the site rather than the page.
    "reddit - the heart of the internet",
    "reddit - prove your humanity",
    "google maps",
    "youtube",
    "vk",
    "tinder | dating, make friends & meet new people",
    "mercado libre",
];

/// Whether `title` is a site's placeholder rather than a description of the page.
fn is_placeholder(title: &str) -> bool {
    // A dash is not in the split set below — real titles use it as
    // punctuation — but a *leading* one means the page-specific half was
    // empty, leaving only the site suffix behind.
    let normalized = title
        .trim()
        .trim_start_matches(['-', '|', '·', '—'])
        .trim()
        .trim_end_matches('.')
        .to_lowercase();

    if normalized.is_empty() {
        return true;
    }
    if PLACEHOLDER_TITLES.contains(&normalized.as_str()) {
        return true;
    }

    // A title whose parts are all the same brand name — "X on X", "VK | VK" —
    // says only which site it came from. Splitting on separators and checking
    // for a single distinct part catches every spelling of that.
    let parts: Vec<&str> = normalized
        .split([':', '·', '|', '—', '»'])
        .flat_map(|part| part.split(" on "))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    if parts.len() > 1 && parts.iter().all(|part| *part == parts[0]) {
        return true;
    }

    // "- YouTube" and "| Reddit" are a stripped title with only the site
    // suffix left; there was never any page-specific text.
    if parts.len() == 1 && PLACEHOLDER_TITLES.contains(&parts[0]) {
        return true;
    }

    // "Instagram · Something" is the same wall with a separator; a real page
    // title has content before the separator too.
    let leading = normalized.split([':', '·', '|', '-', '—']).next().unwrap_or(&normalized).trim();
    PLACEHOLDER_TITLES.contains(&leading) && leading.len() == normalized.trim_end().len()
}

/// Collapses whitespace and trims, returning `None` for empty results.
fn clean(raw: &str) -> Option<String> {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

/// Reads the first non-empty `content` attribute among `selectors`.
fn meta_content(document: &Html, selectors: &[&str]) -> Option<String> {
    for raw in selectors {
        let Ok(selector) = Selector::parse(raw) else { continue };
        if let Some(cleaned) = document
            .select(&selector)
            .next()
            .and_then(|element| element.attr("content"))
            .and_then(clean)
        {
            return Some(cleaned);
        }
    }
    None
}

/// Extracts a title and description from a page.
///
/// Prefers `OpenGraph` over the `<title>` element: it is what the page's author
/// wrote for a link preview, which is exactly this use case, and it is usually
/// free of the " | Site Name" suffix.
#[must_use]
pub fn metadata(html: &str) -> Metadata {
    let document = Html::parse_document(html);

    let title =
        meta_content(&document, &[r#"meta[property="og:title"]"#, r#"meta[name="twitter:title"]"#])
            .or_else(|| {
                Selector::parse("title")
                    .ok()
                    .and_then(|selector| document.select(&selector).next())
                    .and_then(|element| clean(&element.text().collect::<String>()))
            })
            .filter(|title| !is_placeholder(title));

    let description = meta_content(
        &document,
        &[
            r#"meta[property="og:description"]"#,
            r#"meta[name="description"]"#,
            r#"meta[name="twitter:description"]"#,
        ],
    )
    // A description that merely repeats the title adds nothing to the embedded
    // text and costs a duplicate phrase in the vector.
    .filter(|description| Some(description.as_str()) != title.as_deref());

    Metadata { title, description }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_opengraph_over_title_element() {
        let html = r#"<html><head>
            <title>Repo · GitHub</title>
            <meta property="og:title" content="paperless-ngx: document management">
        </head></html>"#;
        assert_eq!(metadata(html).title.as_deref(), Some("paperless-ngx: document management"));
    }

    #[test]
    fn falls_back_to_the_title_element() {
        let html = "<html><head><title>Deterministic builds in Rust</title></head></html>";
        assert_eq!(metadata(html).title.as_deref(), Some("Deterministic builds in Rust"));
    }

    #[test]
    fn rejects_the_instagram_login_wall() {
        // Exactly what instagram.com returns to an unauthenticated fetch.
        let html = "<html><head><title>Instagram</title></head></html>";
        assert_eq!(metadata(html).title, None);
    }

    #[test]
    fn rejects_other_walls_and_error_pages() {
        for title in ["X", "Threads", "Log in", "Just a moment...", "404 Not Found"] {
            let html = format!("<html><head><title>{title}</title></head></html>");
            assert_eq!(metadata(&html).title, None, "should have rejected {title}");
        }
    }

    #[test]
    fn rejects_a_title_that_is_only_a_repeated_brand_name() {
        // Both appeared in the real corpus against many distinct URLs.
        for title in ["X on X", "VK | VK", "- YouTube", "Reddit - The heart of the internet"] {
            let html = format!("<html><head><title>{title}</title></head></html>");
            assert_eq!(metadata(&html).title, None, "should have rejected {title}");
        }
    }

    #[test]
    fn keeps_a_handle_style_title_that_names_a_real_account() {
        // "@levelsio (@levelsio) on X" repeats a handle, not the site name,
        // and still says who the post is by.
        let html = "<html><head><title>@levelsio (@levelsio) on X</title></head></html>";
        assert!(metadata(html).title.is_some());
    }

    #[test]
    fn keeps_a_real_title_that_merely_starts_with_a_brand_name() {
        let html = "<html><head><title>Instagram is changing its feed</title></head></html>";
        assert_eq!(metadata(html).title.as_deref(), Some("Instagram is changing its feed"));
    }

    #[test]
    fn collapses_whitespace() {
        let html = "<html><head><title>  spaced\n   out  </title></head></html>";
        assert_eq!(metadata(html).title.as_deref(), Some("spaced out"));
    }

    #[test]
    fn drops_a_description_that_repeats_the_title() {
        let html = r#"<html><head><title>Same</title>
            <meta name="description" content="Same"></head></html>"#;
        assert_eq!(metadata(html).description, None);
    }

    #[test]
    fn reports_an_empty_page_as_empty() {
        assert!(metadata("<html><head></head><body></body></html>").is_empty());
    }
}
