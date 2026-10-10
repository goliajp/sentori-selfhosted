-- Durable, bounded background commands. Payloads contain identifiers, never credentials.
CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    generation BIGINT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('resymbolicate')),
    release_id UUID NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued','running','retry_wait','succeeded','failed','cancelled')),
    attempt INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts BETWEEN 1 AND 10),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner UUID,
    lease_until TIMESTAMPTZ,
    cancel_requested BOOLEAN NOT NULL DEFAULT false,
    scanned INTEGER NOT NULL DEFAULT 0 CHECK (scanned BETWEEN 0 AND 20000),
    rewritten INTEGER NOT NULL DEFAULT 0,
    frames INTEGER NOT NULL DEFAULT 0,
    cursor_at TIMESTAMPTZ,
    cursor_id UUID,
    error_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    CHECK ((state = 'running') = (lease_owner IS NOT NULL AND lease_until IS NOT NULL))
);
CREATE INDEX jobs_dispatch_idx ON jobs (next_attempt_at, created_at) WHERE state IN ('queued','running','retry_wait');
CREATE INDEX jobs_project_idx ON jobs (project_id, created_at DESC, id DESC);
CREATE TABLE idempotency_requests (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    subject TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    request_digest TEXT NOT NULL,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, subject, key_hash)
);
-- job.ready is consumed transactionally by the worker when first claiming the job.
-- Retries reuse the durable job and do not republish this initial dispatch event.
CREATE TABLE outbox_events (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL UNIQUE REFERENCES jobs(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL CHECK (event_type = 'job.ready'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ
);
CREATE INDEX outbox_pending_idx ON outbox_events (created_at) WHERE delivered_at IS NULL;
CREATE INDEX events_resymbolicate_idx ON events (project_id, release, occurred_at DESC, id DESC) WHERE kind = 'error';
