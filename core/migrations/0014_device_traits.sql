-- Targeting by who someone is, rather than by which device they hold.
--
-- `device_tokens.metadata` already carries facts about the *device* —
-- app version, locale, build channel — sent by `register()`. Traits
-- are facts about the *person*: plan, org, cohort. Two writers, two
-- lifetimes: metadata changes when the app is rebuilt, traits change
-- when the account does. Folding them into one column would make
-- "which of these did the host actually tell us" unanswerable, and a
-- send aimed at `plan = pro` would silently match a device that
-- happened to report a build channel called `pro`.
--
-- Stored raw, unlike `user_key`, which is a hash. That is the whole
-- point of the pair: the identity stays unreadable here, and the
-- attributes a campaign selects on stay readable. A trait is what the
-- host chose to hand us for targeting; an identity is not.

ALTER TABLE device_tokens ADD COLUMN traits jsonb NOT NULL DEFAULT '{}'::jsonb;

COMMENT ON COLUMN device_tokens.traits IS
  'Host-supplied attributes of the person on this device, from sentori.user(). Raw, not hashed — selecting on them is the purpose. Device-side facts live in metadata.';

-- Containment (`@>`) is the operator equality conditions compile to,
-- and jsonb_path_ops indexes it in about half the space of the default
-- opclass. Nothing here needs the key-existence operators the default
-- opclass adds.
CREATE INDEX idx_device_tokens_traits ON device_tokens
  USING gin (traits jsonb_path_ops)
  WHERE revoked_at IS NULL;

-- Versions do not compare as text.
--
-- '4.10.0' < '4.2.0' lexically, which is backwards, and it is backwards
-- exactly at the moment a project ships its tenth minor release — so
-- the bug arrives late, in production, aimed at the users a fix was
-- meant to reach. Segments compare as numbers or the comparison is
-- wrong.
--
-- Padding to four segments makes '4.2' and '4.2.0' the same version,
-- which is what semver says they are. A build suffix ('-beta.1',
-- '+2261') is dropped rather than ordered: ordering prereleases needs
-- the rest of the semver rules, and half of those rules is worse than
-- none. NULL for anything unparseable, so every comparison against it
-- is NULL and the row is left out rather than swept in.
CREATE OR REPLACE FUNCTION sentori_version_key(v text)
  RETURNS bigint[] LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT CASE WHEN coalesce(v, '') ~ '^v?[0-9]+(\.[0-9]+)*'
      THEN (string_to_array(
              regexp_replace(v, '^v?([0-9]+(\.[0-9]+)*).*$', '\1'), '.'
            )::bigint[] || ARRAY[0, 0, 0, 0]::bigint[])[1:4]
      ELSE NULL
    END
  $$;

COMMENT ON FUNCTION sentori_version_key(text) IS
  'Comparable form of a version string: four numeric segments, build suffix dropped, NULL when unparseable.';
