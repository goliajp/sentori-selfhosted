-- Durable owner-only lifecycle commands. Recovery keys exist only while active,
-- encrypted with the separately provisioned instance key and bound to job ID.
CREATE TABLE instance_operations (
    id UUID PRIMARY KEY,
    instance_id UUID NOT NULL,
    generation BIGINT NOT NULL,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    command_id UUID NOT NULL UNIQUE,
    request_hash TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('backup','verify')),
    state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued','running','ready','failed','cancelled','unavailable')),
    key_blob BYTEA,
    cancel_requested BOOLEAN NOT NULL DEFAULT false,
    attempt INTEGER NOT NULL DEFAULT 0,
    lease_owner UUID,
    lease_until TIMESTAMPTZ,
    progress JSONB NOT NULL DEFAULT '{}',
    artifact_id UUID,
    artifact_bytes BIGINT,
    artifact_sha256 TEXT,
    manifest JSONB,
    error_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    CHECK ((state IN ('queued','running')) = (key_blob IS NOT NULL))
);
CREATE UNIQUE INDEX instance_operations_one_active ON instance_operations ((true)) WHERE state IN ('queued','running');
