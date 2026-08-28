//! Bearer credentials for non-browser callers.
//!
//! A service token is what an agent presents to reach `/v1/tools`. Three
//! properties define it, and each rules out a failure this system would
//! otherwise have:
//!
//! **Only the hash is stored.** A leaked database yields no usable credential.
//! The consequence is that a lost token cannot be recovered, only replaced —
//! the right trade for something that can write to the archive.
//!
//! **Scope is fixed at mint time.** There is no way to widen a live token; the
//! operator mints a new one and revokes the old. So a credential issued before
//! a capability existed can never acquire it.
//!
//! **Revocation is a flag, not a delete.** A revoked token stays visible, so
//! "what did this machine have access to" remains answerable.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// What a token may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// List and invoke read-only tools. The default.
    Read,
    /// Reads, plus tools that write to the archive.
    ReadWrite,
}

impl Scope {
    /// Whether this scope permits a tool that mutates.
    ///
    /// A method rather than a comparison at each call site, so adding a scope
    /// means editing one place instead of hoping every caller was updated.
    #[must_use]
    pub fn may_write(self) -> bool {
        matches!(self, Self::ReadWrite)
    }

    /// The stored representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::ReadWrite => "read_write",
        }
    }
}

impl std::str::FromStr for Scope {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "read" => Ok(Self::Read),
            "read_write" => Ok(Self::ReadWrite),
            other => Err(Error::msg(format!("unknown scope: {other}"))),
        }
    }
}

/// A validated token. Carries no secret material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// Which token this is, for audit.
    pub id: String,
    /// Its operator label.
    pub name: String,
    /// What it may do.
    pub scope: Scope,
}

/// A freshly minted token, including the secret shown **once**.
///
/// The `Debug` implementation is written by hand to redact `token`: a
/// credential in a log line or a panic backtrace is a live credential.
#[derive(Clone)]
pub struct Issued {
    /// The token's identity.
    pub id: String,
    /// Its operator label.
    pub name: String,
    /// What it may do.
    pub scope: Scope,
    /// The secret. Shown to the operator once and never stored in the clear.
    pub token: String,
}

impl std::fmt::Debug for Issued {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Issued")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("scope", &self.scope)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// A token as listed for an operator. No secret material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TokenInfo {
    /// The token's identity, which is what `revoke` takes.
    pub id: String,
    /// Its operator label.
    pub name: String,
    /// What it may do.
    pub scope: Scope,
    /// When it was minted.
    pub created_at: String,
    /// When it was revoked, if it was.
    pub revoked_at: Option<String>,
}

/// The prefix every token carries.
///
/// Makes a leaked credential recognizable in a log or a paste — the reason
/// providers give their keys distinctive prefixes — and lets an obviously
/// malformed value be rejected before any database work.
const PREFIX: &str = "tmk_";

/// Bytes of entropy per token.
///
/// 32 bytes is 256 bits: far beyond guessing, and the same order as the
/// session tokens this sits beside.
const ENTROPY_BYTES: usize = 32;

/// Generates a new secret.
pub fn generate() -> Result<String> {
    let mut bytes = [0_u8; ENTROPY_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| Error::msg(format!("could not read system randomness: {error}")))?;
    Ok(format!("{PREFIX}{}", hex(&bytes)))
}

/// Hashes a token for storage and lookup.
///
/// Blake3 rather than a password hash: the input is 256 bits of system
/// randomness, not a human-chosen secret, so there is nothing for a slow KDF
/// to defend against — a brute force is infeasible on entropy alone.
#[must_use]
pub fn hash(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

/// Whether a presented token is well-formed.
///
/// Cheap rejection before touching the database, so a flood of malformed
/// requests costs nothing.
#[must_use]
pub fn looks_valid(token: &str) -> bool {
    token.len() == PREFIX.len() + ENTROPY_BYTES * 2
        && token.starts_with(PREFIX)
        && token[PREFIX.len()..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Extracts a token from an `Authorization` header.
///
/// The scheme is matched case-insensitively because HTTP says it is
/// case-insensitive, and a client that sends `bearer` is not wrong.
#[must_use]
pub fn from_header(header: &str) -> Option<&str> {
    let (scheme, value) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = value.trim();
    if token.is_empty() { None } else { Some(token) }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr as _;

    #[test]
    fn a_generated_token_is_well_formed() {
        let token = generate().unwrap();
        assert!(token.starts_with(PREFIX));
        assert!(looks_valid(&token));
    }

    #[test]
    fn two_tokens_are_never_the_same() {
        assert_ne!(generate().unwrap(), generate().unwrap());
    }

    #[test]
    fn hashing_is_stable_and_is_not_the_token() {
        let token = generate().unwrap();
        assert_eq!(hash(&token), hash(&token));
        assert!(!hash(&token).contains(&token));
    }

    #[test]
    fn different_tokens_hash_differently() {
        assert_ne!(hash("tmk_a"), hash("tmk_b"));
    }

    #[test]
    fn malformed_tokens_are_rejected_before_any_lookup() {
        assert!(!looks_valid(""));
        assert!(!looks_valid("tmk_"));
        assert!(!looks_valid("nope_0123"));
        // Right shape, wrong alphabet.
        assert!(!looks_valid(&format!("{PREFIX}{}", "z".repeat(64))));
        // Right alphabet, wrong length.
        assert!(!looks_valid(&format!("{PREFIX}{}", "a".repeat(63))));
    }

    #[test]
    fn a_bearer_header_yields_its_token() {
        assert_eq!(from_header("Bearer tmk_abc"), Some("tmk_abc"));
        // HTTP says the scheme is case-insensitive.
        assert_eq!(from_header("bearer tmk_abc"), Some("tmk_abc"));
    }

    #[test]
    fn a_non_bearer_header_yields_nothing() {
        assert_eq!(from_header("Basic dXNlcjpwYXNz"), None);
        assert_eq!(from_header("tmk_abc"), None);
        assert_eq!(from_header("Bearer "), None);
        assert_eq!(from_header(""), None);
    }

    #[test]
    fn only_read_write_may_write() {
        assert!(!Scope::Read.may_write());
        assert!(Scope::ReadWrite.may_write());
    }

    #[test]
    fn scopes_round_trip_through_their_stored_form() {
        for scope in [Scope::Read, Scope::ReadWrite] {
            assert_eq!(Scope::from_str(scope.as_str()).unwrap(), scope);
        }
        assert!(Scope::from_str("admin").is_err());
    }

    #[test]
    fn debug_never_prints_the_secret() {
        let issued = Issued {
            id: "1".into(),
            name: "odin".into(),
            scope: Scope::Read,
            token: "tmk_supersecret".into(),
        };
        let rendered = format!("{issued:?}");
        assert!(!rendered.contains("supersecret"), "the secret leaked into Debug");
        assert!(rendered.contains("redacted"));
    }
}
