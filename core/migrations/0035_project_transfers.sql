-- Source and destination keep independent durable receipts with the same UUID.
-- Secrets belong only to an active job, or to explicit source-tail forwarding.
CREATE TABLE project_transfers (
    id uuid PRIMARY KEY,
    direction text NOT NULL CHECK (direction IN ('source','target')),
    instance_id uuid NOT NULL,
    generation bigint NOT NULL,
    source_instance_id uuid NOT NULL,
    source_generation bigint NOT NULL,
    target_instance_id uuid NOT NULL,
    target_generation bigint NOT NULL,
    actor_user_id uuid REFERENCES users(id) ON DELETE SET NULL,
    project_ids uuid[] NOT NULL CHECK (cardinality(project_ids) BETWEEN 1 AND 1000),
    project_mapping jsonb NOT NULL DEFAULT '{}',
    state text NOT NULL DEFAULT 'copying' CHECK (state IN ('copying','copied','sealed','ready','active','observing','completed','cancelled','attention')),
    version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
    latest_archive_id uuid,
    latest_manifest jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    sealed_at timestamptz,
    activated_at timestamptz,
    completed_at timestamptz,
    observed_events jsonb NOT NULL DEFAULT '{}',
    verified_events jsonb NOT NULL DEFAULT '{}',
    rollback_plan text NOT NULL DEFAULT '',
    CHECK (source_instance_id <> target_instance_id)
);

CREATE TABLE project_transfer_jobs (
    id uuid PRIMARY KEY,
    transfer_id uuid NOT NULL REFERENCES project_transfers(id) ON DELETE CASCADE,
    actor_user_id uuid REFERENCES users(id) ON DELETE SET NULL,
    command_id uuid NOT NULL UNIQUE,
    request_hash text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('export','import')),
    phase text NOT NULL CHECK (phase IN ('copy','final')),
    state text NOT NULL DEFAULT 'queued' CHECK (state IN ('queued','running','ready','failed','cancelled','unavailable')),
    key_blob bytea,
    attempt integer NOT NULL DEFAULT 0,
    cancel_requested boolean NOT NULL DEFAULT false,
    lease_owner uuid,
    lease_until timestamptz,
    artifact_id uuid,
    artifact_sha256 text,
    artifact_bytes bigint,
    manifest jsonb,
    progress jsonb NOT NULL DEFAULT '{}',
    error_code text,
    created_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    finished_at timestamptz,
    CHECK ((state IN ('queued','running')) = (key_blob IS NOT NULL))
);
CREATE UNIQUE INDEX project_transfer_jobs_one_active ON project_transfer_jobs(transfer_id) WHERE state IN ('queued','running');

CREATE TABLE project_transfer_commands (
    command_id uuid PRIMARY KEY,
    transfer_id uuid NOT NULL REFERENCES project_transfers(id) ON DELETE CASCADE,
    actor_user_id uuid REFERENCES users(id) ON DELETE SET NULL,
    request_hash text NOT NULL,
    response jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE project_transfer_routes (
    project_id uuid PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    transfer_id uuid NOT NULL REFERENCES project_transfers(id) ON DELETE CASCADE,
    target_project_id uuid NOT NULL,
    target_origin text NOT NULL,
    sealed_token bytea NOT NULL,
    enabled boolean NOT NULL DEFAULT true,
    expires_at timestamptz NOT NULL,
    forwarded bigint NOT NULL DEFAULT 0,
    failed bigint NOT NULL DEFAULT 0,
    last_seen_at timestamptz,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- Every protected write holds SHARE on its existing project row through commit.
-- Setting the fence waits for previously admitted writers. Later writes see the
-- fence, or fail serialization if their older repeatable-read snapshot cannot.
-- This also covers background SQL and in-flight handlers, not just HTTP routes.
ALTER TABLE projects ADD COLUMN migration_fence uuid REFERENCES project_transfers(id);

CREATE FUNCTION sentori_transfer_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    value jsonb;
    values_to_check jsonb[];
    project uuid;
    fence uuid;
BEGIN
    -- Only offline reset/restore set this transaction-local switch, after the
    -- exclusive server lifetime and table locks have been acquired.
    IF current_setting('sentori.offline_recovery', true) = 'on' THEN
        IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
    END IF;
    IF TG_OP = 'UPDATE' THEN values_to_check := ARRAY[to_jsonb(OLD),to_jsonb(NEW)];
    ELSIF TG_OP = 'DELETE' THEN values_to_check := ARRAY[to_jsonb(OLD)];
    ELSE values_to_check := ARRAY[to_jsonb(NEW)]; END IF;
    FOREACH value IN ARRAY values_to_check LOOP
        project := NULL; fence := NULL;
        IF TG_TABLE_NAME = 'projects' THEN
            fence := (value->>'migration_fence')::uuid;
        ELSE
            IF value ? 'project_id' THEN
                project := (value->>'project_id')::uuid;
            ELSIF value ? 'issue_id' THEN
                SELECT project_id INTO project FROM issues WHERE id=(value->>'issue_id')::uuid;
            ELSIF value ? 'release_id' THEN
                SELECT project_id INTO project FROM releases WHERE id=(value->>'release_id')::uuid;
            ELSIF value ? 'task_id' THEN
                SELECT project_id INTO project FROM tasks WHERE id=(value->>'task_id')::uuid;
            ELSE
                RAISE EXCEPTION 'transfer guard missing project scope';
            END IF;
            SELECT migration_fence INTO fence FROM projects WHERE id=project FOR SHARE;
        END IF;
        IF fence IS NOT NULL AND coalesce(current_setting('sentori.transfer_id', true),'') <> fence::text THEN
            RAISE EXCEPTION 'project is fenced for transfer' USING ERRCODE='55000';
        END IF;
    END LOOP;
    IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END $$;

DO $$
DECLARE name text;
BEGIN
    FOREACH name IN ARRAY ARRAY['projects','issues','events','issue_user_hits','sessions','issue_activity','event_attachments','releases','release_artifacts','tasks','task_events','probes','assert_stats'] LOOP
        EXECUTE format('CREATE TRIGGER transfer_write_guard BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION sentori_transfer_write_guard()', name);
    END LOOP;
END $$;
