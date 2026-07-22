-- Sentori core migration 0017 — releases + release_artifacts.
--
-- A `release` is a deploy marker uploaded via `POST /v1/deploys`.
-- Events / sessions / dsyms / proguard mappings reference it by
-- the (project_id, name) pair.
--
-- `release_artifacts` is the metadata row for an uploaded
-- sourcemap / mapping bundle. The actual bytes live at
-- `blob_path` in the configured object store; `content_hash`
-- doubles as the dedup key.
--
-- Column names verbatim from legacy
-- `server/migrations/0002_issues.sql` (releases),
-- `0005_release_artifacts.sql` (release_artifacts),
-- `0016_releases_deploy_at.sql` (releases.deploy_at),
-- `0057_release_artifacts_metadata.sql` (entry_count,
-- uncompressed_size_bytes),
-- `0062_source_bundle_multi.sql` (module_label).

CREATE TABLE IF NOT EXISTS releases (
    id           UUID        PRIMARY KEY,
    workspace_id UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    deploy_at    TIMESTAMPTZ,
    UNIQUE (project_id, name)
);

CREATE INDEX IF NOT EXISTS releases_project_deploy_idx
    ON releases (project_id, deploy_at DESC);

CREATE TABLE IF NOT EXISTS release_artifacts (
    id                      UUID        PRIMARY KEY,
    workspace_id            UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    release_id              UUID        NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    kind                    TEXT        NOT NULL,
    name                    TEXT        NOT NULL,
    content_hash            TEXT        NOT NULL,
    blob_path               TEXT        NOT NULL,
    entry_count             INTEGER,
    uncompressed_size_bytes BIGINT,
    module_label            TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (release_id, name)
);

CREATE INDEX IF NOT EXISTS release_artifacts_release_id_idx
    ON release_artifacts (release_id);
CREATE INDEX IF NOT EXISTS release_artifacts_kind_idx
    ON release_artifacts (release_id, kind);
