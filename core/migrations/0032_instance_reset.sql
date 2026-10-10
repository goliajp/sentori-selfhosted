-- Reset preparation grants no online destructive authority. The offline tool
-- rechecks the live session, role, password and instance before making a backup.
CREATE TABLE instance_reset_intents (
    id UUID PRIMARY KEY,
    instance_id UUID NOT NULL,
    generation BIGINT NOT NULL,
    instance_name TEXT NOT NULL,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_hash BYTEA NOT NULL REFERENCES auth_sessions(id_hash) ON DELETE CASCADE,
    password_fingerprint TEXT NOT NULL,
    permit_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '10 minutes')
);
-- Set in the same transaction that clears the rows. Startup must fail closed
-- until the offline tool has removed the old blob bytes and persisted a receipt.
ALTER TABLE instance_settings ADD COLUMN reset_pending JSONB;
