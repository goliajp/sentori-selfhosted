CREATE TABLE cloud_entitlement (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    active boolean NOT NULL,
    limits jsonb NOT NULL,
    issued_at timestamptz NOT NULL,
    valid_until timestamptz NOT NULL,
    paid_until timestamptz,
    checked_at timestamptz NOT NULL DEFAULT now()
);
-- Calendar months are UTC, independently of monthly/yearly payment cadence.
CREATE TABLE cloud_monthly_usage (
    month date PRIMARY KEY,
    events bigint NOT NULL DEFAULT 0 CHECK (events>=0),
    replays bigint NOT NULL DEFAULT 0 CHECK (replays>=0),
    ingest_bytes bigint NOT NULL DEFAULT 0 CHECK (ingest_bytes>=0)
);
CREATE TABLE cloud_second_usage (
    second timestamptz PRIMARY KEY,
    events bigint NOT NULL CHECK (events>=0)
);
CREATE TABLE cloud_replay_usage (
    project_id uuid NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(project_id,event_id)
);
CREATE INDEX cloud_replay_usage_created ON cloud_replay_usage(created_at);
CREATE INDEX sessions_received ON sessions USING BRIN(received_at);
