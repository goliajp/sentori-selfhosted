-- Preserve existing project data while allowing v5 editors to reject stale writes.
ALTER TABLE projects ADD COLUMN version BIGINT NOT NULL DEFAULT 1;
