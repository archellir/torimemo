//! Importers for the places links already live.
//!
//! These are one-shot backfills, not a sync: once a capture surface exists,
//! links arrive through it. The job here is to get the existing corpus in with
//! its real timestamps and provenance intact.

use crate::model::{NewCapture, Source};
use chrono::{DateTime, Utc};

/// Matches an absolute `http(s)` URL inside prose or a markdown link.
///
/// Deliberately not a full URL grammar: the input is scraped markdown, and the
/// only job is to find candidate spans. Anything that is not really a URL fails
/// in [`crate::normalize::canonicalize`] and is reported as skipped, which is a
/// better place to be strict than here.
fn extract_urls(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let Some(offset) = text[index..].find("http") else { break };
        let start = index + offset;
        let rest = &text[start..];

        if !(rest.starts_with("http://") || rest.starts_with("https://")) {
            index = start + 4;
            continue;
        }

        // Stop at whitespace or at a character that only appears as markdown
        // or prose punctuation around a link, never inside one.
        let end = rest
            .find(|character: char| {
                character.is_whitespace() || matches!(character, ')' | '>' | '"' | '\'' | '`' | ']')
            })
            .map_or(rest.len(), |position| position);

        let candidate = rest[..end].trim_end_matches(['.', ',', ';', ':']);

        if candidate.len() > "https://".len() {
            found.push(candidate.to_string());
        }
        index = start + end.max(1);
    }

    found
}

/// Extracts every URL from a markdown file's text.
///
/// `context` labels where the links came from — the vault filename — so a
/// bookmark can later be traced to `Saved telegram.md` rather than just
/// "the vault".
#[must_use]
pub fn from_markdown(text: &str, context: &str, captured_at: DateTime<Utc>) -> Vec<NewCapture> {
    extract_urls(text)
        .into_iter()
        .map(|url| {
            NewCapture::new(url, Source::Vault).with_context(context.to_string()).at(captured_at)
        })
        .collect()
}

/// Parses a Netscape bookmark file, the export format every browser shares.
///
/// Each bookmark is an `<A HREF="..." ADD_DATE="...">Title</A>`. The add-date
/// is a Unix timestamp and is worth recovering: it is the only honest record
/// of when a link was first saved, and recency is a ranking feature.
#[must_use]
pub fn from_netscape(html: &str, source: Source) -> Vec<NewCapture> {
    let mut captures = Vec::new();

    for line in html.lines() {
        let lowered = line.to_ascii_lowercase();
        let Some(anchor) = lowered.find("<a ") else { continue };
        let Some(href_start) = lowered[anchor..].find("href=\"").map(|offset| anchor + offset + 6)
        else {
            continue;
        };
        let Some(href_length) = line[href_start..].find('"') else { continue };
        let url = &line[href_start..href_start + href_length];

        let captured_at = lowered[anchor..]
            .find("add_date=\"")
            .map(|offset| anchor + offset + 10)
            .and_then(|start| {
                let length = line[start..].find('"')?;
                line[start..start + length].parse::<i64>().ok()
            })
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
            .unwrap_or_else(Utc::now);

        let title = line[href_start + href_length..]
            .find('>')
            .map(|offset| &line[href_start + href_length + offset + 1..])
            .and_then(|rest| rest.split('<').next())
            .map(str::trim)
            .filter(|title| !title.is_empty());

        let mut capture = NewCapture::new(url, source).at(captured_at);
        if let Some(title) = title {
            capture = capture.with_context(title.to_string());
        }
        captures.push(capture);
    }

    captures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_bare_urls_one_per_line() {
        // The shape of every file in the vault's `ALL BOOKMARKS` directory.
        let text = "https://example.com/a\nhttps://example.com/b\n";
        assert_eq!(extract_urls(text).len(), 2);
    }

    #[test]
    fn extracts_urls_from_markdown_links_without_the_paren() {
        let text = "- use [Paperless](https://github.com/paperless-ngx/paperless-ngx)";
        assert_eq!(extract_urls(text), vec!["https://github.com/paperless-ngx/paperless-ngx"]);
    }

    #[test]
    fn strips_trailing_sentence_punctuation() {
        assert_eq!(extract_urls("see https://example.com/a."), vec!["https://example.com/a"]);
    }

    #[test]
    fn keeps_query_strings_intact() {
        let text = "https://example.com/a?b=1&c=2 next";
        assert_eq!(extract_urls(text), vec!["https://example.com/a?b=1&c=2"]);
    }

    #[test]
    fn ignores_the_word_http_outside_a_url() {
        assert!(extract_urls("the http protocol").is_empty());
    }

    #[test]
    fn netscape_import_recovers_url_title_and_date() {
        let html = r#"<DT><A HREF="https://example.com/a" ADD_DATE="1600000000">Example Title</A>"#;
        let captures = from_netscape(html, Source::BrowserImport);

        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].raw_url, "https://example.com/a");
        assert_eq!(captures[0].context.as_deref(), Some("Example Title"));
        assert_eq!(captures[0].captured_at, DateTime::from_timestamp(1_600_000_000, 0));
    }
}
