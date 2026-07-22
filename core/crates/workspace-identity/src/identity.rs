//! [`Identity`] — the one handle a caller constructs, fanning
//! out into five typed sub-handles, all scoped to a fixed
//! [`WorkspaceId`].

use sqlx::{PgPool, Row};

use crate::error::IdentityError;
use crate::model::{User, UserId};
use crate::store::{Invites, Members, Projects, Users, Visibility};
use crate::{Role, WorkspaceId};

/// Workspace-scoped handle into the identity tables.
///
/// Holds a `PgPool` clone + a fixed [`WorkspaceId`]. The five
/// sub-handles inherit both — every CRUD call automatically
/// filters / writes against `workspace_id`. There is no API
/// surface for cross-workspace queries; create one [`Identity`]
/// per workspace context.
///
/// ## Bootstrapping a new workspace
///
/// Use the free function `bootstrap_workspace(&pool, name)` (in
/// the crate root) to INSERT the `workspaces` row, then
/// construct [`Identity::new`] against the returned id.
///
/// ## Pool routing for saas
///
/// In `SaaS` deployments the per-request middleware should set the
/// `app.current_workspace` GUC on the borrowed connection so the
/// RLS policies (see migration 0001) match. The store methods
/// here also explicitly bind `workspace_id` in WHERE clauses /
/// INSERT lists — RLS is defense-in-depth, not the only
/// boundary.
#[derive(Debug, Clone)]
pub struct Identity {
    pool: PgPool,
    workspace_id: WorkspaceId,
}

impl Identity {
    /// Wrap a pool against a specific workspace.
    #[must_use]
    pub const fn new(pool: PgPool, workspace_id: WorkspaceId) -> Self {
        Self { pool, workspace_id }
    }

    /// Borrow the underlying pool for callers that need to run
    /// their own ad-hoc queries against the same connection.
    /// Prefer the sub-handle methods.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// The workspace this handle is scoped to.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// CRUD on `users`.
    #[must_use]
    pub const fn users(&self) -> Users<'_> {
        Users::new(&self.pool, self.workspace_id)
    }

    /// CRUD on `workspace_members` (RBAC: owner / admin / user).
    #[must_use]
    pub const fn members(&self) -> Members<'_> {
        Members::new(&self.pool, self.workspace_id)
    }

    /// CRUD on `projects` (+ owning `privacy_salts`).
    #[must_use]
    pub const fn projects(&self) -> Projects<'_> {
        Projects::new(&self.pool, self.workspace_id)
    }

    /// CRUD on `project_user_visibility` (per-project ACL for
    /// `Role::User`).
    #[must_use]
    pub const fn visibility(&self) -> Visibility<'_> {
        Visibility::new(&self.pool, self.workspace_id)
    }

    /// CRUD on `workspace_invites`.
    #[must_use]
    pub const fn invites(&self) -> Invites<'_> {
        Invites::new(&self.pool, self.workspace_id)
    }

    /// Provision a brand-new tenant atomically: create a fresh
    /// `workspaces` row, the first `users` row inside it, and an
    /// `owner` `workspace_members` row — all in one transaction.
    ///
    /// This is the `SaaS` self-signup primitive. It deliberately
    /// does NOT use `self.workspace_id` (which scopes the CRUD
    /// sub-handles); it mints a new workspace so each self-signup
    /// lands in its own isolated tenant rather than the shared
    /// default. `email` must already be normalised (lowercased +
    /// trimmed) by the caller; the password is stored as the given
    /// argon2 PHC string.
    ///
    /// Atomicity matters: if the user INSERT fails (e.g. the email
    /// is already registered), the whole transaction rolls back so
    /// no empty orphan workspace is left behind. The caller seeds
    /// the billing row separately (this crate does not depend on
    /// billing).
    ///
    /// Returns the new [`User`] plus the [`WorkspaceId`] it now
    /// owns.
    ///
    /// # Errors
    ///
    /// - [`IdentityError::EmailTaken`] if the email is already on
    ///   file (globally unique).
    /// - [`IdentityError::Db`] on any other database failure.
    pub async fn register_tenant_tx(
        &self,
        email: &str,
        password_hash: &str,
        workspace_name: &str,
    ) -> Result<(User, WorkspaceId), IdentityError> {
        let workspace_id = WorkspaceId::new();
        let user_id = UserId::new();

        let mut tx = self.pool.begin().await?;

        sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
            .bind(workspace_id.into_uuid())
            .bind(workspace_name)
            .execute(&mut *tx)
            .await?;

        let user_row = sqlx::query(
            "INSERT INTO users (id, workspace_id, email, password_hash, email_verified) \
             VALUES ($1, $2, $3, $4, FALSE) \
             RETURNING id, email, email_verified, created_at",
        )
        .bind(user_id.into_uuid())
        .bind(workspace_id.into_uuid())
        .bind(email)
        .bind(password_hash)
        .fetch_one(&mut *tx)
        .await
        .map_err(translate_unique_email)?;

        sqlx::query(
            "INSERT INTO workspace_members (workspace_id, user_id, role, added_by) \
             VALUES ($1, $2, $3, $2)",
        )
        .bind(workspace_id.into_uuid())
        .bind(user_id.into_uuid())
        .bind(Role::Owner.as_db_str())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let user = User {
            id: UserId::from_uuid(user_row.get("id")),
            email: user_row.get("email"),
            email_verified: user_row.get("email_verified"),
            created_at: user_row.get("created_at"),
        };
        Ok((user, workspace_id))
    }
}

