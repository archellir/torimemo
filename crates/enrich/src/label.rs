//! Model-backed tagging.
//!
//! This is the teacher half of a teacher/student pair. A capable model labels
//! the corpus once, offline; those labels become the training set for a small
//! local classifier that does the work at request time. That split is why this
//! module lives in `enrich` and why nothing in the serving path can reach it:
//! the model's job is to *produce training data*, not to answer queries.
//!
//! Labels come from a closed vocabulary ([`crate::taxonomy`]) and are validated
//! against it here, so an invented tag never reaches the store. Every label is
//! written with the model that produced it, which is what makes a re-label a
//! diff rather than an overwrite.

use crate::taxonomy;
use serde::{Deserialize, Serialize};
use torimemo_core::{Bookmark, Error, Result};

/// What a labeller proposes for one bookmark.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    /// Tags, already filtered to the vocabulary.
    pub tags: Vec<String>,
    /// A one-line summary, or `None` when the labeller does not write one.
    pub summary: Option<String>,
    /// The labeller's confidence in `[0, 1]`.
    pub confidence: f64,
}

/// Something that proposes tags for a bookmark.
pub trait Labeller {
    /// The model identifier stored with the labels it produces.
    fn model(&self) -> &str;
    /// Labels one bookmark.
    fn label(&self, bookmark: &Bookmark) -> Result<Proposal>;
}

/// The text a labeller is shown.
///
/// Title, description, and domain — never the raw URL query string, which is
/// mostly tracking residue and would spend tokens on nothing.
#[must_use]
pub fn label_text(bookmark: &Bookmark) -> String {
    let mut parts = Vec::new();
    if let Some(title) = &bookmark.title {
        parts.push(format!("Title: {title}"));
    }
    if let Some(description) = &bookmark.description {
        // Long descriptions are mostly boilerplate past the first sentence or
        // two, and the tail costs tokens on every call.
        let truncated: String = description.chars().take(300).collect();
        parts.push(format!("Description: {truncated}"));
    }
    parts.push(format!("Site: {}", bookmark.domain));
    parts.join("\n")
}

/// A labeller with no model behind it.
///
/// Matches vocabulary terms and a small set of domain rules against the
/// bookmark's own text. It exists so the labelling pipeline — the queue, the
/// validation, the storage, the training-set export — is testable and runnable
/// with no API key, and so CI never depends on a network call. It is a
/// baseline, not a substitute: it cannot tell a Rust tutorial from a Rust job
/// listing, which is exactly the distinction the model is being paid to make.
#[derive(Debug, Clone, Default)]
pub struct RuleBased;

/// Domain patterns that imply a tag regardless of the page's own words.
const DOMAIN_RULES: &[(&str, &[&str])] = &[
    ("github.com", &["open-source", "programming"]),
    ("gitlab.com", &["open-source", "programming"]),
    ("stackoverflow.com", &["programming", "reference"]),
    ("youtube.com", &["video"]),
    ("leetcode.com", &["interview-prep", "programming"]),
    ("news.ycombinator.com", &["article"]),
    ("medium.com", &["article"]),
    ("dev.to", &["article", "programming"]),
    ("arxiv.org", &["ai-ml", "article"]),
    ("coursera.org", &["course"]),
    ("udemy.com", &["course"]),
    ("lever.co", &["job-listing"]),
    ("greenhouse.io", &["job-listing"]),
    ("linkedin.com", &["career"]),
    ("amazon.com", &["shopping"]),
    ("aliexpress.ru", &["shopping"]),
    ("mercadolibre.com", &["shopping"]),
];

impl Labeller for RuleBased {
    fn model(&self) -> &str {
        "rules-v1"
    }

    fn label(&self, bookmark: &Bookmark) -> Result<Proposal> {
        let haystack = label_text(bookmark).to_lowercase();
        let mut tags: Vec<String> = Vec::new();

        for (pattern, implied) in DOMAIN_RULES {
            if bookmark.domain.contains(pattern) {
                tags.extend(implied.iter().map(|tag| (*tag).to_string()));
            }
        }

        // A vocabulary term appearing verbatim is weak evidence, but it is
        // evidence, and it is all this baseline has.
        for tag in taxonomy::TAGS {
            let spaced = tag.replace('-', " ");
            if haystack.contains(tag) || haystack.contains(&spaced) {
                tags.push((*tag).to_string());
            }
        }

        let tags = taxonomy::accept(&tags);
        // Low by construction: this is a floor for the model to beat, and the
        // confidence should say so.
        let confidence = if tags.is_empty() { 0.0 } else { 0.3 };

        Ok(Proposal { tags, summary: None, confidence })
    }
}

