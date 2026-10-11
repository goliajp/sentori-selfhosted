//! Project transfer format. Positive table AND column lists deliberately exclude
//! authentication, authorization, credentials, automation and pending deliveries.
//! Telemetry, source files and human-authored content are preserved, not scrubbed.
use anyhow::{Context, ensure};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub const TABLES: &[&str] = &[
    "projects",
    "apps",
    "users",
    "issues",
    "events",
    "issue_user_hits",
    "sessions",
    "issue_activity",
    "event_attachments",
    "releases",
    "app_releases",
    "release_artifacts",
    "tasks",
    "task_events",
    "probes",
    "assert_stats",
];

/// Identifiers below are compile-time literals. Never derive SQL identifiers
/// from package input or a caller's project list.
pub fn columns(table: &str) -> &'static [&'static str] {
    match table {
        "projects" => &["id", "name", "platform", "created_at"],
        "apps" => &[
            "id",
            "project_id",
            "platform",
            "identifier",
            "has_explicit_identifier",
            "name",
            "repository_url",
            "version",
            "created_at",
            "last_event_at",
            "last_session_at",
        ],
        "app_releases" => &[
            "project_id",
            "app_id",
            "release_id",
            "first_seen_at",
            "last_seen_at",
        ],
        "users" => &["id", "display_name"],
        "issues" => &[
            "id",
            "project_id",
            "fingerprint",
            "kind",
            "group_title",
            "message_sample",
            "surface",
            "status",
            "first_seen",
            "last_seen",
            "event_count",
            "users_count",
            "max_per_user",
            "last_environment",
            "last_release",
            "assignee_user_id",
            "resolved_at",
            "resolved_in_release",
            "regressed_at",
            "regressed_in_release",
            "environment",
            "platform",
        ],
        "events" => &[
            "id",
            "project_id",
            "app_id",
            "issue_id",
            "kind",
            "platform",
            "occurred_at",
            "received_at",
            "release",
            "environment",
            "user_key",
            "payload",
        ],
        "issue_user_hits" => &["issue_id", "user_key", "hit_count", "last_hit"],
        "sessions" => &[
            "id",
            "project_id",
            "app_id",
            "status",
            "release",
            "environment",
            "platform",
            "started_at",
            "received_at",
            "duration_ms",
            "user_key",
        ],
        "issue_activity" => &["id", "issue_id", "at", "actor_user_id", "kind", "body"],
        "event_attachments" => &[
            "ref",
            "project_id",
            "event_id",
            "kind",
            "media_type",
            "size_bytes",
            "blob_hash",
            "source",
            "captured_at",
            "received_at",
        ],
        "releases" => &["id", "project_id", "name", "created_at"],
        "release_artifacts" => &[
            "id",
            "release_id",
            "kind",
            "name",
            "content_hash",
            "size_bytes",
            "meta",
            "created_at",
            "usable",
        ],
        "tasks" => &[
            "id",
            "project_id",
            "issue_id",
            "assignee_user_id",
            "created_by",
            "title",
            "description",
            "kind",
            "priority",
            "stage",
            "due_date",
            "reproduction",
            "root_cause",
            "fix_release",
            "created_at",
            "updated_at",
        ],
        "task_events" => &[
            "id",
            "task_id",
            "actor_user_id",
            "action",
            "body",
            "created_at",
        ],
        "probes" => &[
            "id",
            "project_id",
            "ref",
            "issue_id",
            "first_registered_release",
            "last_seen_release",
            "registered_at",
            "last_fired_at",
            "fire_count",
        ],
        "assert_stats" => &[
            "project_id",
            "name",
            "release",
            "pass_count",
            "fail_count",
            "last_pass_at",
            "last_fail_at",
        ],
        _ => &[],
    }
}