/// Map a unique-violation on the users email index to the typed
/// [`IdentityError::EmailTaken`]. Mirrors the private helper in the
/// `users` store so the transactional signup path returns the same
/// error the non-transactional `Users::create` does.
fn translate_unique_email(err: sqlx::Error) -> IdentityError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23505")
    {
        return IdentityError::EmailTaken;
    }
    IdentityError::Db(err)
}

/// Resolve which workspace an invite token belongs to, without
/// knowing the workspace up front.
///
/// The invite-accept flow only has the token (from the emailed
/// link); the [`crate::Invites::accept`] store method is
/// workspace-scoped. This looks the invite up by its globally-unique
/// `token_hash` and returns the owning workspace so the caller can
/// construct a scoped [`Identity`] and call `accept`. Only pending,
/// unexpired invites resolve — an accepted or expired token returns
/// `None` (indistinguishable from an unknown one).
///
/// # Errors
///
/// - [`IdentityError::InviteInvalid`] if the token is malformed.
/// - [`IdentityError::Db`] on database failure.
pub async fn resolve_invite_workspace(
    pool: &PgPool,
    token_wire: &str,
) -> Result<Option<WorkspaceId>, IdentityError> {
    let token_hash = crate::InviteToken::parse_and_hash(token_wire)?;
    let row: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT workspace_id FROM workspace_invites \
         WHERE token_hash = $1 AND accepted_at IS NULL AND expires_at > now()",
    )
    .bind(token_hash.as_bytes().as_slice())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id,)| WorkspaceId::from_uuid(id)))
}

/// Bootstrap a new workspace row. Used at server startup
/// (self-hosted's default workspace) and by `SaaS` signup flow.
///
/// Returns the new [`WorkspaceId`]; immediately usable as
/// argument to [`Identity::new`].
///
/// # Errors
///
/// Forwards any sqlx error.
pub async fn bootstrap_workspace(pool: &PgPool, name: &str) -> Result<WorkspaceId, sqlx::Error> {
    let id = WorkspaceId::new();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id.into_uuid())
        .bind(name)
        .execute(pool)
        .await?;
    Ok(id)
}

/// Ensure a workspace exists with the given id (idempotent). The
/// `SaaS` signup flow uses this to round-trip through transactions
/// where the workspace id is minted before the row commits.
///
/// # Errors
///
/// Forwards any sqlx error.
pub async fn ensure_workspace(
    pool: &PgPool,
    id: WorkspaceId,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO workspaces (id, name) VALUES ($1, $2) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(id.into_uuid())
    .bind(name)
    .execute(pool)
    .await?;
    Ok(())
}
