-- Sentori core migration 0004 — issue-store triage augmentation (K5).
--
-- K4 owned the ingest-time issues schema (id / project_id /
-- fingerprint / error_type / message_sample / kind / status /
-- counts / regression columns). K5 layers on the **operator-
-- triage** axes the dashboard needs but that the ingest path
-- never writes:
--
--   - assignee_user_id — who's looking at this
--   - priority         — p0..p3 triage axis
--   - labels           — operator-typed tags
--   - resolved_in_release — release the operator marked
--                         resolved against (companion to
--                         resolved_at from K4)
--
-- All four columns are nullable / default-able so existing K4
-- rows (none in fresh-start v0.1, but pattern matters for the
-- legacy → v0.1 migration tool) survive without backfill.

ALTER TABLE issues
    ADD COLUMN IF NOT EXISTS assignee_user_id UUID
        REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE issues
    ADD COLUMN IF NOT EXISTS priority TEXT NOT NULL DEFAULT 'p3'
        CHECK (priority IN ('p0', 'p1', 'p2', 'p3'));

ALTER TABLE issues
    ADD COLUMN IF NOT EXISTS labels TEXT[] NOT NULL DEFAULT '{}';

ALTER TABLE issues
    ADD COLUMN IF NOT EXISTS resolved_in_release TEXT;

-- Operator filter: "show me p0/p1 only".
CREATE INDEX IF NOT EXISTS issues_project_priority_idx
    ON issues (project_id, priority);

-- Operator filter: "show me everything assigned to user X".
CREATE INDEX IF NOT EXISTS issues_assignee_idx
    ON issues (assignee_user_id)
    WHERE assignee_user_id IS NOT NULL;

-- Operator filter: "issues tagged 'crash'".
CREATE INDEX IF NOT EXISTS issues_labels_gin_idx
    ON issues USING GIN (labels);
