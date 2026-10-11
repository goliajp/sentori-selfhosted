//! Server-discovered applications inside a project. No app registration or
//! app-scoped credential is required on the client.

use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum Observation {
    Event,
    Session,
    Artifact,
}

struct Identity {
    identifier: String,
    fallback: String,
    explicit: bool,
}

// Discovery can refine an inferred identity while another request reports a
// session or uploads another build. Keep those decisions in one order. Normal
// observations already update the app row in their accepting transaction.
async fn lock_apps(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    platform: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('sentori-app:' || $1::uuid::text || ':' || $2,0))")
        .bind(project).bind(platform).execute(&mut **tx).await?;
    Ok(())
}

async fn release_id(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    release: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    if release.is_empty() {
        return Ok(None);
    }
    let existing = sqlx::query_scalar("SELECT id FROM releases WHERE project_id=$1 AND name=$2")
        .bind(project)
        .bind(release)
        .fetch_optional(&mut **tx)
        .await?;
    if existing.is_some() {
        return Ok(existing);
    }
    // Do not UPDATE an existing release for each observation. Besides avoiding
    // unnecessary writes, this keeps native uploads' FK locks compatible with
    // discovery. A separate SELECT sees a concurrent committed insertion.
    sqlx::query("INSERT INTO releases(id,project_id,name) VALUES(gen_random_uuid(),$1,$2) ON CONFLICT(project_id,name) DO NOTHING")
        .bind(project).bind(release).execute(&mut **tx).await?;
    sqlx::query_scalar("SELECT id FROM releases WHERE project_id=$1 AND name=$2")
        .bind(project)
        .bind(release)
        .fetch_optional(&mut **tx)
        .await
}

enum Inference {
    Known(Uuid),
    Ambiguous,
    Missing,
}

fn unambiguous(candidates: &[(Uuid, bool)]) -> Inference {
    match candidates {
        [(id, _)] | [(id, true), (_, false)] => Inference::Known(*id),
        [] => Inference::Missing,
        _ => Inference::Ambiguous,
    }
}

async fn inferred_app(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    platform: &str,
    release: Option<Uuid>,
    namespace: &str,
) -> Result<Inference, sqlx::Error> {
    let exact: Vec<(Uuid, bool)> = sqlx::query_as(
        "SELECT a.id,a.has_explicit_identifier FROM apps a JOIN app_releases ar ON ar.app_id=a.id \
         WHERE a.project_id=$1 AND a.platform=$2 AND ar.release_id=$3 ORDER BY a.has_explicit_identifier DESC LIMIT 2",
    )
    .bind(project)
    .bind(platform)
    .bind(release)
    .fetch_all(&mut **tx)
    .await?;
    if !exact.is_empty() {
        return Ok(unambiguous(&exact));
    }
    if namespace.is_empty() {
        return Ok(Inference::Missing);
    }
    let related: Vec<(Uuid, bool)> = sqlx::query_as(
        "SELECT a.id,a.has_explicit_identifier FROM apps a WHERE a.project_id=$1 AND a.platform=$2 AND EXISTS( \
           SELECT 1 FROM app_releases ar JOIN releases r ON r.id=ar.release_id \
           WHERE ar.app_id=a.id AND sentori_app_identifier(r.name,'null'::jsonb)=$3) ORDER BY a.has_explicit_identifier DESC LIMIT 2",
    )
    .bind(project)
    .bind(platform)
    .bind(namespace)
    .fetch_all(&mut **tx)
    .await?;
    Ok(unambiguous(&related))
}

