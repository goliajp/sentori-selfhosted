//! Property tests for `AuditQuery` builder + limit clamping.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::redundant_clone
)]

use proptest::prelude::*;
use sentori_audit_event::{AuditEntryDraft, AuditQuery};
use sentori_workspace_identity::{ProjectId, UserId, WorkspaceId};

proptest! {
    #[test]
    fn resolved_limit_in_bounds(limit in any::<u32>()) {
        let q = AuditQuery { limit: Some(limit), ..Default::default() };
        let r = q.resolved_limit();
        if limit == 0 {
            prop_assert_eq!(r, AuditQuery::DEFAULT_LIMIT);
        } else {
            prop_assert!(r >= 1);
            prop_assert!(r <= AuditQuery::MAX_LIMIT);
        }
    }

    #[test]
    fn draft_builder_idempotent(
        action in "[a-z][a-z_.]{0,30}",
        target_type in "[a-z]{1,10}",
        target_id in "[a-zA-Z0-9_-]{1,30}",
    ) {
        let pid = ProjectId::new();
        let actor = UserId::new();
        let d = AuditEntryDraft::new(WorkspaceId::new(), &action)
            .with_project(pid)
            .with_actor(actor)
            .with_target(&target_type, &target_id);
        prop_assert_eq!(d.action, action);
        prop_assert_eq!(d.project_id, Some(pid));
        prop_assert_eq!(d.actor_user_id, Some(actor));
        prop_assert_eq!(d.target_type, Some(target_type));
        prop_assert_eq!(d.target_id, Some(target_id));
    }
}

#[test]
fn limit_default_when_unset() {
    let q = AuditQuery::default();
    assert_eq!(q.resolved_limit(), AuditQuery::DEFAULT_LIMIT);
}
