-- Sentori core migration 0032 — dashboard OAuth identities.
--
-- Legacy stored these as `users.oauth_provider` / `users.oauth_subject`.
-- v0.2's `users` has neither column, and `user_federation_links` is
-- not a substitute despite the name: it is project-scoped
-- (`project_id NOT NULL`), its `user_id` is TEXT rather than a
-- reference to `users`, and it carries an `install_id` — it models
-- identity federation for the *end users of a customer's app* on the
-- SDK side, not operator login to the dashboard.
--
-- A separate table rather than two nullable columns on `users`: one
-- account can link GitHub and Google both, which columns cannot
-- express without duplicating the row.

CREATE TABLE IF NOT EXISTS user_oauth_identities (
    id           UUID        PRIMARY KEY,
    user_id      UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- 'github' | 'google'. Kept open rather than CHECK-constrained so
    -- adding a provider is a code change, not a migration.
    provider     TEXT        NOT NULL,
    -- The provider's stable account id. Deliberately NOT the email:
    -- an address can be reassigned upstream, the subject cannot.
    subject      TEXT        NOT NULL,
    -- Cached for display only; never used to match an account.
    display_name TEXT,
    avatar_url   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at TIMESTAMPTZ,

    -- One upstream account maps to at most one Sentori user, so a
    -- second person cannot claim an identity already linked.
    UNIQUE (provider, subject),
    -- And a user links each provider at most once.
    UNIQUE (user_id, provider)
);

CREATE INDEX IF NOT EXISTS user_oauth_identities_user_idx
    ON user_oauth_identities (user_id);