async fn resolve_app(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    platform: &str,
    release: Option<Uuid>,
    identity: &mut Identity,
) -> Result<Option<Uuid>, sqlx::Error> {
    if !identity.explicit {
        match inferred_app(tx, project, platform, release, &identity.fallback).await? {
            Inference::Known(id) => return Ok(Some(id)),
            Inference::Ambiguous => identity.identifier.clear(),
            Inference::Missing => {}
        }
    }
    let existing = sqlx::query_scalar(
        "SELECT id FROM apps WHERE project_id=$1 AND platform=$2 AND identifier=$3",
    )
    .bind(project)
    .bind(platform)
    .bind(&identity.identifier)
    .fetch_optional(&mut **tx)
    .await?;
    if existing.is_some() || !identity.explicit {
        return Ok(existing);
    }
    // A build or session can arrive before explicit package metadata. Refine
    // its inferred name while preserving the app ID, label and repository.
    // An unnamed platform group spanning several builds remains ambiguous.
    sqlx::query_scalar(
        "UPDATE apps a SET identifier=$4 WHERE project_id=$1 AND platform=$2 AND identifier=$3 \
         AND NOT has_explicit_identifier AND ($3<>'' OR (SELECT count(*) FROM app_releases ar WHERE ar.app_id=a.id)=1) AND EXISTS( \
           SELECT 1 FROM app_releases ar WHERE ar.app_id=a.id AND ar.release_id=$5) \
         AND NOT EXISTS(SELECT 1 FROM apps known JOIN app_releases ar ON ar.app_id=known.id \
           JOIN releases r ON r.id=ar.release_id WHERE known.project_id=$1 AND known.platform=$2 \
             AND known.has_explicit_identifier AND (ar.release_id=$5 \
               OR $3<>'' AND sentori_app_identifier(r.name,'null'::jsonb)=$3)) RETURNING id",
    )
    .bind(project)
    .bind(platform)
    .bind(&identity.fallback)
    .bind(&identity.identifier)
    .bind(release)
    .fetch_optional(&mut **tx)
    .await
}