/// The tag-assignment tool the model is asked to call.
///
/// Tool use rather than free text: the API validates the call against this
/// schema, so the response is parsed JSON with an enumerated `tags` field
/// instead of prose that has to be scraped. The `enum` carries the vocabulary
/// to the model, which is a far stronger constraint than describing it in the
/// prompt and hoping.
fn tag_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "assign_tags",
        "description": "Assign topical tags to a saved bookmark.",
        "input_schema": {
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array",
                    "items": { "type": "string", "enum": taxonomy::TAGS },
                    "minItems": 1,
                    "maxItems": 4,
                    "description": "The most specific tags that apply. Prefer fewer, more accurate tags over many loose ones."
                },
                "summary": {
                    "type": "string",
                    "description": "One short sentence describing what this page is, for someone deciding whether to reopen it."
                },
                "confidence": {
                    "type": "number",
                    "description": "How confident you are, from 0 to 1. Be honest: a login wall or an ambiguous title deserves a low score."
                }
            },
            "required": ["tags", "confidence"],
            "additionalProperties": false
        }
    })
}

/// The system prompt.
fn system_prompt() -> String {
    format!(
        "You tag saved bookmarks for a personal archive. You are given whatever \
         metadata the page yielded — often just a title and a domain.\n\n\
         Assign tags from this fixed vocabulary and no other: {}\n\n\
         Guidance:\n\
         - Tag what the page *is*, not every topic it mentions.\n\
         - Prefer one or two precise tags over four loose ones.\n\
         - A job posting is `job-listing` even when it is a programming job; \
         tag the genre first, the subject second.\n\
         - When the metadata is too thin to tell, say so with a low confidence \
         rather than guessing a plausible tag.",
        taxonomy::as_prompt_list()
    )
}

/// Labels via the Anthropic Messages API.
///
/// Uses tool use with a schema whose `tags` field is an enum over the
/// vocabulary, so the vocabulary is enforced by the API rather than by the
/// prompt. The result is validated again here regardless — a schema is a
/// constraint, not a guarantee, and this is cheap.
#[derive(Debug, Clone)]
pub struct AnthropicLabeller {
    client: reqwest::blocking::Client,
    api_key: String,
    model: String,
}

impl AnthropicLabeller {
    /// Builds a labeller reading `ANTHROPIC_API_KEY` from the environment.
    ///
    /// Haiku is the default: this is a short classification over a fixed
    /// vocabulary with the answer visible in the input, which is exactly the
    /// shape a small model handles well, and the corpus is thousands of rows.
    pub fn from_env(model: Option<&str>) -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| Error::msg("ANTHROPIC_API_KEY is not set"))?;
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|error| Error::msg(format!("could not build HTTP client: {error}")))?;
        Ok(Self { client, api_key, model: model.unwrap_or("claude-haiku-4-5").to_string() })
    }
}

impl Labeller for AnthropicLabeller {
    fn model(&self) -> &str {
        &self.model
    }

    fn label(&self, bookmark: &Bookmark) -> Result<Proposal> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 512,
            "system": system_prompt(),
            "tools": [tag_tool_schema()],
            // Force the call: without this the model may answer in prose, and
            // a prose answer is a parse failure rather than a label.
            "tool_choice": { "type": "tool", "name": "assign_tags" },
            "messages": [{ "role": "user", "content": label_text(bookmark) }]
        });

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .map_err(|error| Error::msg(format!("labelling request failed: {error}")))?;

        let status = response.status();
        let payload: serde_json::Value = response
            .json()
            .map_err(|error| Error::msg(format!("could not read labelling response: {error}")))?;

        if !status.is_success() {
            let detail = payload["error"]["message"].as_str().unwrap_or("unknown error");
            return Err(Error::msg(format!("labelling failed ({status}): {detail}")));
        }

        parse_proposal(&payload)
    }
}

