-- Additive schema; historical source IPs are unknown.
ALTER TABLE audit_logs ADD COLUMN IF NOT EXISTS ip text;