pub fn query(table: &str) -> anyhow::Result<String> {
    let columns = columns(table);
    ensure!(!columns.is_empty(), "unknown migration table");
    let condition = match table {
        "projects" => "id = ANY($1)",
        "users" => {
            "id IN (SELECT assignee_user_id FROM issues WHERE project_id=ANY($1) UNION SELECT actor_user_id FROM issue_activity WHERE issue_id IN (SELECT id FROM issues WHERE project_id=ANY($1)) UNION SELECT assignee_user_id FROM tasks WHERE project_id=ANY($1) UNION SELECT created_by FROM tasks WHERE project_id=ANY($1) UNION SELECT actor_user_id FROM task_events WHERE task_id IN (SELECT id FROM tasks WHERE project_id=ANY($1)))"
        }
        "issue_user_hits" | "issue_activity" => {
            "issue_id IN (SELECT id FROM issues WHERE project_id=ANY($1))"
        }
        "release_artifacts" => "release_id IN (SELECT id FROM releases WHERE project_id=ANY($1))",
        "task_events" => "task_id IN (SELECT id FROM tasks WHERE project_id=ANY($1))",
        _ => "project_id=ANY($1)",
    };
    Ok(format!(
        "SELECT row_to_json(t) AS row FROM (SELECT {} FROM {table} WHERE {condition}) t",
        columns.join(",")
    ))
}

