-- A credential you can try before you trust, and a record of which
-- one sent.
--
-- Until now `push_credentials` held one row per (project, kind) and
-- the upsert said `ON CONFLICT DO UPDATE`. Pasting a key replaced the
-- working one in the same statement that saved it. If the new key was
-- wrong — and the two ways to be wrong are both invisible from the
-- file, an App Store Connect key looks exactly like an APNs key and
-- `google-services.json` looks a lot like a service account — push
-- stopped, and the row that had been working was already gone.
--
-- So: many rows per kind, exactly one of them active. A new key lands
-- beside the working one, gets asked of Apple or Google, and is
-- promoted by hand once the answer is good. Nothing about the send
-- path changes until someone decides it does.
--
-- `active` is where the partial unique index goes: one active
-- credential per (project, kind) is still the rule, it is just no
-- longer the rule about *存在*.

ALTER TABLE push_credentials
  ADD COLUMN active BOOLEAN NOT NULL DEFAULT true,
  -- What the operator calls it. Two Apple teams, or an old key kept
  -- through a rotation, are otherwise two identical rows.
  ADD COLUMN label TEXT,
  -- The vendor's own words, kept verbatim. A verdict of `limited`
  -- with no reason is a worse answer than no verdict.
  ADD COLUMN last_validate_detail TEXT;

DROP INDEX push_credentials_project_kind_idx;

CREATE UNIQUE INDEX push_credentials_active_idx
  ON push_credentials (project_id, kind)
  WHERE active;

-- `limited` is the state that was missing. A credential can be
-- perfectly valid and still unable to do the job: a service account
-- that authenticates but was never granted the messaging role, an
-- APNs key that signs but is not authorised for the topic it was
-- given. Reporting that as `rejected` sends an operator back to
-- re-download a key that was never the problem.
ALTER TABLE push_credentials
  DROP CONSTRAINT push_credentials_last_validate_status_check;

ALTER TABLE push_credentials
  ADD CONSTRAINT push_credentials_last_validate_status_check
  CHECK (last_validate_status IN
         ('ok', 'limited', 'rejected', 'malformed', 'unreachable', 'not_implemented')
         OR last_validate_status IS NULL);

COMMENT ON COLUMN push_credentials.active IS
  'Whether the send path may use this row. Exactly one per (project, kind).';
COMMENT ON COLUMN push_credentials.last_validate_status IS
  'ok = the vendor accepted it. limited = the vendor knows it but will not let it do this job. rejected = the vendor refused the credential itself.';

-- Which credential sent this. With rotation, "it stopped working at
-- 14:20" and "the key was swapped at 14:19" are the same fact, and
-- nothing recorded the second one.
ALTER TABLE push_sends ADD COLUMN credential_id uuid;

CREATE INDEX push_sends_credential_idx ON push_sends (credential_id)
  WHERE credential_id IS NOT NULL;

COMMENT ON COLUMN push_sends.credential_id IS
  'The push_credentials row that authorised this send. Null for rows written before 0017.';
