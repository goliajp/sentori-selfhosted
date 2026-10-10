-- A restored database starts with every automatic worker paused. Verification
-- can read replays and ingest test events before an explicit offline resume.
ALTER TABLE instance_settings ADD COLUMN operating_mode TEXT NOT NULL DEFAULT 'normal'
    CHECK (operating_mode IN ('normal','restore_validation'));
ALTER TABLE instance_settings ADD COLUMN restored_from UUID;
ALTER TABLE instance_settings ADD COLUMN restored_source_id UUID;
ALTER TABLE instance_settings ADD COLUMN restored_at TIMESTAMPTZ;