/// Enforce the declared scope independently while reading the package. Parent
/// rows always precede children, so references cannot introduce other projects.
pub(crate) struct Scope {
    projects: BTreeSet<Uuid>,
    seen: BTreeMap<String, BTreeMap<Uuid, Option<Uuid>>>,
}
impl Scope {
    pub fn new(projects: &[Uuid]) -> anyhow::Result<Self> {
        let unique: BTreeSet<_> = projects.iter().copied().collect();
        ensure!(
            !unique.is_empty() && unique.len() <= 1000 && unique.len() == projects.len(),
            "invalid migration project scope"
        );
        Ok(Self {
            projects: unique,
            seen: BTreeMap::new(),
        })
    }
    pub fn row(&mut self, table: &str, row: &Value) -> anyhow::Result<()> {
        let id = |key: &str| -> anyhow::Result<Uuid> {
            Ok(row[key]
                .as_str()
                .context("migration identity missing")?
                .parse()?)
        };
        let mut project = None;
        if table == "projects" {
            let value = id("id")?;
            ensure!(
                self.projects.contains(&value),
                "undeclared migration project"
            );
            project = Some(value);
        } else if !row["project_id"].is_null() {
            let value = id("project_id")?;
            ensure!(
                self.seen
                    .get("projects")
                    .is_some_and(|m| m.contains_key(&value)),
                "row outside migration project scope"
            );
            project = Some(value);
        }
        for (key, parent) in [
            ("app_id", "apps"),
            ("issue_id", "issues"),
            ("release_id", "releases"),
            ("task_id", "tasks"),
        ] {
            if !row[key].is_null() {
                let value = id(key)?;
                let parent_project = *self
                    .seen
                    .get(parent)
                    .and_then(|m| m.get(&value))
                    .context("missing migration parent")?;
                ensure!(
                    project.is_none() || project == parent_project,
                    "cross-project migration reference"
                );
                project = parent_project;
            }
        }
        // Attachments intentionally have no FK: retention may remove the event
        // before its attachment; their explicit project scope is authoritative.
        if table == "event_attachments" {
            let event = id("event_id")?;
            if let Some(event_project) = self.seen.get("events").and_then(|m| m.get(&event)) {
                ensure!(
                    project == *event_project,
                    "cross-project attachment reference"
                );
            }
        }
        for key in ["assignee_user_id", "actor_user_id", "created_by"] {
            if !row[key].is_null() {
                let principal = id(key)?;
                ensure!(
                    self.seen
                        .get("users")
                        .is_some_and(|m| m.contains_key(&principal)),
                    "unknown migration principal"
                );
            }
        }
        if !row["id"].is_null() {
            ensure!(
                self.seen
                    .entry(table.to_owned())
                    .or_default()
                    .insert(id("id")?, project)
                    .is_none(),
                "duplicate migration identity"
            );
        }
        Ok(())
    }
    pub fn finish(&self) -> anyhow::Result<()> {
        ensure!(
            self.seen
                .get("projects")
                .is_some_and(|m| m.len() == self.projects.len()),
            "requested project missing from package"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{ArchiveWriter, Column, Header, Mode, Record};
    use crate::{RecoveryKey, verify::verify};
    use serde_json::json;
    use sha2::{Digest, Sha256};

    fn package(extra_secret: bool, foreign_project: bool) -> anyhow::Result<Vec<u8>> {
        let project = Uuid::new_v4();
        let header = Header {
            format: 1,
            archive_id: Uuid::new_v4(),
            instance_id: Uuid::new_v4(),
            generation: 1,
            created_at: "2026-10-10T00:00:00Z".into(),
            application_version: "5.0.0".into(),
            migrations: crate::expected_migrations(),
            master_key_fingerprint: None,
            mode: Mode::ProjectMigration,
            project_ids: vec![project],
            transfer: None,
        };
        let mut writer = ArchiveWriter::new(Vec::new(), RecoveryKey::from_bytes([5; 32]), header)?;
        for table in TABLES {
            let mut names = columns(table).to_vec();
            if *table == "projects" && extra_secret {
                names.push("webhook_secret_blob");
            }
            writer.write(Record::Table {
                name: (*table).into(),
                columns: names
                    .iter()
                    .map(|name| Column {
                        name: (*name).into(),
                        data_type: "text".into(),
                        nullable: true,
                        default_expression: None,
                    })
                    .collect(),
            })?;
            let mut digest = Sha256::new();
            let count = u64::from(*table == "projects");
            if *table == "projects" {
                let mut row = json!({"id":if foreign_project {Uuid::new_v4()} else {project}, "name":"Migration", "platform":null, "created_at":"2026-10-10T00:00:00Z"});
                if extra_secret {
                    row["webhook_secret_blob"] = json!("secret");
                }
                digest.update(serde_json::to_vec(&row)?);
                digest.update(b"\n");
                writer.write(Record::Row {
                    table: (*table).into(),
                    value: row,
                })?;
            }
            writer.write(Record::TableEnd {
                name: (*table).into(),
                rows: count,
                sha256: hex::encode(digest.finalize()),
            })?;
        }
        writer.finish()
    }
    #[test]
    fn independently_rejects_secrets_and_wrong_project_scope() -> anyhow::Result<()> {
        for (secret, foreign) in [(false, false), (true, false), (false, true)] {
            let bytes = package(secret, foreign)?;
            assert_eq!(
                verify(bytes.as_slice(), RecoveryKey::from_bytes([5; 32])).is_ok(),
                !secret && !foreign
            );
        }
        assert!(query("tokens").is_err());
        assert!(query("projects; DROP TABLE users").is_err());
        assert!(Scope::new(&[]).is_err());
        let project = Uuid::new_v4();
        assert!(Scope::new(&[project, project]).is_err());
        Ok(())
    }
    #[test]
    fn child_references_cannot_cross_projects() -> anyhow::Result<()> {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let issue = Uuid::new_v4();
        let mut scope = Scope::new(&[a, b])?;
        scope.row("projects", &json!({"id":a}))?;
        scope.row("projects", &json!({"id":b}))?;
        scope.row("issues", &json!({"id":issue,"project_id":a}))?;
        assert!(
            scope
                .row(
                    "events",
                    &json!({"id":Uuid::new_v4(),"project_id":b,"issue_id":issue})
                )
                .is_err()
        );
        assert!(
            scope
                .row(
                    "tasks",
                    &json!({"id":Uuid::new_v4(),"project_id":a,"created_by":Uuid::new_v4()})
                )
                .is_err()
        );
        let event = Uuid::new_v4();
        scope.row(
            "events",
            &json!({"id":event,"project_id":a,"issue_id":issue}),
        )?;
        assert!(
            scope
                .row(
                    "event_attachments",
                    &json!({"event_id":event,"project_id":b})
                )
                .is_err()
        );
        scope.finish()
    }
}
