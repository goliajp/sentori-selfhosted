-- One id for one call to `POST /v1/push/send`.
--
-- A send to a hundred and twenty-eight devices wrote a hundred and
-- twenty-eight rows and handed the caller a hundred and twenty-eight
-- ids. Answering "did it go out" meant polling all of them, and a send
-- to a hundred thousand meant a response body that was three megabytes
-- of uuid before it meant anything else.
--
-- The call is the thing an integrator has. It gets an id, the rows
-- carry it, and the aggregate is one grouped read on this index.
--
-- Nullable, because every row written before this exists and no row
-- written before this belongs to a batch. `campaign_id` is not this:
-- that is a label the caller invents and may reuse across calls.

ALTER TABLE push_sends ADD COLUMN batch_id uuid;

CREATE INDEX push_sends_batch_idx ON push_sends (project_id, batch_id)
  WHERE batch_id IS NOT NULL;

COMMENT ON COLUMN push_sends.batch_id IS
  'The one call that produced this row. Returned as sendId, and what GET /v1/push/sends/{sendId} groups on.';
