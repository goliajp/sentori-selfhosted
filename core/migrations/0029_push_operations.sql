ALTER TABLE push_credentials ADD COLUMN secret_format TEXT NOT NULL DEFAULT 'legacy'
    CHECK (secret_format IN ('legacy','sealed'));
ALTER TABLE push_credentials ADD COLUMN version BIGINT NOT NULL DEFAULT 1;
ALTER TABLE device_tokens ADD COLUMN version BIGINT NOT NULL DEFAULT 1;
ALTER TABLE push_sends ADD COLUMN version BIGINT NOT NULL DEFAULT 1;
ALTER TABLE push_sends ADD COLUMN generation BIGINT NOT NULL DEFAULT 1;
ALTER TABLE push_sends ADD COLUMN lease_owner UUID;
ALTER TABLE push_sends ADD COLUMN lease_until TIMESTAMPTZ;
ALTER TABLE push_sends ADD CONSTRAINT push_sends_lease_check
    CHECK ((lease_owner IS NULL) = (lease_until IS NULL));
UPDATE push_sends SET generation=(SELECT generation FROM instance_settings WHERE singleton);

CREATE FUNCTION push_row_version() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN NEW.version=OLD.version+1; RETURN NEW; END $$;
CREATE TRIGGER push_credential_version BEFORE UPDATE ON push_credentials
    FOR EACH ROW EXECUTE FUNCTION push_row_version();
CREATE TRIGGER push_device_version BEFORE UPDATE ON device_tokens
    FOR EACH ROW EXECUTE FUNCTION push_row_version();
CREATE TRIGGER push_send_version BEFORE UPDATE ON push_sends
    FOR EACH ROW EXECUTE FUNCTION push_row_version();
CREATE FUNCTION push_send_generation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN NEW.generation=(SELECT generation FROM instance_settings WHERE singleton); RETURN NEW; END $$;
CREATE TRIGGER push_send_generation BEFORE INSERT ON push_sends
    FOR EACH ROW EXECUTE FUNCTION push_send_generation();

CREATE TABLE push_commands (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    command_id TEXT NOT NULL CHECK (length(command_id) BETWEEN 1 AND 128),
    digest TEXT NOT NULL,
    response JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(project_id,actor_user_id,command_id)
);
CREATE TABLE push_audience_previews (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    digest TEXT NOT NULL,
    generation BIGINT NOT NULL,
    token_ids UUID[] NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT now()+interval '10 minutes',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX push_preview_expiry_idx ON push_audience_previews(expires_at);

CREATE INDEX push_sends_project_page_idx ON push_sends(project_id,created_at DESC,id DESC);
