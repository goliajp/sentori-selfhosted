//! [`WorkspaceScopedPool`] — pool wrapper that pins
//! `app.current_workspace` on every checkout so RLS policies
//! activate. See the migration 0001 header for the RLS model.
//!
//! Pattern:
//! ```no_run
//! # use sentori_tenant_scoping::WorkspaceScopedPool;
//! # use sentori_workspace_identity::WorkspaceId;
//! # async fn ex(pool: sqlx::PgPool, ws: WorkspaceId) -> Result<(), sqlx::Error> {
//! let scoped = WorkspaceScopedPool::new(pool, ws);
//! let mut conn = scoped.acquire().await?;
//! sqlx::query("SELECT id FROM projects").fetch_all(&mut *conn).await?;
//! # Ok(()) }
//! ```
//!
//! Connections borrowed from the underlying pool are reused.
//! `acquire()` sets the GUC at transaction or connection scope
//! via `set_config(..., true)` (the `true` is `is_local` —
//! resets to default when the connection returns to the pool /
//! the transaction commits).
//!
//! Janitors + migration runners that need to bypass RLS should
//! use the raw `PgPool` directly (Postgres superuser default
//! bypasses RLS).

use sqlx::{PgPool, Postgres, Transaction, pool::PoolConnection};

use crate::WorkspaceId;

/// Pool handle pinned to a specific [`WorkspaceId`]. Every
/// acquired connection / transaction has `app.current_workspace`
/// set, activating the RLS policies declared in the migrations.
#[derive(Debug, Clone)]
pub struct WorkspaceScopedPool {
    inner: PgPool,
    workspace: WorkspaceId,
}

impl WorkspaceScopedPool {
    /// Wrap the pool with a fixed workspace scope.
    #[must_use]
    pub const fn new(inner: PgPool, workspace: WorkspaceId) -> Self {
        Self { inner, workspace }
    }

    /// The workspace this pool is pinned to.
    #[must_use]
    pub const fn workspace(&self) -> WorkspaceId {
        self.workspace
    }

    /// Borrow the inner pool. Use sparingly — the borrowed
    /// pool does NOT auto-set the GUC; callers MUST set
    /// `app.current_workspace` before issuing queries, or RLS
    /// will reject every row. Prefer [`Self::acquire`] /
    /// [`Self::begin`] when possible.
    #[must_use]
    pub const fn raw(&self) -> &PgPool {
        &self.inner
    }

    /// Acquire a connection with `app.current_workspace`
    /// pre-set. The GUC is set with `is_local=true` so it
    /// resets when the connection returns to the pool.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`sqlx::Error`] from
    /// `pool.acquire()` or the `set_config` call.
    pub async fn acquire(&self) -> Result<PoolConnection<Postgres>, sqlx::Error> {
        let mut conn = self.inner.acquire().await?;
        sqlx::query("SELECT set_config('app.current_workspace', $1, true)")
            .bind(self.workspace.into_uuid().to_string())
            .execute(&mut *conn)
            .await?;
        Ok(conn)
    }

    /// Begin a transaction with `app.current_workspace` pinned
    /// for its scope. Commit / rollback resets the GUC
    /// (Postgres semantics for `SET LOCAL` / `set_config(...,
    /// true)` apply at transaction boundary).
    ///
    /// # Errors
    ///
    /// Returns the underlying [`sqlx::Error`] from
    /// `pool.begin()` or the `set_config` call.
    pub async fn begin(&self) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
        let mut tx = self.inner.begin().await?;
        sqlx::query("SELECT set_config('app.current_workspace', $1, true)")
            .bind(self.workspace.into_uuid().to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}
