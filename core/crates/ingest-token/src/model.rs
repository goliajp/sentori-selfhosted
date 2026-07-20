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
    pub created_at: OffsetDateTime,
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
