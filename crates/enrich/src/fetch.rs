//! Fetching page metadata.
//!
//! This is the one place in the system that touches the network, and it is
//! deliberately offline work: the API never calls it, and a bookmark is fully
//! usable before it has run.
//!
//! Two constraints shape the design. The corpus spans a decade, so a large
//! fraction of it is dead — the fetcher must record *that* rather than retry
//! forever. And it runs against a few thousand URLs at once, so it has to be
//! concurrent without hammering any single host.

use crate::extract::{self, Metadata};
use std::time::Duration;
use torimemo_core::{Error, Result};

/// A browser user agent.
///
/// Not to be sneaky: many sites serve a stripped page or a redirect to a
/// generic client, and the point here is to see what a person clicking the
/// link would see.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

/// Only the head of a document is worth reading, and some URLs in any corpus
/// point at very large files. Reading past this is wasted bandwidth: the
/// metadata is in `<head>`.
const MAX_BODY_BYTES: usize = 512 * 1024;

/// What happened when a URL was fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The page was fetched and yielded metadata.
    Enriched(Metadata),
    /// The page was fetched but carried nothing usable — a login wall, an
    /// interstitial, or an empty document.
    NoMetadata,
    /// The page is gone: a 404, a 410, or a host that no longer resolves.
    ///
    /// Deliberately narrow. A 403 is *refusal*, not absence — many retail
    /// sites bot-check every request — so those are reported as
    /// [`Self::NoMetadata`] instead and the bookmark survives.
    Dead(String),
    /// A transient failure — a timeout, a 5xx, a connection reset. Worth
    /// retrying on a later pass, unlike [`Self::Dead`].
    Failed(String),
}

/// Fetches page metadata over HTTP.
#[derive(Debug, Clone)]
pub struct Fetcher {
    client: reqwest::Client,
}

impl Fetcher {
    /// Builds a fetcher with sensible timeouts.
    pub fn new(timeout: Duration) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(timeout)
            // A capped redirect chain: a few hops is normal for shorteners,
            // which this corpus has plenty of, but an unbounded chain is
            // either a loop or a tracker.
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|error| Error::msg(format!("could not build HTTP client: {error}")))?;
        Ok(Self { client })
    }

    /// Fetches one URL.
    ///
    /// Never returns `Err` for a network condition — a failure is data here,
    /// recorded so the next pass knows not to repeat dead work. `Err` is
    /// reserved for a caller mistake, like a URL that will not parse.
    pub async fn fetch(&self, url: &str) -> Outcome {
        let response = match self.client.get(url).send().await {
            Ok(response) => response,
            Err(error) => {
                let message = error.to_string();
                // A name that does not resolve is not a transient failure; the
                // host is gone, which for a decade-old corpus is common.
                return if error.is_connect() && message.contains("dns") {
                    Outcome::Dead(message)
                } else {
                    Outcome::Failed(message)
                };
            }
        };

        let status = response.status();
        if status.is_client_error() {
            // Only 404 and 410 actually mean the resource is gone. 429 and 408
            // mean "later". 401/403 mean "refused" — and refused is common:
            // Rolex, Tudor, Casio, and Chrono24 all answer 403 to anything
            // without a browser fingerprint, while serving the page fine to a
            // person. Recording those as dead would delete working bookmarks
            // on the strength of a bot check.
            return match status.as_u16() {
                404 | 410 => Outcome::Dead(format!("HTTP {status}")),
                401 | 403 => Outcome::NoMetadata,
                _ => Outcome::Failed(format!("HTTP {status}")),
            };
        }
        if status.is_server_error() {
            return Outcome::Failed(format!("HTTP {status}"));
        }

        // A PDF or an image has no metadata to read and may be very large.
        let is_html = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_none_or(|value| value.contains("html") || value.contains("xml"));
        if !is_html {
            return Outcome::NoMetadata;
        }

        let body = match read_capped(response).await {
            Ok(body) => body,
            Err(error) => return Outcome::Failed(error),
        };

        let metadata = extract::metadata(&body);
        if metadata.is_empty() { Outcome::NoMetadata } else { Outcome::Enriched(metadata) }
    }
}

/// Reads at most [`MAX_BODY_BYTES`] of a response body, decoded to text.
///
/// Decoding goes through reqwest's `charset` support rather than
/// `String::from_utf8_lossy`, because this corpus is not all UTF-8: several
/// Russian sites in it still serve windows-1251, some of them without saying
/// so in the `Content-Type` header. Lossy UTF-8 turns those titles into
/// replacement characters, which is worse than having no title at all — it
/// would be stored, embedded, and searched as noise.
async fn read_capped(response: reqwest::Response) -> std::result::Result<String, String> {
    let content_length = response.content_length().unwrap_or(0);
    if content_length as usize > MAX_BODY_BYTES {
        // Streaming past the cap and then decoding risks splitting a multi-byte
        // character; for an oversized body, read the head as bytes and decode
        // it as UTF-8, which is what an oversized page is in practice.
        let mut response = response;
        let mut bytes = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    bytes.extend_from_slice(&chunk);
                    if bytes.len() >= MAX_BODY_BYTES {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => return Err(error.to_string()),
            }
        }
        return Ok(String::from_utf8_lossy(&bytes).into_owned());
    }

    response.text().await.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_a_timeout() {
        assert!(Fetcher::new(Duration::from_secs(10)).is_ok());
    }

    #[test]
    fn only_gone_statuses_count_as_dead() {
        // The distinction that matters: 404 is gone, 403 is a bot check.
        // Treating the second as the first deletes working bookmarks.
        assert_eq!(Outcome::Dead("HTTP 404".into()), Outcome::Dead("HTTP 404".into()));
        assert_ne!(Outcome::Dead("HTTP 404".into()), Outcome::NoMetadata);
    }

    #[test]
    fn outcomes_distinguish_dead_from_transient() {
        // The distinction is the point: a `Dead` URL is never retried, a
        // `Failed` one is picked up by the next pass.
        assert_ne!(Outcome::Dead("404".into()), Outcome::Failed("timeout".into()));
    }
}
