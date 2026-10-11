-- Clients address a project. Apps are discovered from accepted telemetry and
-- native build artifacts; selecting a project platform is not evidence.
CREATE FUNCTION sentori_app_identifier(release_name TEXT, payload JSONB)
RETURNS TEXT LANGUAGE SQL IMMUTABLE PARALLEL SAFE AS $$
    SELECT coalesce((
        SELECT btrim(value) FROM (VALUES
            (1, CASE WHEN jsonb_typeof(payload #> '{app,identifier}')='string' THEN payload #>> '{app,identifier}' END),
            (2, CASE WHEN jsonb_typeof(payload #> '{app,bundleIdentifier}')='string' THEN payload #>> '{app,bundleIdentifier}' END),
            (3, CASE WHEN jsonb_typeof(payload #> '{app,packageName}')='string' THEN payload #>> '{app,packageName}' END),
            (4, CASE WHEN strpos(reverse(release_name), '@') > 0
                THEN left(release_name, length(release_name) - strpos(reverse(release_name), '@')) END)
        ) candidates(priority, value)
        WHERE length(btrim(value)) BETWEEN 1 AND 200
          AND value !~ '[[:cntrl:]]'
        ORDER BY priority LIMIT 1
    ), '')
$$;

CREATE TABLE apps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    platform TEXT NOT NULL CHECK (platform IN ('ios','android','javascript','web','weapp','unknown')),
    identifier TEXT NOT NULL DEFAULT '',
    has_explicit_identifier BOOLEAN NOT NULL DEFAULT FALSE,
    name TEXT,
    repository_url TEXT,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_event_at TIMESTAMPTZ,
    last_session_at TIMESTAMPTZ,
    UNIQUE (project_id, platform, identifier),
    UNIQUE (id, project_id)
);

-- Existing release IDs remain stable for upload URLs, jobs, and old clients.
-- An app owns its release membership. A common RN build can supply artifacts
-- to both native apps without copying or charging for the same files twice.
ALTER TABLE releases ADD CONSTRAINT releases_id_project_key UNIQUE (id, project_id);
CREATE TABLE app_releases (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    app_id UUID NOT NULL,
    release_id UUID NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ,
    PRIMARY KEY (app_id, release_id),
    FOREIGN KEY (app_id, project_id) REFERENCES apps(id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (release_id, project_id) REFERENCES releases(id, project_id) ON DELETE CASCADE
);
CREATE INDEX app_releases_release_idx ON app_releases(release_id);

ALTER TABLE events ADD COLUMN app_id UUID;
ALTER TABLE events ADD CONSTRAINT events_app_project_fk
    FOREIGN KEY (app_id, project_id) REFERENCES apps(id, project_id);
ALTER TABLE sessions ADD COLUMN app_id UUID;
ALTER TABLE sessions ADD CONSTRAINT sessions_app_project_fk
    FOREIGN KEY (app_id, project_id) REFERENCES apps(id, project_id);

WITH raw AS (
    SELECT id,project_id,platform,release,received_at,
        sentori_app_identifier('',payload) AS metadata,
        sentori_app_identifier(release,'null'::jsonb) AS namespace FROM events
), by_release AS (
    SELECT project_id,platform,release,min(metadata) AS identifier,count(DISTINCT metadata) AS identities FROM raw
    WHERE metadata<>'' AND release<>'' GROUP BY project_id,platform,release
), by_namespace AS (
    SELECT project_id,platform,namespace,min(metadata) AS identifier,count(DISTINCT metadata) AS identities FROM raw
    WHERE metadata<>'' AND namespace<>'' GROUP BY project_id,platform,namespace
), observations AS (
    SELECT e.id,e.project_id,e.platform,
        coalesce(nullif(e.metadata,''),CASE WHEN r.identities=1 THEN r.identifier END,
            CASE WHEN n.identities=1 THEN n.identifier END,
            CASE WHEN r.identities>1 OR n.identities>1 THEN '' ELSE e.namespace END) AS identifier,
        e.received_at AS at,'event' AS source,e.metadata<>'' AS explicit
    FROM raw e LEFT JOIN by_release r ON r.project_id=e.project_id AND r.platform=e.platform AND r.release=e.release
    LEFT JOIN by_namespace n ON n.project_id=e.project_id AND n.platform=e.platform AND n.namespace=e.namespace
    UNION ALL
    SELECT s.id,s.project_id,s.platform,
        coalesce(CASE WHEN r.identities=1 THEN r.identifier END,CASE WHEN n.identities=1 THEN n.identifier END,
            CASE WHEN r.identities>1 OR n.identities>1 THEN '' ELSE sentori_app_identifier(s.release,'null'::jsonb) END),
        s.received_at,'session',false FROM sessions s
    LEFT JOIN by_release r ON r.project_id=s.project_id AND r.platform=s.platform AND r.release=s.release
    LEFT JOIN by_namespace n ON n.project_id=s.project_id AND n.platform=s.platform
        AND n.namespace=sentori_app_identifier(s.release,'null'::jsonb)
)
INSERT INTO apps(project_id,platform,identifier,created_at,last_event_at,last_session_at,has_explicit_identifier)
SELECT project_id,platform,identifier,min(at),max(at) FILTER (WHERE source='event'),
    max(at) FILTER (WHERE source='session'),bool_or(explicit)
FROM observations GROUP BY project_id,platform,identifier;

WITH raw AS (
    SELECT id,project_id,platform,release,received_at,
        sentori_app_identifier('',payload) AS metadata,
        sentori_app_identifier(release,'null'::jsonb) AS namespace FROM events
), by_release AS (
    SELECT project_id,platform,release,min(metadata) AS identifier,count(DISTINCT metadata) AS identities FROM raw
    WHERE metadata<>'' AND release<>'' GROUP BY project_id,platform,release
), by_namespace AS (
    SELECT project_id,platform,namespace,min(metadata) AS identifier,count(DISTINCT metadata) AS identities FROM raw
    WHERE metadata<>'' AND namespace<>'' GROUP BY project_id,platform,namespace
), observations AS (
    SELECT e.id,e.project_id,e.platform,
        coalesce(nullif(e.metadata,''),CASE WHEN r.identities=1 THEN r.identifier END,
            CASE WHEN n.identities=1 THEN n.identifier END,
            CASE WHEN r.identities>1 OR n.identities>1 THEN '' ELSE e.namespace END) AS identifier,
        e.received_at AS at,'event' AS source,e.metadata<>'' AS explicit
    FROM raw e LEFT JOIN by_release r ON r.project_id=e.project_id AND r.platform=e.platform AND r.release=e.release
    LEFT JOIN by_namespace n ON n.project_id=e.project_id AND n.platform=e.platform AND n.namespace=e.namespace
    UNION ALL
    SELECT s.id,s.project_id,s.platform,
        coalesce(CASE WHEN r.identities=1 THEN r.identifier END,CASE WHEN n.identities=1 THEN n.identifier END,
            CASE WHEN r.identities>1 OR n.identities>1 THEN '' ELSE sentori_app_identifier(s.release,'null'::jsonb) END),
        s.received_at,'session',false FROM sessions s
    LEFT JOIN by_release r ON r.project_id=s.project_id AND r.platform=s.platform AND r.release=s.release
    LEFT JOIN by_namespace n ON n.project_id=s.project_id AND n.platform=s.platform
        AND n.namespace=sentori_app_identifier(s.release,'null'::jsonb)
), event_links AS (
    UPDATE events e SET app_id=a.id FROM observations o JOIN apps a
        ON a.project_id=o.project_id AND a.platform=o.platform AND a.identifier=o.identifier
    WHERE o.source='event' AND e.id=o.id RETURNING e.id
)
UPDATE sessions s SET app_id=a.id FROM observations o JOIN apps a
    ON a.project_id=o.project_id AND a.platform=o.platform AND a.identifier=o.identifier
WHERE o.source='session' AND s.id=o.id;

INSERT INTO releases(id,project_id,name,created_at)
SELECT gen_random_uuid(),project_id,release,min(at) FROM (
    SELECT project_id,release,received_at AS at FROM events WHERE release<>''
    UNION ALL
    SELECT project_id,release,received_at FROM sessions WHERE release<>''
) observations GROUP BY project_id,release ON CONFLICT(project_id,name) DO NOTHING;

INSERT INTO app_releases(project_id,app_id,release_id,first_seen_at,last_seen_at)
SELECT o.project_id,o.app_id,r.id,min(o.at),max(o.at) FROM (
    SELECT project_id,app_id,release,received_at AS at FROM events WHERE app_id IS NOT NULL AND release<>''
    UNION ALL
    SELECT project_id,app_id,release,received_at FROM sessions WHERE app_id IS NOT NULL AND release<>''
) o JOIN releases r ON r.project_id=o.project_id AND r.name=o.release
GROUP BY o.project_id,o.app_id,r.id;

-- Native files identify their target even before the first device reports.
-- A JavaScript map alone cannot distinguish a web app from React Native.
WITH known AS (
    SELECT a.project_id,a.platform,sentori_app_identifier(r.name,'null'::jsonb) AS namespace,
        min(a.identifier) AS identifier,count(DISTINCT a.id) AS identities
    FROM apps a JOIN app_releases ar ON ar.app_id=a.id JOIN releases r ON r.id=ar.release_id
    WHERE a.has_explicit_identifier AND sentori_app_identifier(r.name,'null'::jsonb)<>''
    GROUP BY a.project_id,a.platform,sentori_app_identifier(r.name,'null'::jsonb)
)
INSERT INTO apps(project_id,platform,identifier,created_at)
SELECT r.project_id,CASE a.kind WHEN 'dsym' THEN 'ios' ELSE 'android' END,
    CASE WHEN known.identities=1 THEN known.identifier WHEN known.identities>1 THEN '' ELSE sentori_app_identifier(r.name,'null'::jsonb) END,min(a.created_at)
FROM releases r JOIN release_artifacts a ON a.release_id=r.id
LEFT JOIN known ON known.project_id=r.project_id
    AND known.platform=CASE a.kind WHEN 'dsym' THEN 'ios' ELSE 'android' END
    AND known.namespace=sentori_app_identifier(r.name,'null'::jsonb)
WHERE a.kind IN ('dsym','proguard')
  AND NOT EXISTS (SELECT 1 FROM app_releases ar JOIN apps linked ON linked.id=ar.app_id
    WHERE ar.release_id=r.id AND linked.platform=CASE a.kind WHEN 'dsym' THEN 'ios' ELSE 'android' END)
GROUP BY r.project_id,a.kind,CASE WHEN known.identities=1 THEN known.identifier WHEN known.identities>1 THEN '' ELSE sentori_app_identifier(r.name,'null'::jsonb) END
ON CONFLICT(project_id,platform,identifier) DO NOTHING;

WITH known AS (
    SELECT a.project_id,a.platform,sentori_app_identifier(r.name,'null'::jsonb) AS namespace,
        min(a.identifier) AS identifier,count(DISTINCT a.id) AS identities
    FROM apps a JOIN app_releases ar ON ar.app_id=a.id JOIN releases r ON r.id=ar.release_id
    WHERE a.has_explicit_identifier AND sentori_app_identifier(r.name,'null'::jsonb)<>''
    GROUP BY a.project_id,a.platform,sentori_app_identifier(r.name,'null'::jsonb)
)
INSERT INTO app_releases(project_id,app_id,release_id,first_seen_at)
SELECT DISTINCT r.project_id,app.id,r.id,r.created_at
FROM releases r JOIN release_artifacts a ON a.release_id=r.id
LEFT JOIN known ON known.project_id=r.project_id
    AND known.platform=CASE a.kind WHEN 'dsym' THEN 'ios' ELSE 'android' END
    AND known.namespace=sentori_app_identifier(r.name,'null'::jsonb)
JOIN apps app ON app.project_id=r.project_id
    AND app.identifier=CASE WHEN known.identities=1 THEN known.identifier WHEN known.identities>1 THEN '' ELSE sentori_app_identifier(r.name,'null'::jsonb) END
    AND app.platform=CASE a.kind WHEN 'dsym' THEN 'ios' WHEN 'proguard' THEN 'android' END
WHERE a.kind IN ('dsym','proguard')
  AND NOT EXISTS (SELECT 1 FROM app_releases ar JOIN apps linked ON linked.id=ar.app_id
    WHERE ar.release_id=r.id AND linked.platform=app.platform)
ON CONFLICT(app_id,release_id) DO NOTHING;

CREATE INDEX events_app_received_idx ON events(project_id,app_id,received_at DESC);
CREATE INDEX sessions_app_received_idx ON sessions(project_id,app_id,received_at DESC);
