-- The Releases page asks which platforms actually reported in each release.
-- The existing project/received_at index scans the project's history once per
-- visible release; the error-only resymbolication index cannot answer for all
-- five event kinds. Keep the lookup covered without weakening that contract.
CREATE INDEX events_project_release_platform_idx ON events(project_id, release, platform);
