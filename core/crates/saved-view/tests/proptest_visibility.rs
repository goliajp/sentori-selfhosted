//! Property tests for `SavedView::is_visible_to`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_saved_view::{SavedView, Scope, Target};
use sentori_workspace_identity::UserId;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

fn view(scope: Scope, owner: Option<UserId>) -> SavedView {
    SavedView {
        id: Uuid::now_v7(),
        project_id: None,
        target: Target::Issues,
        scope,
        user_id: owner,
        name: "x".into(),
        payload: Value::Null,
        created_at: OffsetDateTime::UNIX_EPOCH,
        created_by: owner,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

proptest! {
    #[test]
    fn workspace_view_visible_to_anyone(_seed in 0u8..255) {
        let v = view(Scope::Workspace, None);
        let viewer = UserId::new();
        prop_assert!(v.is_visible_to(viewer));
    }

    #[test]
    fn personal_view_visible_only_to_owner(_seed in 0u8..255) {
        let owner = UserId::new();
        let viewer = UserId::new();
        prop_assume!(owner != viewer);
        let v = view(Scope::Personal, Some(owner));
        prop_assert!(v.is_visible_to(owner));
        prop_assert!(!v.is_visible_to(viewer));
    }
}
