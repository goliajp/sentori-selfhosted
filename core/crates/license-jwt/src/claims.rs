//! License claim types — the JWT payload schema for Sentori license tokens.
//!
//! The on-the-wire JSON layout is locked by [`LicenseClaims`]'s serde
//! attributes; any field rename or removal is a breaking change to every
//! issued token. See module-level docs in `lib.rs` for the authoritative
//! schema description.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::SENTORI_ISSUER;

/// Product edition that the license token authorises.
///
/// Mapped 1:1 onto the deployment binaries declared in
/// `product-architecture.html` §03. Self-hosted Community has no license
/// (forever unlocked) and therefore is **not** represented here — only
/// editions that require a signed token appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Edition {
    /// `sentori-saas` — GOLIA-hosted multi-tenant deployment.
    Saas,
    /// `sentori-selfhosted-enterprise` — customer-hosted commercial.
    Enterprise,
}

/// Pricing tier the license grants. Tiers are global across editions so
/// the same enum serves both SaaS billing plans and Enterprise contract
/// classifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Hobby / evaluation. Capped by [`LicenseLimits`].
    Free,
    /// Paid tier for individuals + small teams.
    Pro,
    /// Enterprise cloud (managed by GOLIA, contract-based limits).
    EnterpriseCloud,
    /// Enterprise self-host (per-deployment binary + contract limits).
    EnterpriseSelf,
}

impl Tier {
    /// Default per-tier resource limits applied at issuance time.
    ///
    /// Returning [`LicenseLimits::unlimited`] (all `None`) signals
    /// contract-governed limits enforced outside the JWT.
    #[must_use]
    pub const fn default_limits(self) -> LicenseLimits {
        match self {
            Self::Free => LicenseLimits {
                events_per_month: Some(5_000),
                users: Some(1),
                projects: Some(1),
                retention_days: Some(14),
            },
            Self::Pro => LicenseLimits {
                events_per_month: Some(100_000),
                users: Some(5),
                projects: Some(10),
                retention_days: Some(30),
            },
            Self::EnterpriseCloud | Self::EnterpriseSelf => LicenseLimits::unlimited(),
        }
    }
}

/// Hard caps the license enforces. `None` means unlimited.
///
/// Limits are encoded into the token at issue time; rotating limits
/// requires re-issuing the license (covered by the Stripe lifecycle in
/// `S5` / `S17`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct LicenseLimits {
    /// Ingested events allowed per calendar month.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events_per_month: Option<u64>,
    /// Maximum members across the workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub users: Option<u32>,
    /// Maximum projects within the workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projects: Option<u32>,
    /// Days of event retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
}

impl LicenseLimits {
    /// All fields `None` — contract-governed.
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            events_per_month: None,
            users: None,
            projects: None,
            retention_days: None,
        }
    }
}

/// Full set of claims encoded into a license JWT.
///
/// Field naming mirrors RFC 7519 for the standard registered claims
/// (`iss`, `sub`, `iat`, `exp`, `jti`) and uses snake_case for the
/// Sentori-specific extensions.
///
/// **Stability**: the JSON shape is part of the wire contract between
/// issuer (GOLIA) and verifier (deployed binary). Adding fields is
/// non-breaking when paired with `#[serde(default)]`; removing or
/// renaming is breaking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseClaims {
    /// Issuer — always [`SENTORI_ISSUER`]. Verified by [`crate::Verifier`].
    pub iss: String,
    /// Subject — tenant UUID for SaaS, customer string for Enterprise.
    pub sub: String,
    /// Issued-at, seconds since Unix epoch.
    pub iat: i64,
    /// Expiration (`period_end + grace_seconds`), seconds since Unix epoch.
    pub exp: i64,
    /// JWT ID — UUID v7 string. Targeted by the revocation list.
    pub jti: String,
    /// Which product binary may consume this token.
    pub edition: Edition,
    /// Which pricing tier the holder has paid for.
    pub tier: Tier,
    /// Tenant UUID for SaaS deployments (mirrors `sub`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<Uuid>,
    /// Stripe customer ID (Enterprise: sales-side identifier).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    /// Stripe subscription that produced this token (SaaS only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
    /// Feature flags unlocked. v0.1 stays empty; v0.2+ uses entries
    /// like `"sso"`, `"advanced_alert"`, `"white_label"`.
    #[serde(default)]
    pub features: Vec<String>,
    /// Hard caps. Defaults to per-tier limits at issue time.
    #[serde(default)]
    pub limits: LicenseLimits,
}

