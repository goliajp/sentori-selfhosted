-- Sentori v1 migration 0008 — visual replay + native source bundles.
--
-- `screens` — the visual-replay attachment: NDJSON of low-bitrate
-- screenshot frames covering the window before an error/warn.
-- `sessionTrail` was in the server's accept list since v1 but never
-- in this CHECK, so any upload died at the INSERT; aligned while
-- the constraint is open anyway.
--
-- `srcbundle` — a release artifact carrying native source files
-- (path → content, JSON), uploaded by the build so dSYM/proguard
-- frames can show the failing line without the server ever
-- touching a repository. The JS equivalent rides inside the
-- sourcemap (sourcesContent); native has no such carrier.

ALTER TABLE event_attachments DROP CONSTRAINT event_attachments_kind_check;
ALTER TABLE event_attachments ADD CONSTRAINT event_attachments_kind_check
    CHECK (kind IN ('replay', 'screens', 'screenshot', 'viewTree',
                    'stateSnapshot', 'logTail', 'sessionTrail'));

ALTER TABLE release_artifacts DROP CONSTRAINT release_artifacts_kind_check;
ALTER TABLE release_artifacts ADD CONSTRAINT release_artifacts_kind_check
    CHECK (kind IN ('sourcemap', 'dsym', 'proguard', 'srcbundle'));