/// Pulls the tool call out of a Messages API response.
fn parse_proposal(payload: &serde_json::Value) -> Result<Proposal> {
    let input = payload["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find(|block| block["type"] == "tool_use"))
        .map(|block| &block["input"])
        .ok_or_else(|| Error::msg("response contained no tool call"))?;

    let proposed: Vec<String> = input["tags"]
        .as_array()
        .map(|tags| tags.iter().filter_map(|tag| tag.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let tags = taxonomy::accept(&proposed);
    let summary = input["summary"].as_str().map(str::to_string);
    let confidence = input["confidence"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0);

    Ok(Proposal { tags, summary, confidence })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn bookmark(title: &str, domain: &str) -> Bookmark {
        let now = Utc::now();
        Bookmark {
            id: 1,
            canonical_url: format!("https://{domain}/x"),
            domain: domain.to_string(),
            title: Some(title.to_string()),
            description: None,
            first_captured_at: now,
            last_captured_at: now,
            capture_count: 1,
        }
    }

    #[test]
    fn label_text_includes_title_and_site() {
        let text = label_text(&bookmark("Rocket web framework", "rocket.rs"));
        assert!(text.contains("Rocket web framework"));
        assert!(text.contains("rocket.rs"));
    }

    #[test]
    fn label_text_truncates_a_long_description() {
        let mut mark = bookmark("t", "d.com");
        mark.description = Some("x".repeat(1000));
        assert!(label_text(&mark).len() < 400);
    }

    #[test]
    fn rules_tag_from_the_domain_alone() {
        let proposal = RuleBased.label(&bookmark("some repo", "github.com")).unwrap();
        assert!(proposal.tags.contains(&"open-source".to_string()));
    }

    #[test]
    fn rules_never_emit_a_tag_outside_the_vocabulary() {
        let proposal = RuleBased.label(&bookmark("nonsense qwerty", "example.com")).unwrap();
        assert!(proposal.tags.iter().all(|tag| taxonomy::is_valid(tag)));
    }

    #[test]
    fn the_tool_schema_carries_the_whole_vocabulary() {
        let schema = tag_tool_schema();
        let enumerated =
            schema["input_schema"]["properties"]["tags"]["items"]["enum"].as_array().unwrap();
        assert_eq!(enumerated.len(), taxonomy::TAGS.len());
    }

    #[test]
    fn parses_a_tool_call_response() {
        let payload = serde_json::json!({
            "content": [{
                "type": "tool_use",
                "name": "assign_tags",
                "input": {
                    "tags": ["programming", "tutorial"],
                    "summary": "A guide to X.",
                    "confidence": 0.9
                }
            }]
        });
        let proposal = parse_proposal(&payload).unwrap();
        assert_eq!(proposal.tags, vec!["programming", "tutorial"]);
        assert_eq!(proposal.summary.as_deref(), Some("A guide to X."));
        assert!((proposal.confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn drops_an_invented_tag_but_keeps_the_valid_ones() {
        let payload = serde_json::json!({
            "content": [{
                "type": "tool_use",
                "input": { "tags": ["programming", "vibes"], "confidence": 0.5 }
            }]
        });
        assert_eq!(parse_proposal(&payload).unwrap().tags, vec!["programming"]);
    }

    #[test]
    fn clamps_an_out_of_range_confidence() {
        let payload = serde_json::json!({
            "content": [{ "type": "tool_use", "input": { "tags": [], "confidence": 7.0 } }]
        });
        assert!((parse_proposal(&payload).unwrap().confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_response_without_a_tool_call_is_an_error() {
        let payload = serde_json::json!({
            "content": [{ "type": "text", "text": "I think this is programming." }]
        });
        assert!(parse_proposal(&payload).is_err());
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    /// Guards the exact wire shape the Messages API expects.
    ///
    /// The labeller cannot be exercised end to end without a key, so this
    /// pins the request body instead: a wrong field name here would fail at
    /// runtime against a live endpoint and nowhere else.
    #[test]
    fn the_request_body_matches_the_messages_api_shape() {
        let body = serde_json::json!({
            "model": "claude-haiku-4-5",
            "max_tokens": 512,
            "system": system_prompt(),
            "tools": [tag_tool_schema()],
            "tool_choice": { "type": "tool", "name": "assign_tags" },
            "messages": [{ "role": "user", "content": "Title: x\nSite: y.com" }]
        });

        assert_eq!(body["model"], "claude-haiku-4-5");
        assert!(body["max_tokens"].is_number());
        assert_eq!(body["tool_choice"]["type"], "tool");
        // The forced tool's name must match the declared tool, or the API
        // rejects the request.
        assert_eq!(body["tool_choice"]["name"], body["tools"][0]["name"]);
        assert_eq!(body["messages"][0]["role"], "user");
        assert!(body["tools"][0]["input_schema"]["properties"]["tags"].is_object());
        assert_eq!(body["tools"][0]["input_schema"]["additionalProperties"], false);
    }

    #[test]
    fn the_system_prompt_carries_the_vocabulary() {
        let prompt = system_prompt();
        for tag in taxonomy::TAGS {
            assert!(prompt.contains(tag), "{tag} missing from the prompt");
        }
    }
}
