//! Property tests for the Role × Permission matrix.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_tenant_scoping::{Permission, role_allows};
use sentori_workspace_identity::Role;

proptest! {
    #[test]
    fn owner_dominates_admin_dominates_user(
        p in prop_oneof![
            Just(Permission::ViewProject),
            Just(Permission::WriteEvent),
            Just(Permission::EditProject),
            Just(Permission::DeleteProject),
            Just(Permission::ManageMembers),
            Just(Permission::PromoteToAdmin),
            Just(Permission::TransferOwner),
            Just(Permission::ManageIntegrations),
        ],
    ) {
        // Owner ⊇ Admin ⊇ User capability sets.
        let owner = role_allows(Role::Owner, p);
        let admin = role_allows(Role::Admin, p);
        let user = role_allows(Role::User, p);
        prop_assert!(owner);
        prop_assert!(owner >= admin, "owner ⊇ admin for {p}");
        prop_assert!(admin >= user, "admin ⊇ user for {p}");
    }
}

#[test]
fn permission_count_stays_8() {
    assert_eq!(Permission::ALL.len(), 8);
}
