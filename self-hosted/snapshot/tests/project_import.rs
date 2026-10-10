//! Invoked by project_import.py against its disposable PostgreSQL container.
use anyhow::{Context, ensure};
use sentori_snapshot::{
    RecoveryKey,
    archive::{TransferBinding, TransferPhase},
    capture::{CaptureControl, capture_projects, capture_transfer},
    project_import::{PreparedImport, mapped_id},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, PgPool, postgres::PgConnectOptions};
use std::{path::Path, str::FromStr};
use uuid::Uuid;

async fn database(admin: &PgPool, options: &PgConnectOptions) -> anyhow::Result<PgPool> {
    let name = format!("transfer_{}", Uuid::new_v4().simple());
    // Audited: generated identifier is a fixed prefix plus hexadecimal UUID.
    sqlx::query(AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(admin)
        .await?;
    let pool = PgPool::connect_with(options.clone().database(&name)).await?;
    sqlx::migrate!("../../core/migrations").run(&pool).await?;
    Ok(pool)
}

fn blob(root: &Path, bytes: &[u8]) -> anyhow::Result<String> {
    let hash = hex::encode(Sha256::digest(bytes));
    std::fs::create_dir_all(root.join(&hash[..2]))?;
    std::fs::write(
        root.join(&hash[..2]).join(format!("{}.bin", &hash[2..])),
        bytes,
    )?;
    Ok(hash)
}

#[tokio::test]
#[ignore = "run python3 self-hosted/snapshot/tests/project_import.py with an isolated PostgreSQL container"]
async fn verified_import_reconciles_content_without_credentials() -> anyhow::Result<()> {
    let options = PgConnectOptions::from_str(
        &std::env::var("SENTORI_IMPORT_TEST_DATABASE_URL")
            .context("isolated import fixture URL required")?,
    )?;
    let admin = PgPool::connect_with(options.clone()).await?;
    let source = database(&admin, &options).await?;
    let target = database(&admin, &options).await?;
    let root = tempfile::tempdir()?;
    let source_blobs = root.path().join("source");
    let target_blobs = root.path().join("target");
    std::fs::create_dir_all(&source_blobs)?;
    std::fs::create_dir_all(&target_blobs)?;
    let project = Uuid::new_v4();
    let foreign = Uuid::new_v4();
    let user = Uuid::new_v4();
    let issue = Uuid::new_v4();
    let event = Uuid::new_v4();
    let task = Uuid::new_v4();
    let reference = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name) VALUES($1,'Move me'),($2,'Other source project')")
        .bind(project)
        .bind(foreign)
        .execute(&source)
        .await?;
    sqlx::query("INSERT INTO users(id,email,password_hash,role,display_name) VALUES($1,'private@example.test','source-secret','superadmin','Historical author')")
        .bind(user).execute(&source).await?;
    sqlx::query("INSERT INTO tokens(id,project_id,name,scope,token_hash) VALUES($1,$2,'Do not copy','ingest','source-token-secret')")
        .bind(Uuid::new_v4()).bind(project).execute(&source).await?;
    sqlx::query("INSERT INTO issues(id,project_id,fingerprint,kind,group_title,first_seen,last_seen,event_count,assignee_user_id) VALUES($1,$2,'fixture','error','History',now(),now(),1,$3)")
        .bind(issue).bind(project).bind(user).execute(&source).await?;
    sqlx::query("INSERT INTO events(id,project_id,issue_id,kind,platform,occurred_at,payload) VALUES($1,$2,$3,'error','javascript',now(),$4)")
        .bind(event).bind(project).bind(issue).bind(json!({"message":"actual bytes 中文","nested":{"id":event}})).execute(&source).await?;
    sqlx::query("INSERT INTO tasks(id,project_id,issue_id,created_by,title,kind,priority,automation,verified) VALUES($1,$2,$3,$4,'Preserve human content','bug','normal','{\"private\":true}',true)")
        .bind(task).bind(project).bind(issue).bind(user).execute(&source).await?;
    sqlx::query("INSERT INTO task_events(id,task_id,actor_user_id,action,body,category,notify_user_id) VALUES($1,$2,$3,'comment','Author text','mention',$3)")
        .bind(Uuid::new_v4()).bind(task).bind(user).execute(&source).await?;
    let bytes = b"{\"type\":\"frame\",\"tree\":[]}\n";
    let hash = blob(&source_blobs, bytes)?;
    sqlx::query("INSERT INTO event_attachments(ref,project_id,event_id,kind,media_type,size_bytes,blob_hash,source,captured_at) VALUES($1,$2,$3,'replay','application/x-ndjson',$4,$5,'javascript',now())")
        .bind(reference).bind(project).bind(event).bind(i32::try_from(bytes.len())?).bind(&hash).execute(&source).await?;
    let unrelated = Uuid::new_v4();
    sqlx::query("INSERT INTO projects(id,name) VALUES($1,'Unrelated target')")
        .bind(unrelated)
        .execute(&target)
        .await?;
    let archive = root.path().join("copy.sentori");
    let key = RecoveryKey::from_bytes([91; 32]);
    let control = CaptureControl::default();
    capture_projects(
        &source,
        &source_blobs,
        &archive,
        key.clone(),
        "5.0.0",
        &[project],
        &control,
    )
    .await?;
    let transfer = Uuid::new_v4();
    let destination = mapped_id(transfer, "projects", project);
    let mut prepared = PreparedImport::open(&archive, key.clone(), &control)?;
    ensure!(
        prepared.manifest.tables["users"].rows == 1,
        "author metadata missing"
    );
    ensure!(
        PreparedImport::open(&archive, RecoveryKey::from_bytes([92; 32]), &control).is_err(),
        "wrong key accepted"
    );

    // A trigger changing a valid row must fail readback and roll back all rows.
    sqlx::raw_sql("CREATE FUNCTION alter_transfer() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.payload='{}'; RETURN NEW; END $$; CREATE TRIGGER alter_transfer BEFORE INSERT ON events FOR EACH ROW EXECUTE FUNCTION alter_transfer();").execute(&target).await?;
    let mut tx = target.begin().await?;
    ensure!(
        prepared
            .apply(&mut tx, &target_blobs, transfer, &control)
            .await
            .is_err(),
        "changed content accepted"
    );
    tx.rollback().await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(&target)
        .await?;
    ensure!(count == 1, "failed import leaked rows");
    sqlx::raw_sql("DROP TRIGGER alter_transfer ON events; DROP FUNCTION alter_transfer();")
        .execute(&target)
        .await?;
    let mut tx = target.begin().await?;
    let checksums = prepared
        .apply(&mut tx, &target_blobs, transfer, &control)
        .await?;
    ensure!(
        checksums["events"].rows == 1 && checksums["task_events"].rows == 1,
        "missing imported content"
    );
    tx.commit().await?;
    let imported: (Uuid, Uuid, Value) =
        sqlx::query_as("SELECT project_id,issue_id,payload FROM events WHERE id=$1")
            .bind(event)
            .fetch_one(&target)
            .await?;
    ensure!(
        imported.0 == destination && imported.1 == mapped_id(transfer, "issues", issue),
        "reference mapping wrong"
    );
    ensure!(
        imported.2["nested"]["id"] == event.to_string(),
        "telemetry rewritten"
    );
    let principal: (String, String, String) =
        sqlx::query_as("SELECT email,password_hash,role FROM users WHERE id=$1")
            .bind(mapped_id(transfer, "users", user))
            .fetch_one(&target)
            .await?;
    ensure!(
        principal.0.ends_with("@migration.invalid")
            && principal.1 == "!migration"
            && principal.2 == "admin",
        "credentials or privileges copied"
    );
    let private:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM tokens)+(SELECT count(*) FROM auth_sessions)+(SELECT count(*) FROM project_assignments)+(SELECT count(*) FROM cloud_user_identities)").fetch_one(&target).await?;
    ensure!(private == 0, "authentication/authorization copied");
    let task_state: (bool, Value) =
        sqlx::query_as("SELECT verified,automation FROM tasks WHERE id=$1")
            .bind(mapped_id(transfer, "tasks", task))
            .fetch_one(&target)
            .await?;
    ensure!(
        !task_state.0 && task_state.1 == json!({}),
        "active automation copied"
    );
    let activity: (String, Option<Uuid>) =
        sqlx::query_as("SELECT category,notify_user_id FROM task_events")
            .fetch_one(&target)
            .await?;
    ensure!(
        activity == ("task".into(), None),
        "historical activity created a notification"
    );
    ensure!(
        std::fs::read(
            target_blobs
                .join(&hash[..2])
                .join(format!("{}.bin", &hash[2..]))
        )? == bytes,
        "blob bytes changed"
    );
    let mut tx = target.begin().await?;
    ensure!(
        prepared
            .apply(&mut tx, &target_blobs, transfer, &control)
            .await
            .is_err(),
        "overwrote existing projects"
    );
    tx.rollback().await?;

    // Final reconciliation happens only while destination projects are fenced.
    // Deletions, updates and late source events must replace the earlier copy.
    let late = Uuid::new_v4();
    sqlx::query("DELETE FROM tasks WHERE id=$1")
        .bind(task)
        .execute(&source)
        .await?;
    sqlx::query("UPDATE projects SET name='Final source name' WHERE id=$1")
        .bind(project)
        .execute(&source)
        .await?;
    sqlx::query("INSERT INTO events(id,project_id,issue_id,kind,platform,occurred_at) VALUES($1,$2,$3,'error','javascript',now())")
        .bind(late).bind(project).bind(issue).execute(&source).await?;
    let final_archive = root.path().join("final.sentori");
    capture_projects(
        &source,
        &source_blobs,
        &final_archive,
        key.clone(),
        "5.0.0",
        &[project],
        &control,
    )
    .await?;
    let mut final_copy = PreparedImport::open(&final_archive, key, &control)?;
    let mut tx = target.begin().await?;
    sqlx::query("DELETE FROM projects WHERE id=$1")
        .bind(destination)
        .execute(&mut *tx)
        .await?;
    final_copy
        .apply(&mut tx, &target_blobs, transfer, &control)
        .await?;
    tx.commit().await?;
    let final_rows:(i64,i64,String)=sqlx::query_as("SELECT (SELECT count(*) FROM events WHERE project_id=$1),(SELECT count(*) FROM tasks WHERE project_id=$1),name FROM projects WHERE id=$1")
        .bind(destination).fetch_one(&target).await?;
    ensure!(
        final_rows == (2, 0, "Final source name".into()),
        "final changes/deletions not reconciled"
    );
    let keep: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id=$1 AND name='Unrelated target')",
    )
    .bind(unrelated)
    .fetch_one(&target)
    .await?;
    ensure!(keep, "unrelated target changed");
    let binding = fence_contract(&source, project, foreign, issue, event).await?;
    let sealed_path = root.path().join("sealed.sentori");
    let key = RecoveryKey::from_bytes([91; 32]);
    ensure!(
        capture_transfer(
            &source,
            &source_blobs,
            &sealed_path,
            key.clone(),
            "5.0.0",
            &binding,
            &control
        )
        .await
        .is_err(),
        "final package accepted before sealed receipt"
    );
    sqlx::query("UPDATE project_transfers SET state='sealed',sealed_at=now() WHERE id=$1")
        .bind(binding.id)
        .execute(&source)
        .await?;
    let manifest = capture_transfer(
        &source,
        &source_blobs,
        &sealed_path,
        key.clone(),
        "5.0.0",
        &binding,
        &control,
    )
    .await?;
    ensure!(
        manifest.header.transfer.as_ref() == Some(&binding),
        "transfer binding missing from authenticated header"
    );
    let mut sealed = PreparedImport::open(&sealed_path, key, &control)?;
    let mut tx = target.begin().await?;
    let mismatch = sealed
        .apply(&mut tx, &target_blobs, binding.id, &control)
        .await
        .err()
        .context("wrong target accepted")?;
    ensure!(
        mismatch.to_string().contains("target identity"),
        "destination binding was not checked before writes"
    );
    tx.rollback().await?;
    println!(
        "PASS verified project import, mapped identities, credential/automation exclusion, blob/readback checks, transactional failure, replay guard and final reconciliation"
    );
    source.close().await;
    target.close().await;
    admin.close().await;
    Ok(())
}

