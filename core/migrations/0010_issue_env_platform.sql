-- Issues split by deployment environment and platform (user ruling
-- 2026-08-02: a staging occurrence of the "same" error is not the
-- same case as the production one, and an iOS error is not the
-- Android one — their fingerprints now include both dimensions).
--
-- Historical issues aggregated across both; their columns stay NULL
-- ("mixed, pre-split") and they age out naturally — new events open
-- new, correctly-split issues. Release deliberately stays OUT of the
-- fingerprint: resolve anchors on a release and only a recurrence in
-- that release or newer reopens — the cross-release narrative is the
-- point.

ALTER TABLE issues ADD COLUMN environment text;
ALTER TABLE issues ADD COLUMN platform text;

CREATE INDEX idx_issues_environment ON issues (project_id, environment)
  WHERE environment IS NOT NULL;
