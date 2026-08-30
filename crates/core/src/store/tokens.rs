//! Bearer credentials for non-browser callers.
//!
//! Only hashes are stored, scope is fixed at mint time, and revocation is a
//! flag rather than a delete — see [`crate::token`] for why each of those
//! matters.

use super::Store;
use crate::error::{Error, Result};
use crate::token::{self, Issued, Principal, Scope, TokenInfo};
use chrono::Utc;
use rusqlite::{OptionalExtension, params};

impl Store {
    /// Mints a service token.
    ///
    /// The secret is returned once and never stored in the clear. Losing it
    /// means minting a replacement, which is the intended failure mode.
    pub fn issue_token(&self, name: &str, scope: Scope) -> Result<Issued> {
        let secret = token::generate()?;
        let id = token::hash(&secret)[..16].to_string();

        self.connection.execute(
            "INSERT INTO service_tokens (id, name, token_hash, scope, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, token::hash(&secret), scope.as_str(), Utc::now().to_rfc3339()],
        )?;

        Ok(Issued { id, name: name.to_string(), scope, token: secret })
    }

    /// Validates a presented token.
    ///
    /// Lookup is **by hash**, so the comparison happens inside `SQLite`'s index
    /// on a value that reveals nothing, and no code path here ever holds a
    /// stored secret to compare against — there is none.
    ///
    /// Returns `None` for every failure — unknown, malformed, revoked —
    /// because the caller answers one generic 401 regardless. Distinguishing
    /// them would let a caller probe for which tokens exist.
    /// Validates a presented token.
    ///
    /// Lookup is **by hash**, so the comparison happens inside `SQLite`'s index
    /// on a value that reveals nothing, and no code path here ever holds a
    /// stored secret to compare against — there is none.
    ///
    /// Returns `None` for every failure — unknown, malformed, revoked —
    /// because the caller answers one generic 401 regardless. Distinguishing
    /// them would let a caller probe for which tokens exist.
    pub fn authenticate(&self, presented: &str) -> Result<Option<Principal>> {
        if !token::looks_valid(presented) {
            return Ok(None);
        }

        let found: Option<(String, String, String, Option<String>)> = self
            .connection
            .query_row(
                "SELECT id, name, scope, revoked_at FROM service_tokens WHERE token_hash = ?1",
                params![token::hash(presented)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        let Some((id, name, scope, revoked_at)) = found else { return Ok(None) };
        if revoked_at.is_some() {
            return Ok(None);
        }

        Ok(Some(Principal { id, name, scope: scope.parse()? }))
    }

    /// Revokes a token by id.
    ///
    /// A flag rather than a delete, so the record of what a machine could
    /// reach survives the revocation. Returns whether a live token was
    /// revoked, so revoking twice is visible rather than silent.
    /// Revokes a token by id.
    ///
    /// A flag rather than a delete, so the record of what a machine could
    /// reach survives the revocation. Returns whether a live token was
    /// revoked, so revoking twice is visible rather than silent.
    pub fn revoke_token(&self, id: &str) -> Result<bool> {
        let affected = self.connection.execute(
            "UPDATE service_tokens SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            params![id, Utc::now().to_rfc3339()],
        )?;
        Ok(affected > 0)
    }

    /// Every token, newest first, including revoked ones.
    /// Every token, newest first, including revoked ones.
    pub fn list_tokens(&self) -> Result<Vec<TokenInfo>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, scope, created_at, revoked_at
               FROM service_tokens ORDER BY created_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            let scope: String = row.get(2)?;
            Ok(TokenInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                scope: scope.parse().unwrap_or(Scope::Read),
                created_at: row.get(3)?,
                revoked_at: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Error::from)
    }

    /// Whether any usable token exists.
    ///
    /// The API uses this to decide whether the registry is open or closed: a
    /// store with no tokens has not been configured for agent access yet, and
    /// refusing every call would be a confusing first run.
    /// Whether any usable token exists.
    ///
    /// The API uses this to decide whether the registry is open or closed: a
    /// store with no tokens has not been configured for agent access yet, and
    /// refusing every call would be a confusing first run.
    pub fn has_tokens(&self) -> Result<bool> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM service_tokens WHERE revoked_at IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}
