-- Sentori v1 migration 0005 — releases, symbolication artifacts,
-- probes, assert stats.
--
-- release_artifacts holds all three symbolication inputs under one
-- roof (sourcemap / dsym / proguard) instead of the v0.2 trio of
-- release_artifacts + dsyms + proguard_mappings tables. Bytes go to
-- the blob store; meta carries the kind-specific lookup keys (dSYM
-- debug_id + arch, proguard debug_id, sourcemap bundle name). A
-- late upload triggers retro-symbolication backfill over stored
-- events (design.md §6 — "allowed to fail" upload without backfill
-- would be "allowed to lose forever").
--
-- probes is the tripwire registry (design.md §2). Rows are born two
-- ways: the CLI's static scan registers every sentori.probe(ref) it
-- finds at release-upload time (so a silent probe is visibly alive,
-- distinguishable from deleted code), and ingest upserts on first
-- fire for probes the scan never saw. issue_id links the probe to
-- the issue it guards; firing flips that issue to regressed.
--
-- assert_stats is the liveness ledger for production assertions:
-- passes are counted client-side and piggybacked in aggregate (no
-- per-pass events — that would be the heartbeat flood again), fails
-- arrive as real events. Together: "ran 45k times, failed 3".

CREATE TABLE releases (
    id         UUID        PRIMARY KEY,
    project_id UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, name)
);
CREATE INDEX releases_project_created_idx
    ON releases (project_id, created_at DESC);

CREATE TABLE release_artifacts (
    id           UUID        PRIMARY KEY,
    release_id   UUID        NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    kind         TEXT        NOT NULL CHECK (kind IN ('sourcemap', 'dsym', 'proguard')),
    name         TEXT        NOT NULL,
    content_hash TEXT        NOT NULL,
    size_bytes   BIGINT,
    meta         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (release_id, kind, name)
);
CREATE INDEX release_artifacts_release_kind_idx
    ON release_artifacts (release_id, kind);

CREATE TABLE probes (
    id                       UUID        PRIMARY KEY,
    project_id               UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ref                      TEXT        NOT NULL,
    issue_id                 UUID        REFERENCES issues(id) ON DELETE SET NULL,
    first_registered_release TEXT,
    last_seen_release        TEXT,
    registered_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_fired_at            TIMESTAMPTZ,
    fire_count               BIGINT      NOT NULL DEFAULT 0,
    UNIQUE (project_id, ref)
);

CREATE TABLE assert_stats (
    project_id   UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    release      TEXT        NOT NULL DEFAULT '',
    pass_count   BIGINT      NOT NULL DEFAULT 0,
    fail_count   BIGINT      NOT NULL DEFAULT 0,
    last_pass_at TIMESTAMPTZ,
    last_fail_at TIMESTAMPTZ,
    PRIMARY KEY (project_id, name, release)
);
