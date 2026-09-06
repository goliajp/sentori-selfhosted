-- One identity for a device, the same one every event carries.
--
-- `device_tokens` had `user_fingerprint_hex bytea` that nothing ever
-- wrote or read, and `push_tokens.app_user_id text` — a free string
-- the host sets to whatever it likes. Neither can answer the question
-- push exists inside this product to answer: *notify the people who
-- hit this issue*. That needs the device to carry the same key the
-- events do.
--
-- `events.user_key` is text: the salted hash of a user's id or email,
-- computed on the device so a raw identity never reaches the server.
-- Matching its type exactly means the audience query is a plain join
-- with no encoding step in the middle to get wrong.
--
-- The bytea column goes rather than staying beside the new one. Two
-- columns that almost mean the same thing is how the next person
-- picks the wrong one.

ALTER TABLE device_tokens DROP COLUMN IF EXISTS user_fingerprint_hex;
ALTER TABLE device_tokens ADD COLUMN user_key text;

-- The audience lookup: live devices for a set of user keys.
CREATE INDEX idx_device_tokens_user_key ON device_tokens (project_id, user_key)
  WHERE user_key IS NOT NULL AND revoked_at IS NULL;

COMMENT ON COLUMN device_tokens.user_key IS
  'Salted identity hash, same value and type as events.user_key; NULL when the app registered a device before calling sentori.user()';
