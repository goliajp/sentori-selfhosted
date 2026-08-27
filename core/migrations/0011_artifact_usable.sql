-- Whether an uploaded artifact is one the symbolicator can actually
-- read, decided once at upload rather than re-derived on every read
-- (a React Native source map is tens of megabytes; parsing one to
-- answer "is it fine?" is not a question you ask on a list endpoint).
--
-- insight uploaded the Hermes bytecode *bundle* under
-- `kind=sourcemap`, twice, on two releases. It stored, it listed, it
-- showed a green light on the releases page, and it could never
-- symbolicate anything. Nothing said so for months, because nothing
-- ever looked at the bytes again until a crash needed them — and by
-- then the only reader was a server log.
--
-- NULL means "uploaded before this column existed, never checked".
-- Distinct from false on purpose: an old artifact nobody has verified
-- is not the same claim as one that has been read and rejected.

ALTER TABLE release_artifacts ADD COLUMN usable boolean;

COMMENT ON COLUMN release_artifacts.usable IS
  'true = parsed at upload; false = stored but unreadable; NULL = never checked';
