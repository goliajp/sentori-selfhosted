//! Typed Token + TokenKind.

use sentori_workspace_identity::{ProjectId, WorkspaceId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// One row in the `tokens` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub id: Uuid,
    pub workspace_id: WorkspaceId,
    pub project_id: ProjectId,
    pub kind: TokenKind,
    pub label: Option<String>,
    /// Last 4 chars of the original token for UI display
    /// (`ad4f` etc.) — non-secret.
    pub last4: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub revoked_at: Option<OffsetDateTime>,
}

impl Token {
    /// True iff this token can authenticate SDK requests.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }
}

/// `tokens.kind` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    /// SDK ingest token — clients send this in `Authorization:
    /// Bearer st_pk_...`.
    Public,
    /// Server-to-server admin token — same wire prefix but
    /// different kind enforced server-side.
    Admin,
}

impl TokenKind {
    /// Whether this token may perform server-side operations.
    ///
    /// The two kinds share a wire prefix, so the middleware cannot
    /// tell them apart and does not try — it authenticates the token
    /// and hands the kind to the handler. That arrangement only works
    /// if handlers actually look, and for a long time none did: the
    /// kind rode along in `IngestContext` and every endpoint behind
    /// the middleware accepted both.
    ///
    /// The distinction is not about trust levels in the abstract. A
    /// public token is *compiled into a shipped application* — anyone
    /// who has the app has the token. So the question each endpoint
    /// must answer is concrete: would it be acceptable for a stranger
    /// with a copy of the customer's app to do this? Reporting a crash,
    /// yes. Uploading a source map that rewrites how every stack in a
    /// release is read, or sending a push notification to the
    /// customer's users, no.
    #[must_use]
    pub const fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }

    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Admin => "admin",
        }
    }

    #[must_use]
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "public" => Some(Self::Public),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }
}

#[cfg(test)]
mod kind_tests {
    use super::*;

    #[test]
    fn only_admin_is_admin() {
        assert!(TokenKind::Admin.is_admin());
        assert!(!TokenKind::Public.is_admin());
    }

    /// The db strings are the stored representation; renaming one
    /// silently reclassifies every existing token.
    #[test]
    fn db_strings_round_trip() {
        for k in [TokenKind::Public, TokenKind::Admin] {
            assert_eq!(TokenKind::from_db_str(k.as_db_str()), Some(k));
        }
        assert_eq!(TokenKind::from_db_str("Admin"), None);
        assert_eq!(TokenKind::from_db_str(""), None);
    }
}
