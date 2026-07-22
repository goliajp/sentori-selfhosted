-- Sentori core migration 0012 — audit_logs query indexes (K13).
--
-- The `audit_logs` table itself shipped in K1 migration 0001 with
-- only `(project_id, created_at DESC)`. K13 surfaces operator
-- queries that filter by actor (`who did what this week`) and by
-- action (`every org.deleted in the last 24h`) — both need
-- (col, created_at DESC) for fast pagination on the dashboard.

-- "What did user X do recently?" — operator inbox for incident
-- forensics + GDPR data-subject access.
CREATE INDEX IF NOT EXISTS audit_logs_actor_created_idx
    ON audit_logs (actor_user_id, created_at DESC)
    WHERE actor_user_id IS NOT NULL;

-- "Every record of action Y" — filter dashboards (e.g. show all
-- `identity.erased` rows for compliance evidence).
CREATE INDEX IF NOT EXISTS audit_logs_action_created_idx
    ON audit_logs (action, created_at DESC);

-- "What touched (target_type, target_id)?" — used by the issue /
-- project drill-down panels. Partial because most rows have no
-- target.
CREATE INDEX IF NOT EXISTS audit_logs_target_created_idx
    ON audit_logs (target_type, target_id, created_at DESC)
    WHERE target_type IS NOT NULL;
