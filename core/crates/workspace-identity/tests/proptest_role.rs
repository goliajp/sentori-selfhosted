//! Property tests for [`sentori_workspace_identity::Role`] /
//! [`sentori_workspace_identity::InviteRole`] capability fns
//! and DB-string round-trips.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_workspace_identity::{InviteRole, Role};

fn role_strategy() -> impl Strategy<Value = Role> {
    prop_oneof![Just(Role::Owner), Just(Role::Admin), Just(Role::User),]
}

fn invite_role_strategy() -> impl Strategy<Value = InviteRole> {
    prop_oneof![Just(InviteRole::Admin), Just(InviteRole::User)]
}

proptest! {
    #[test]
    fn role_db_str_round_trip(role in role_strategy()) {
        let s = role.as_db_str();
        prop_assert_eq!(Role::from_db_str(s).expect("known role"), role);
    }

    #[test]
    fn invite_role_db_str_round_trip(role in invite_role_strategy()) {
        let s = role.as_db_str();
        prop_assert_eq!(InviteRole::from_db_str(s).expect("known role"), role);
    }

    #[test]
    fn role_capabilities_consistent(role in role_strategy()) {
        // can_grant_admin and can_transfer_owner are Owner-only.
        prop_assert_eq!(role.can_grant_admin(), matches!(role, Role::Owner));
        prop_assert_eq!(role.can_transfer_owner(), matches!(role, Role::Owner));

        // The "elevated" predicates (manage workspace, create
        // project, manage users, auto-see) all imply each other
        // for the current model.
        let elevated = matches!(role, Role::Owner | Role::Admin);
        prop_assert_eq!(role.can_manage_workspace(), elevated);
        prop_assert_eq!(role.can_create_project(), elevated);
        prop_assert_eq!(role.can_manage_users(), elevated);
        prop_assert_eq!(role.auto_sees_all_projects(), elevated);
    }

    #[test]
    fn role_only_owner_admin_user(role in role_strategy()) {
        let canonical = ["owner", "admin", "user"];
        prop_assert!(canonical.contains(&role.as_db_str()));
    }

    #[test]
    fn from_db_str_rejects_unknown(s in "[a-z]{1,8}") {
        prop_assume!(!["owner", "admin", "user"].contains(&s.as_str()));
        prop_assert!(Role::from_db_str(&s).is_err());
    }

    #[test]
    fn invite_role_promotes_consistently(invite in invite_role_strategy()) {
        let promoted = invite.to_role();
        // InviteRole -> Role never lands on Owner.
        prop_assert!(!matches!(promoted, Role::Owner));
        match (invite, promoted) {
            (InviteRole::Admin, Role::Admin) | (InviteRole::User, Role::User) => {}
            (i, p) => prop_assert!(false, "unexpected promotion: {:?} -> {:?}", i, p),
        }
    }
}

#[test]
fn role_all_covers_every_variant() {
    let all = Role::ALL;
    assert_eq!(all.len(), 3);
    for r in [Role::Owner, Role::Admin, Role::User] {
        assert!(all.contains(&r), "Role::ALL missing {r:?}");
    }
}

#[test]
fn invite_role_all_covers_every_variant() {
    let all = InviteRole::ALL;
    assert_eq!(all.len(), 2);
    assert!(all.contains(&InviteRole::Admin));
    assert!(all.contains(&InviteRole::User));
}

#[test]
fn role_display_matches_db_str() {
    for r in Role::ALL {
        assert_eq!(format!("{r}"), r.as_db_str());
    }
}

#[test]
fn invite_role_display_matches_db_str() {
    for r in InviteRole::ALL {
        assert_eq!(format!("{r}"), r.as_db_str());
    }
}
