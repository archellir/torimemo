//! Importing a GitHub user's starred repositories.
//!
//! Stars are a better record of "repositories I care about" than bookmarks
//! ever were: they are maintained in the place you actually press the button,
//! they carry the repo's own description and language, and re-running this
//! picks up everything starred since the last run. A bookmarked repo is a
//! stale snapshot of the same information.
//!
//! This lives in `enrich` rather than `core` because it reaches the network,
//! and the serving path must not.

use serde::Deserialize;
use std::time::Duration;
use torimemo_core::{Error, NewCapture, Result, Source};

/// Repositories per request. GitHub's maximum, so a 561-star account costs
/// six requests rather than sixty.
const PAGE_SIZE: usize = 100;

/// A hard stop on pagination, so a malformed `Link` header or an API change
/// cannot turn this into an unbounded loop.
const MAX_PAGES: usize = 100;

/// The fields of a starred repository this import reads.
#[derive(Debug, Clone, Deserialize)]
struct Repository {
    full_name: String,
    html_url: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    language: Option<String>,
    /// Whether the repository is archived — read-only and no longer developed.
    #[serde(default)]
    archived: bool,
    /// Whether it is a fork rather than an original.
    #[serde(default)]
    fork: bool,
}

impl Repository {
    /// The context stored with the capture.
    ///
    /// The repo's own description, plus its language and status. This is what
    /// makes an imported star searchable immediately, without the enrichment
    /// pass having to fetch the page.
    fn context(&self) -> String {
        let mut parts = vec![self.full_name.clone()];
        if let Some(description) = &self.description {
            parts.push(description.clone());
        }
        if let Some(language) = &self.language {
            parts.push(language.clone());
        }
        if self.archived {
            parts.push("archived".to_string());
        }
        if self.fork {
            parts.push("fork".to_string());
        }
        parts.join(" — ")
    }
}

/// What to import.
#[derive(Debug, Clone)]
pub struct Config {
    /// Whose stars to read.
    pub user: String,
    /// A personal access token. Optional, but unauthenticated requests are
    /// limited to 60 per hour against a shared pool, so a large account may
    /// need one.
    pub token: Option<String>,
    /// Skip repositories the owner has archived. They still exist, but a
    /// read-only repository is rarely what someone wants surfaced.
    pub skip_archived: bool,
    /// Skip forks, which are usually a working copy rather than a thing
    /// starred for its own sake.
    pub skip_forks: bool,
}

/// What the import found.
#[derive(Debug, Default, Clone, Copy)]
pub struct Summary {
    /// Stars seen across every page.
    pub fetched: usize,
    /// Repositories skipped as archived or as forks.
    pub skipped: usize,
    /// New bookmarks created.
    pub created: usize,
    /// Stars already present, recorded as another capture.
    pub merged: usize,
}

/// Fetches every starred repository and returns them as captures.
///
/// Pagination follows the page number rather than the `Link` header: the
/// header is more correct in principle, but a short page is an unambiguous
/// end-of-results signal and needs no URL parsing.
pub async fn fetch(config: &Config) -> Result<(Vec<NewCapture>, Summary)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        // GitHub rejects requests without one.
        .user_agent("torimemo")
        .build()
        .map_err(|error| Error::msg(format!("could not build HTTP client: {error}")))?;

    let mut captures = Vec::new();
    let mut summary = Summary::default();

    for page in 1..=MAX_PAGES {
        let url = format!(
            "https://api.github.com/users/{}/starred?per_page={PAGE_SIZE}&page={page}",
            config.user
        );

        let mut request = client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &config.token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }

        let response = request
            .send()
            .await
            .map_err(|error| Error::msg(format!("could not reach GitHub: {error}")))?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::msg(format!("no such GitHub user: {}", config.user)));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            // Rate limiting arrives as a 403 rather than a 429, and the fix is
            // a token, so say so rather than reporting a bare status.
            return Err(Error::msg(
                "GitHub refused the request; unauthenticated calls are limited to \
                 60 per hour — pass --token to raise it to 5000",
            ));
        }
        if !status.is_success() {
            return Err(Error::msg(format!("GitHub returned {status}")));
        }

        let repositories: Vec<Repository> = response
            .json()
            .await
            .map_err(|error| Error::msg(format!("could not read GitHub's response: {error}")))?;

        let page_size = repositories.len();
        summary.fetched += page_size;

        for repository in repositories {
            if (config.skip_archived && repository.archived)
                || (config.skip_forks && repository.fork)
            {
                summary.skipped += 1;
                continue;
            }
            captures.push(
                NewCapture::new(repository.html_url.clone(), Source::BrowserImport)
                    .with_context(repository.context()),
            );
        }

        // A short page is the last page.
        if page_size < PAGE_SIZE {
            break;
        }
    }

    Ok((captures, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository(archived: bool, fork: bool) -> Repository {
        Repository {
            full_name: "rust-lang/rust-analyzer".into(),
            html_url: "https://github.com/rust-lang/rust-analyzer".into(),
            description: Some("A Rust compiler front-end for IDEs".into()),
            language: Some("Rust".into()),
            archived,
            fork,
        }
    }

    #[test]
    fn context_carries_the_repos_own_description_and_language() {
        let context = repository(false, false).context();
        assert!(context.contains("rust-lang/rust-analyzer"));
        assert!(context.contains("A Rust compiler front-end for IDEs"));
        assert!(context.contains("Rust"));
    }

    #[test]
    fn archived_and_fork_status_are_recorded_in_the_context() {
        assert!(repository(true, false).context().contains("archived"));
        assert!(repository(false, true).context().contains("fork"));
        assert!(!repository(false, false).context().contains("archived"));
    }

    #[test]
    fn a_repo_without_a_description_still_has_usable_context() {
        let mut repo = repository(false, false);
        repo.description = None;
        repo.language = None;
        assert_eq!(repo.context(), "rust-lang/rust-analyzer");
    }

    #[test]
    fn the_api_response_shape_parses() {
        // Trimmed from a real /starred response; a field rename upstream
        // would fail here rather than at import time.
        let json = r#"[{
            "full_name": "rust-lang/rust-analyzer",
            "html_url": "https://github.com/rust-lang/rust-analyzer",
            "description": "A Rust compiler front-end for IDEs",
            "language": "Rust",
            "archived": false,
            "fork": false,
            "stargazers_count": 16805
        }]"#;
        let parsed: Vec<Repository> = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].full_name, "rust-lang/rust-analyzer");
    }

    #[test]
    fn missing_optional_fields_do_not_break_parsing() {
        let json = r#"[{"full_name": "a/b", "html_url": "https://github.com/a/b"}]"#;
        let parsed: Vec<Repository> = serde_json::from_str(json).unwrap();
        assert!(parsed[0].description.is_none());
        assert!(!parsed[0].archived);
    }
}
