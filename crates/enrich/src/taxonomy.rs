//! The tag vocabulary.
//!
//! Labels come from a fixed list rather than free-form model output, for three
//! reasons. Free text would produce "webdev", "web-dev", "web development",
//! and "frontend" as four tags for one idea, and no amount of prompting
//! reliably prevents that across a thousand calls. A closed set makes the
//! distilled classifier a well-posed problem — a fixed number of labels with
//! real support, instead of a long tail of singletons. And a closed set can be
//! validated: a label the model invents is caught here rather than silently
//! entering the store.
//!
//! The vocabulary was derived from this corpus, not chosen in the abstract:
//! it covers software, jobs, and learning because that is most of the corpus,
//! and motorcycles, fitness, and shopping because those are real clusters in
//! it too. Adding a tag is cheap; the cost is that every existing label
//! predates it, so additions should be rare and deliberate.

/// Every tag a labeller may assign.
pub const TAGS: &[&str] = &[
    // Software: what the corpus is mostly made of, so it earns subdivision.
    "programming",
    "web-dev",
    "backend",
    "frontend",
    "devops",
    "database",
    "ai-ml",
    "security",
    "open-source",
    "developer-tool",
    "api",
    "system-design",
    // Work and learning.
    "job-listing",
    "career",
    "interview-prep",
    "course",
    "tutorial",
    "documentation",
    "book",
    // Reading and media.
    "article",
    "video",
    "podcast",
    "newsletter",
    "social-post",
    // Business.
    "startup",
    "business",
    "finance",
    "crypto",
    "marketing",
    // Personal interests visible as real clusters in this corpus.
    "motorcycle",
    "automotive",
    "fitness",
    "health",
    "travel",
    "food",
    "music",
    "gaming",
    "design",
    "photography",
    // Practical.
    "shopping",
    "tool",
    "reference",
    "government",
    "real-estate",
    "personal-admin",
];

/// Whether `tag` is in the vocabulary.
#[must_use]
pub fn is_valid(tag: &str) -> bool {
    TAGS.contains(&tag)
}

/// Keeps only the valid tags in `proposed`, lowercased and deduplicated.
///
/// Invalid labels are dropped rather than rejected wholesale: a model that
/// returns three good tags and one invented one has still done useful work,
/// and discarding the whole row would lose it.
#[must_use]
pub fn accept(proposed: &[String]) -> Vec<String> {
    let mut accepted: Vec<String> = Vec::new();
    for tag in proposed {
        let normalized = tag.trim().to_lowercase();
        if is_valid(&normalized) && !accepted.contains(&normalized) {
            accepted.push(normalized);
        }
    }
    accepted
}

/// The vocabulary as a comma-separated list, for a prompt.
#[must_use]
pub fn as_prompt_list() -> String {
    TAGS.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_has_no_duplicates() {
        let mut sorted = TAGS.to_vec();
        sorted.sort_unstable();
        let count = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), count, "the tag list contains a duplicate");
    }

    #[test]
    fn tags_are_lowercase_kebab_case() {
        for tag in TAGS {
            assert!(
                tag.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{tag} is not lowercase kebab-case"
            );
        }
    }

    #[test]
    fn accept_keeps_known_tags_and_drops_invented_ones() {
        let proposed = vec!["programming".into(), "not-a-real-tag".into(), "devops".into()];
        assert_eq!(accept(&proposed), vec!["programming", "devops"]);
    }

    #[test]
    fn accept_normalizes_case_and_whitespace() {
        assert_eq!(accept(&["  DevOps  ".into()]), vec!["devops"]);
    }

    #[test]
    fn accept_deduplicates() {
        assert_eq!(accept(&["devops".into(), "DevOps".into()]), vec!["devops"]);
    }

    #[test]
    fn accept_returns_empty_rather_than_failing_on_all_invalid() {
        assert!(accept(&["nonsense".into()]).is_empty());
    }
}
