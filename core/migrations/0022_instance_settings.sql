-- v5 instance identity and setup state. Existing installations retain their
-- accounts, projects and credentials; this migration only adds metadata.
CREATE TABLE instance_settings (
    singleton       BOOLEAN PRIMARY KEY DEFAULT true CHECK (singleton),
    id              UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL DEFAULT 'Sentori' CHECK (char_length(name) BETWEEN 1 AND 80),
    generation      BIGINT NOT NULL DEFAULT 1 CHECK (generation > 0),
    version         BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    initialized_at  TIMESTAMPTZ,
    last_check      JSONB,
    checked_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A v4 owner already completed bootstrap. A fresh database has no owner
-- until bootstrap runs after migrations and starts with setup incomplete.
INSERT INTO instance_settings (singleton, initialized_at)
VALUES (true, CASE WHEN EXISTS (SELECT 1 FROM users) THEN now() ELSE NULL END);
