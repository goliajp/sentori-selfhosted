//! `sentorictl` subcommand impls.

pub mod dump;
pub mod export;
pub mod import;
pub mod restore;
pub mod status;

/// Tables snapshotted by `dump`, scanned by `status`, and reloaded by
/// `restore`, in an order where a parent always precedes its children.
///
/// It was a list from the pre-v1 kernel until 2026-09-06 and nobody
/// had noticed: **twenty-seven of its thirty-four entries named tables
/// the v1 rewrite deleted, and it reached seven of our twenty-six.**
/// So `dump` wrote seven files, `restore` skipped the nineteen it could
/// not find — silently, by design, because a snapshot may be
/// incremental — and the whole round trip reported success. A backup
/// that omits `tokens`, `releases`, `release_artifacts`, every push
/// table and every attachment is not a backup, and it said so nowhere.
///
/// The order below is the foreign-key topology of `core/migrations`,
/// computed rather than curated. `check-sql-tables-exist.mjs` does not
/// cover this list — these are bare strings, not SQL — so the gate that
/// covers it is `dump-restore-covers-the-schema` in the e2e suite,
/// which round-trips a populated database and compares row counts.
pub const TABLES: &[&str] = &[
    "users",
    "auth_sessions",
    "password_resets",
    "audit_logs",
    "projects",
    "project_assignments",
    "tokens",
    "issues",
    "events",
    "issue_user_hits",
    "issue_activity",
    "event_attachments",
    "releases",
    "release_artifacts",
    "probes",
    "assert_stats",
    "notification_prefs",
    "delivery_log",
    "push_tokens",
    "push_credentials",
    "device_tokens",
    "push_sends",
    "push_delivery_logs",
    "device_topics",
    "push_preferences",
    "backend_checks",
];

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "setup failure in a test should stop the test, loudly and at \
              the line that failed. The crate denies `expect` because a \
              CLI that panics on a user's data is a bad CLI; a test that \
              cannot read the migrations has nothing to assert and must \
              say so rather than pass."
)]
mod schema_agreement {
    //! `TABLES` and `PROJECT_SCOPED` must agree with `core/migrations`.
    //!
    //! Until 2026-09-06 they did not, and nothing said so. `TABLES` was
    //! the pre-v1 kernel's list: twenty-seven of its thirty-four names
    //! were tables the v1 rewrite had deleted, and it reached seven of
    //! the twenty-six that exist. `dump` wrote seven files; `restore`
    //! skipped the missing nineteen silently, because a snapshot may be
    //! incremental; the round trip reported success both ways.
    //!
    //! Nothing else can catch this. The compiler sees a list of string
    //! literals. Clippy sees a list of string literals. The names are
    //! only wrong relative to a directory of SQL, so the test has to
    //! read that directory — which is why this reads the migrations at
    //! test time rather than embedding a copy that could rot the same
    //! way the list did.

    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use super::TABLES;

    fn migrations_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../core/migrations")
            .canonicalize()
            .expect("core/migrations must be reachable from the CLI crate")
    }

    /// Every `CREATE TABLE` in the migrations, and whether it has a
    /// `project_id` column.
    fn schema() -> Vec<(String, bool)> {
        let dir = migrations_dir();
        let mut files: Vec<_> = fs::read_dir(&dir)
            .expect("read core/migrations")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "sql"))
            .collect();
        files.sort();

        let mut out = Vec::new();
        for f in files {
            let src = fs::read_to_string(&f).expect("read a migration");
            let mut rest = src.as_str();
            while let Some(at) = rest.find("CREATE TABLE ") {
                rest = &rest[at + "CREATE TABLE ".len()..];
                let rest_trim = rest.strip_prefix("IF NOT EXISTS ").unwrap_or(rest);
                let name: String = rest_trim
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if name.is_empty() {
                    continue;
                }
                // The body runs to the line that closes the statement.
                let body_end = rest_trim.find("\n);").unwrap_or(rest_trim.len());
                let body = &rest_trim[..body_end];
                let has_pid = body
                    .lines()
                    .any(|l| l.trim_start().starts_with("project_id "));
                out.push((name, has_pid));
            }
        }
        out
    }

    // A test that reads nothing passes everything.
    #[test]
    fn the_test_can_read_the_schema() {
        let s = schema();
        assert!(
            s.len() >= 20,
            "read only {} tables from core/migrations — this test is broken, \
             not the tree. It has read 26 before now.",
            s.len()
        );
    }

    #[test]
    fn tables_is_exactly_the_schema() {
        let want: BTreeSet<String> = schema().into_iter().map(|(t, _)| t).collect();
        let have: BTreeSet<String> = TABLES.iter().map(|s| (*s).to_string()).collect();

        let missing: Vec<_> = want.difference(&have).collect();
        let extra: Vec<_> = have.difference(&want).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "TABLES disagrees with core/migrations.\n  \
             in the schema but not dumped ({}): {:?}\n  \
             dumped but not in the schema ({}): {:?}",
            missing.len(),
            missing,
            extra.len(),
            extra
        );
    }

    #[test]
    fn project_scoped_is_exactly_the_tables_with_a_project_id() {
        let want: BTreeSet<String> = schema()
            .into_iter()
            .filter(|(_, pid)| *pid)
            .map(|(t, _)| t)
            .collect();
        let have: BTreeSet<String> = super::export::project_scoped_for_test()
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        let missing: Vec<_> = want.difference(&have).collect();
        let extra: Vec<_> = have.difference(&want).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "PROJECT_SCOPED disagrees with core/migrations.\n  \
             has project_id but is not filtered ({}): {:?}\n  \
             filtered but has no project_id ({}): {:?}",
            missing.len(),
            missing,
            extra.len(),
            extra
        );
    }

    #[test]
    fn a_parent_always_precedes_its_children() {
        // `restore` loads in this order and relies on it: a child row
        // whose parent is not in yet is a foreign-key violation.
        let pos = |t: &str| TABLES.iter().position(|x| *x == t);
        for (child, parent) in [
            ("auth_sessions", "users"),
            ("project_assignments", "projects"),
            ("events", "issues"),
            ("release_artifacts", "releases"),
            ("push_sends", "device_tokens"),
            ("push_delivery_logs", "push_sends"),
            ("device_topics", "device_tokens"),
        ] {
            let (c, p) = (pos(child), pos(parent));
            assert!(
                c.is_some() && p.is_some(),
                "{child} or {parent} is missing from TABLES"
            );
            assert!(
                p < c,
                "TABLES lists {child} before its parent {parent}; restore would \
                 fail on the foreign key"
            );
        }
    }
}
