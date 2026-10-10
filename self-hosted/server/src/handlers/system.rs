//! Authenticated discovery of implemented capabilities. This is not an
//! entitlement override: mutation handlers still enforce their own authority.

use std::collections::BTreeMap;

use crate::state::AppState;
use axum::{Extension, Json, extract::State};
use serde::Serialize;
use std::sync::Arc;

use crate::session_mw::{Role, SessionContext};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    supported: bool,
    configured: bool,
    allowed: bool,
    reason: Option<&'static str>,
}

fn capability(supported: bool, configured: bool, allowed: bool) -> Capability {
    Capability {
        supported,
        configured,
        allowed: supported && configured && allowed,
        reason: if !supported {
            Some("not_implemented")
        } else if !configured {
            Some("not_configured")
        } else if !allowed {
            Some("owner_required")
        } else {
            None
        },
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    api_version: u8,
    server_version: &'static str,
    deployment: &'static str,
    capabilities: BTreeMap<&'static str, Capability>,
}

fn describe(role: Role, backup_configured: bool, transfer_configured: bool) -> SystemInfo {
    let owner = role.is_superadmin();
    let mut capabilities = BTreeMap::new();
    for key in [
        "projects.read",
        "issues",
        "releases",
        "jobs",
        "instruments",
        "push",
        "tokens",
    ] {
        capabilities.insert(key, capability(true, true, true));
    }
    for key in [
        "projects.manage",
        "members.manage",
        "audit",
        "instance.configure",
        "instance.healthcheck",
    ] {
        capabilities.insert(key, capability(true, true, owner));
    }
    capabilities.insert("instance.read", capability(true, true, true));
    capabilities.insert("tasks", capability(true, true, true));
    capabilities.insert("inbox", capability(true, true, true));
    capabilities.insert("monitors", capability(true, true, true));
    capabilities.insert("integrations", capability(true, true, true));
    capabilities.insert(
        "instance.backup",
        capability(true, backup_configured, owner),
    );
    capabilities.insert("instance.reset", capability(true, backup_configured, owner));
    capabilities.insert(
        "cloud.migrate",
        capability(true, transfer_configured, owner),
    );
    // Names are stable so a newer UI can explain an unavailable capability
    // without assuming that a design preview proves backend support.
    for key in [
        "instance.restore",
        "cloud.billing",
        "auth.oauth",
        "auth.saml",
    ] {
        capabilities.insert(key, capability(false, false, false));
    }
    SystemInfo {
        api_version: 1,
        server_version: env!("CARGO_PKG_VERSION"),
        deployment: "self-hosted",
        capabilities,
    }
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> Json<SystemInfo> {
    let mut info = describe(
        ctx.role,
        crate::instance_operations::configured(&state),
        crate::project_transfers::configured(&state),
    );
    if state.cloud.is_some() {
        info.deployment = "cloud";
        for name in [
            "members.manage",
            "instance.configure",
            "instance.healthcheck",
            "instance.backup",
            "instance.reset",
            "instance.restore",
        ] {
            info.capabilities.insert(
                name,
                Capability {
                    supported: true,
                    configured: true,
                    allowed: false,
                    reason: Some("managed_by_cloud"),
                },
            );
        }
        info.capabilities.insert(
            "members.assign",
            capability(true, true, ctx.role.is_superadmin()),
        );
    }
    Json(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_cannot_acquire_owner_capabilities() {
        let member = describe(Role::Admin, true, true);
        assert!(member.capabilities["instance.read"].allowed);
        assert!(!member.capabilities["instance.backup"].allowed);
        assert!(!member.capabilities["cloud.migrate"].allowed);
        assert!(describe(Role::Superadmin, true, true).capabilities["cloud.migrate"].allowed);
        assert!(!describe(Role::Superadmin, true, false).capabilities["cloud.migrate"].allowed);
        assert!(describe(Role::Superadmin, true, true).capabilities["instance.backup"].allowed);
        assert_eq!(
            describe(Role::Superadmin, false, false).capabilities["instance.backup"].reason,
            Some("not_configured")
        );
        assert!(!member.capabilities["instance.configure"].allowed);
        assert_eq!(
            member.capabilities["members.manage"].reason,
            Some("owner_required")
        );
        assert!(describe(Role::Superadmin, true, true).capabilities["instance.configure"].allowed);
    }

    #[test]
    fn previews_are_not_advertised_as_real_services_even_to_owner() {
        let owner = describe(Role::Superadmin, true, true);
        for key in ["cloud.billing", "auth.oauth", "auth.saml"] {
            assert!(!owner.capabilities[key].supported);
            assert!(!owner.capabilities[key].allowed);
            assert_eq!(owner.capabilities[key].reason, Some("not_implemented"));
        }
    }
}