impl LicenseClaims {
    /// Construct claims for a SaaS tenant subscription.
    ///
    /// `expires_at` is the *grace-adjusted* expiry (i.e. caller
    /// pre-computes `current_period_end + grace`). `grace` is recorded
    /// only for symmetry with [`Self::enterprise`] and does not appear
    /// in the token.
    ///
    /// Limits are derived from `tier` via [`Tier::default_limits`].
    #[must_use]
    pub fn saas_tenant(
        tenant_id: Uuid,
        tier: Tier,
        subscription_id: String,
        expires_at: OffsetDateTime,
        _grace: time::Duration,
    ) -> Self {
        Self {
            iss: SENTORI_ISSUER.to_owned(),
            sub: tenant_id.to_string(),
            iat: OffsetDateTime::now_utc().unix_timestamp(),
            exp: expires_at.unix_timestamp(),
            jti: Uuid::now_v7().to_string(),
            edition: Edition::Saas,
            tier,
            tenant_id: Some(tenant_id),
            customer_id: None,
            subscription_id: Some(subscription_id),
            features: Vec::new(),
            limits: tier.default_limits(),
        }
    }

    /// Construct claims for an Enterprise (contract-based) license.
    ///
    /// `customer_id` is a free-form sales identifier rather than a
    /// Stripe ID — Enterprise licensing happens out-of-band.
    #[must_use]
    pub fn enterprise(
        customer_id: String,
        tier: Tier,
        expires_at: OffsetDateTime,
        features: Vec<String>,
    ) -> Self {
        Self {
            iss: SENTORI_ISSUER.to_owned(),
            sub: customer_id.clone(),
            iat: OffsetDateTime::now_utc().unix_timestamp(),
            exp: expires_at.unix_timestamp(),
            jti: Uuid::now_v7().to_string(),
            edition: Edition::Enterprise,
            tier,
            tenant_id: None,
            customer_id: Some(customer_id),
            subscription_id: None,
            features,
            limits: tier.default_limits(),
        }
    }

    /// Parsed tenant UUID, if this is a SaaS claim with a well-formed
    /// `tenant_id` field. Returns `None` for Enterprise tokens.
    #[must_use]
    pub const fn tenant_id(&self) -> Option<Uuid> {
        self.tenant_id
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]
mod tests {
    use super::*;
    use time::{Duration, macros::datetime};

    #[test]
    fn tier_serialises_kebab_case() {
        assert_eq!(serde_json::to_string(&Tier::Free).unwrap(), "\"free\"");
        assert_eq!(serde_json::to_string(&Tier::Pro).unwrap(), "\"pro\"");
        assert_eq!(
            serde_json::to_string(&Tier::EnterpriseCloud).unwrap(),
            "\"enterprise-cloud\""
        );
        assert_eq!(
            serde_json::to_string(&Tier::EnterpriseSelf).unwrap(),
            "\"enterprise-self\""
        );
    }

    #[test]
    fn edition_serialises_kebab_case() {
        assert_eq!(serde_json::to_string(&Edition::Saas).unwrap(), "\"saas\"");
        assert_eq!(
            serde_json::to_string(&Edition::Enterprise).unwrap(),
            "\"enterprise\""
        );
    }

