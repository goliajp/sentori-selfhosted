-- A project is not React Native until something says it is.
--
-- `platform` defaulted to `react-native`, so every project created
-- without an explicit one carried that badge on the projects page —
-- including the ones reporting only Swift and Kotlin events. A native
-- team read it as the product telling them whose tool this really is,
-- and they were reading a default, not a fact.
--
-- The honest value is "not stated". Events carry their own platform,
-- and that is what the dashboard should draw conclusions from.
-- Existing rows are left alone: a project that has been labelled
-- react-native may well be one, and this migration is not in a
-- position to know.

ALTER TABLE projects ALTER COLUMN platform DROP NOT NULL;
ALTER TABLE projects ALTER COLUMN platform DROP DEFAULT;