async fn fence_contract(
    source: &PgPool,
    project: Uuid,
    foreign: Uuid,
    issue: Uuid,
    event: Uuid,
) -> anyhow::Result<TransferBinding> {
    let transfer = Uuid::new_v4();
    let destination = Uuid::new_v4();
    sqlx::query("INSERT INTO project_transfers(id,direction,instance_id,generation,source_instance_id,source_generation,target_instance_id,target_generation,project_ids) SELECT $1,'source',id,generation,id,generation,$2,1,$3 FROM instance_settings WHERE singleton")
        .bind(transfer).bind(destination).bind(vec![project]).execute(source).await?;
    let mut old_snapshot = source.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *old_snapshot)
        .await?;
    sqlx::query("SELECT id FROM projects WHERE id=$1")
        .bind(project)
        .fetch_one(&mut *old_snapshot)
        .await?;
    let mut writer = source.begin().await?;
    let writer_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await?;
    sqlx::query("INSERT INTO events(id,project_id,issue_id,kind,platform,occurred_at) VALUES($1,$2,$3,'error','javascript',now())")
        .bind(Uuid::new_v4()).bind(project).bind(issue).execute(&mut *writer).await?;
    let pool = source.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    let seal = tokio::spawn(async move {
        let mut tx = pool.begin().await?;
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await?;
        let _ = send.send(pid);
        sqlx::query("SELECT set_config('sentori.transfer_id',$1,true)")
            .bind(transfer.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE projects SET migration_fence=$1 WHERE id=$2")
            .bind(transfer)
            .bind(project)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok::<_, anyhow::Error>(())
    });
    let seal_pid = receive.await?;
    let mut blocked = false;
    for _ in 0..100 {
        blocked = sqlx::query_scalar("SELECT $1=ANY(pg_blocking_pids($2))")
            .bind(writer_pid)
            .bind(seal_pid)
            .fetch_one(source)
            .await?;
        if blocked {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    ensure!(blocked, "fence did not wait for the admitted writer");
    writer.commit().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), seal).await???;
    let stale=sqlx::query("INSERT INTO events(id,project_id,issue_id,kind,platform,occurred_at) VALUES($1,$2,$3,'error','javascript',now())")
        .bind(Uuid::new_v4()).bind(project).bind(issue).execute(&mut *old_snapshot).await;
    ensure!(
        stale
            .as_ref()
            .err()
            .and_then(sqlx::Error::as_database_error)
            .and_then(|e| e.code())
            .as_deref()
            == Some("40001"),
        "repeatable-read writer bypassed the fence"
    );
    old_snapshot.rollback().await?;
    for sql in [
        "UPDATE events SET payload='{}' WHERE id=$1",
        "DELETE FROM events WHERE id=$1",
    ] {
        // Audited: only the two fixed statements immediately above.
        let result = sqlx::query(AssertSqlSafe(sql))
            .bind(event)
            .execute(source)
            .await;
        ensure!(
            result
                .as_ref()
                .err()
                .and_then(sqlx::Error::as_database_error)
                .and_then(|e| e.code())
                .as_deref()
                == Some("55000"),
            "ordinary write bypassed the fence"
        );
    }
    let moved = sqlx::query("UPDATE events SET project_id=$1 WHERE id=$2")
        .bind(foreign)
        .bind(event)
        .execute(source)
        .await;
    ensure!(
        moved.is_err(),
        "moving a row out of a fenced project bypassed the fence"
    );
    let child = sqlx::query("INSERT INTO issue_activity(id,issue_id,kind) VALUES($1,$2,'note')")
        .bind(Uuid::new_v4())
        .bind(issue)
        .execute(source)
        .await;
    ensure!(child.is_err(), "parent-scoped write bypassed the fence");
    sqlx::query("UPDATE projects SET name='Still writable' WHERE id=$1")
        .bind(foreign)
        .execute(source)
        .await?;
    let remove = sqlx::query("DELETE FROM projects WHERE id=$1")
        .bind(project)
        .execute(source)
        .await;
    ensure!(remove.is_err(), "project deletion bypassed the fence");
    println!(
        "PASS cutover waits for admitted writers, rejects stale snapshots and source/parent-scope changes, and preserves other projects"
    );
    Ok(TransferBinding {
        id: transfer,
        target_instance_id: destination,
        target_generation: 1,
        phase: TransferPhase::Final,
    })
}