    #[test]
    fn tier_default_limits_free_is_capped() {
        let limits = Tier::Free.default_limits();
        assert_eq!(limits.events_per_month, Some(5_000));
        assert_eq!(limits.users, Some(1));
        assert_eq!(limits.projects, Some(1));
        assert_eq!(limits.retention_days, Some(14));
    }

    #[test]
    fn tier_default_limits_pro_is_capped() {
        let limits = Tier::Pro.default_limits();
        assert_eq!(limits.events_per_month, Some(100_000));
        assert_eq!(limits.users, Some(5));
        assert_eq!(limits.projects, Some(10));
        assert_eq!(limits.retention_days, Some(30));
    }

    #[test]
    fn tier_default_limits_enterprise_is_unlimited() {
        for tier in [Tier::EnterpriseCloud, Tier::EnterpriseSelf] {
            let limits = tier.default_limits();
            assert!(limits.events_per_month.is_none());
            assert!(limits.users.is_none());
            assert!(limits.projects.is_none());
            assert!(limits.retention_days.is_none());
        }
    }

    #[test]
    fn limits_unlimited_constructor() {
        assert_eq!(LicenseLimits::unlimited(), LicenseLimits::default());
    }

    #[test]
    fn saas_claim_round_trip() {
        let tenant = Uuid::now_v7();
        let exp = datetime!(2030-01-01 0:00 UTC);
        let claim = LicenseClaims::saas_tenant(
            tenant,
            Tier::Pro,
            "sub_test".into(),
            exp,
            Duration::days(7),
        );
        assert_eq!(claim.edition, Edition::Saas);
        assert_eq!(claim.tier, Tier::Pro);
        assert_eq!(claim.tenant_id(), Some(tenant));
        assert_eq!(claim.sub, tenant.to_string());
        assert_eq!(claim.subscription_id.as_deref(), Some("sub_test"));
        assert_eq!(claim.exp, exp.unix_timestamp());
        assert!(claim.customer_id.is_none());
        assert_eq!(claim.limits, Tier::Pro.default_limits());
    }

    #[test]
    fn enterprise_claim_round_trip() {
        let exp = datetime!(2030-01-01 0:00 UTC);
        let claim = LicenseClaims::enterprise(
            "ACME-001".into(),
            Tier::EnterpriseSelf,
            exp,
            vec!["sso".into(), "white_label".into()],
        );
        assert_eq!(claim.edition, Edition::Enterprise);
        assert_eq!(claim.tier, Tier::EnterpriseSelf);
        assert!(claim.tenant_id().is_none());
        assert_eq!(claim.customer_id.as_deref(), Some("ACME-001"));
        assert!(claim.subscription_id.is_none());
        assert_eq!(claim.features.len(), 2);
        assert_eq!(claim.limits, LicenseLimits::unlimited());
    }

    #[test]
    fn json_round_trip_preserves_fields() {
        let tenant = Uuid::now_v7();
        let original = LicenseClaims::saas_tenant(
            tenant,
            Tier::Pro,
            "sub_x".into(),
            datetime!(2030-01-01 0:00 UTC),
            Duration::days(7),
        );
        let json = serde_json::to_string(&original).unwrap();
        let decoded: LicenseClaims = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn json_omits_none_optionals() {
        let claim = LicenseClaims::enterprise(
            "C".into(),
            Tier::EnterpriseSelf,
            datetime!(2030-01-01 0:00 UTC),
            vec![],
        );
        let json = serde_json::to_string(&claim).unwrap();
        assert!(!json.contains("tenant_id"));
        assert!(!json.contains("subscription_id"));
        assert!(json.contains("customer_id"));
    }

    #[test]
    fn iss_is_locked_to_constant() {
        let tenant = Uuid::now_v7();
        let claim = LicenseClaims::saas_tenant(
            tenant,
            Tier::Free,
            "s".into(),
            datetime!(2030-01-01 0:00 UTC),
            Duration::days(7),
        );
        assert_eq!(claim.iss, SENTORI_ISSUER);
    }
}
