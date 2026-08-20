//! Typed Token + Scope.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// One row in the `tokens` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub id: Uuid,
    pub project_id: Uuid,
    pub scope: Scope,
    pub name: String,
    /// Last 4 chars of the original token for UI display — non-secret.
    pub last4: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub revoked_at: Option<OffsetDateTime>,
}

impl Token {
    /// True iff this token can authenticate requests.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }
}

/// `tokens.scope` enum (design.md §9).
///
/// The scopes answer one concrete question per endpoint: would it
/// be acceptable for a stranger holding a copy of the customer's
/// shipped app to do this? An `ingest` token is compiled into the
/// app — anyone who has the app has the token — so it may only
/// report events and attachments. An `api` token is held by the
/// customer's automation (CI, AI agents) and may read issues, pull
/// bundles, change status, append notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// SDK ingest — write events/attachments only.
    Ingest,
    /// Automation / AI closed loop — issue read + triage write.
    Api,
}

impl Scope {
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Ingest => "ingest",
            Self::Api => "api",
        }
    }

    #[must_use]
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "ingest" => Some(Self::Ingest),
            "api" => Some(Self::Api),
            _ => None,
        }
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    /// The db strings are the stored representation; renaming one
    /// silently reclassifies every existing token.
    #[test]
    fn db_strings_round_trip() {
        for k in [Scope::Ingest, Scope::Api] {
            assert_eq!(Scope::from_db_str(k.as_db_str()), Some(k));
        }
        assert_eq!(Scope::from_db_str("public"), None);
        assert_eq!(Scope::from_db_str(""), None);
    }
}