async fn link_release(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    app: Uuid,
    release: Uuid,
    telemetry: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_releases(project_id,app_id,release_id,last_seen_at) \
         VALUES($1,$2,$3,CASE WHEN $4 THEN now() END) ON CONFLICT(app_id,release_id) \
         DO UPDATE SET last_seen_at=greatest(EXCLUDED.last_seen_at,app_releases.last_seen_at)",
    )
    .bind(project)
    .bind(app)
    .bind(release)
    .bind(telemetry)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn merge_inferred_app(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    platform: &str,
    fallback: &str,
    release: Uuid,
    app: Uuid,
) -> Result<(), sqlx::Error> {
    // A known package may use a new build namespace. Merge inferred identities
    // only; two explicit package identifiers must never collapse. Conflicting
    // operator metadata and ambiguous unnamed groups remain separate.
    let alias: Option<Uuid> = sqlx::query_scalar(
        "SELECT a.id FROM apps a JOIN apps target ON target.id=$4 \
         WHERE a.project_id=$1 AND a.platform=$2 AND a.identifier=$3 AND a.id<>$4 \
           AND NOT a.has_explicit_identifier \
           AND ($3<>'' OR (SELECT count(*) FROM app_releases ar WHERE ar.app_id=a.id)=1) \
           AND (a.name IS NULL OR target.name IS NULL OR a.name=target.name) \
           AND (a.repository_url IS NULL OR target.repository_url IS NULL OR a.repository_url=target.repository_url) \
           AND EXISTS(SELECT 1 FROM app_releases ar WHERE ar.app_id=a.id AND ar.release_id=$5) \
           AND NOT EXISTS(SELECT 1 FROM apps known JOIN app_releases ar ON ar.app_id=known.id \
             JOIN releases r ON r.id=ar.release_id WHERE known.project_id=$1 AND known.platform=$2 \
               AND known.has_explicit_identifier AND known.id<>$4 AND (ar.release_id=$5 \
                 OR $3<>'' AND sentori_app_identifier(r.name,'null'::jsonb)=$3)) \
         FOR UPDATE OF a,target",
    ).bind(project).bind(platform).bind(fallback).bind(app).bind(release)
        .fetch_optional(&mut **tx).await?;
    let Some(alias) = alias else { return Ok(()) };
    sqlx::query(
        "UPDATE apps t SET name=coalesce(t.name,a.name),repository_url=coalesce(t.repository_url,a.repository_url), \
           created_at=least(t.created_at,a.created_at), \
           last_event_at=greatest(t.last_event_at,a.last_event_at),last_session_at=greatest(t.last_session_at,a.last_session_at),version=CASE \
             WHEN t.name IS NULL AND a.name IS NOT NULL OR t.repository_url IS NULL AND a.repository_url IS NOT NULL \
             THEN greatest(t.version,a.version)+1 ELSE t.version END FROM apps a WHERE t.id=$1 AND a.id=$2",
    ).bind(app).bind(alias).execute(&mut **tx).await?;
    sqlx::query(
        "INSERT INTO app_releases(project_id,app_id,release_id,first_seen_at,last_seen_at) \
         SELECT project_id,$1,release_id,first_seen_at,last_seen_at FROM app_releases WHERE app_id=$2 \
         ON CONFLICT(app_id,release_id) DO UPDATE SET \
           first_seen_at=least(app_releases.first_seen_at,EXCLUDED.first_seen_at), \
           last_seen_at=greatest(app_releases.last_seen_at,EXCLUDED.last_seen_at)",
    ).bind(app).bind(alias).execute(&mut **tx).await?;
    sqlx::query("UPDATE events SET app_id=$1 WHERE app_id=$2")
        .bind(app)
        .bind(alias)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE sessions SET app_id=$1 WHERE app_id=$2")
        .bind(app)
        .bind(alias)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM apps WHERE id=$1")
        .bind(alias)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Called after duplicate detection, in the transaction accepting the data.
/// A rejected event or retry must not invent an application.
pub async fn observe(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    platform: &str,
    release: &str,
    payload: &Value,
    source: Observation,
) -> Result<Uuid, sqlx::Error> {
    lock_apps(tx, project_id, platform).await?;
    let version_id = release_id(tx, project_id, release).await?;
    let (identifier, fallback, explicit): (String, String, bool) = sqlx::query_as(
        "SELECT sentori_app_identifier($1,$2),sentori_app_identifier($1,'null'::jsonb),sentori_app_identifier('',$2)<>''",
    ).bind(release).bind(payload).fetch_one(&mut **tx).await?;
    let mut identity = Identity {
        identifier,
        fallback,
        explicit,
    };
    let existing = resolve_app(tx, project_id, platform, version_id, &mut identity).await?;
    let event = matches!(source, Observation::Event);
    let session = matches!(source, Observation::Session);
    let app_id = if let Some(id) = existing {
        sqlx::query("UPDATE apps SET last_event_at=CASE WHEN $2 THEN greatest(now(),last_event_at) ELSE last_event_at END,last_session_at=CASE WHEN $3 THEN greatest(now(),last_session_at) ELSE last_session_at END,has_explicit_identifier=has_explicit_identifier OR $4 WHERE id=$1")
            .bind(id).bind(event).bind(session).bind(identity.explicit).execute(&mut **tx).await?;
        id
    } else {
        sqlx::query_scalar(
            "INSERT INTO apps(project_id,platform,identifier,last_event_at,last_session_at,has_explicit_identifier) \
             VALUES($1,$2,$3,CASE WHEN $4 THEN now() END,CASE WHEN $5 THEN now() END,$6) RETURNING id",
        )
        .bind(project_id)
        .bind(platform)
        .bind(&identity.identifier)
        .bind(event)
        .bind(session)
        .bind(identity.explicit)
        .fetch_one(&mut **tx)
        .await?
    };
    if let Some(release) = version_id {
        link_release(tx, project_id, app_id, release, event || session).await?;
        if identity.explicit {
            merge_inferred_app(
                tx,
                project_id,
                platform,
                &identity.fallback,
                release,
                app_id,
            )
            .await?;
        }
    }
    Ok(app_id)
}

/// Native formats identify their platform. Generic JS/source files remain
/// build evidence until telemetry tells us which apps use the build.
pub async fn observe_artifact(
    tx: &mut Transaction<'_, Postgres>,
    release_id: Uuid,
    kind: &str,
) -> Result<(), sqlx::Error> {
    let platform = match kind {
        "dsym" => "ios",
        "proguard" => "android",
        _ => return Ok(()),
    };
    let (project_id, release): (Uuid, String) =
        sqlx::query_as("SELECT project_id,name FROM releases WHERE id=$1")
            .bind(release_id)
            .fetch_one(&mut **tx)
            .await?;
    lock_apps(tx, project_id, platform).await?;
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_releases ar JOIN apps a ON a.id=ar.app_id \
         WHERE ar.release_id=$1 AND a.platform=$2)",
    )
    .bind(release_id)
    .bind(platform)
    .fetch_one(&mut **tx)
    .await?;
    if !known {
        observe(
            tx,
            project_id,
            platform,
            &release,
            &Value::Null,
            Observation::Artifact,
        )
        .await?;
    }
    Ok(())
}
