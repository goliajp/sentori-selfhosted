-- `idempotencyKey` is documented as a dedup key and could not work.
--
-- One send to N devices writes N rows in `push_sends`, all carrying
-- the caller's key. The unique index was on `(project_id,
-- idempotency_key)`, so the second row collided with the first: the
-- field only ever worked for an audience of exactly one, and for any
-- larger one it took the endpoint down with it. Measured against a
-- real database: a keyed send to two devices answered 500.
--
-- The grain the word actually means is one delivery per device per
-- key. That is this index. It is strictly more permissive than the
-- one it replaces, so no existing row can violate it.

DROP INDEX IF EXISTS push_sends_idempotency_idx;

CREATE UNIQUE INDEX push_sends_idempotency_idx
    ON push_sends (project_id, token_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

COMMENT ON INDEX push_sends_idempotency_idx IS
  'One delivery per device per caller-supplied key. Retrying a send with the same key adds nothing rather than sending twice.';
