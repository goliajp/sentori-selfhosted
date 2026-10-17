-- Product role is independent of the telemetry runtime. In particular,
-- JavaScript alone is not evidence that an app is a browser or an API.
ALTER TABLE apps ADD COLUMN kind TEXT CHECK (
    kind IN ('ios','android','webapi','webapp','weapp','unknown')
);
UPDATE apps SET kind=CASE platform
    WHEN 'ios' THEN 'ios' WHEN 'android' THEN 'android'
    WHEN 'web' THEN 'webapp' WHEN 'weapp' THEN 'weapp'
    ELSE 'unknown' END;
