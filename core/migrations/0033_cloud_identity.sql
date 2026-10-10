-- Empty for self-hosted installations. A populated binding cannot silently
-- become a self-hosted password-auth database when environment variables vanish.
CREATE TABLE cloud_instance_binding (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    workspace_id uuid NOT NULL,
    control_origin text NOT NULL,
    runtime_origin text NOT NULL,
    generation bigint NOT NULL CHECK (generation > 0),
    credential_hash bytea NOT NULL CHECK (octet_length(credential_hash)=32),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE cloud_user_identities (
    subject uuid PRIMARY KEY,
    user_id uuid NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    access_epoch text NOT NULL
);
ALTER TABLE auth_sessions ADD COLUMN sealed_cloud_grant bytea;
ALTER TABLE auth_sessions ADD COLUMN cloud_generation bigint;
