//! License JWT verification side — runs inside deployed Sentori binaries.
//!
//! The [`Verifier`] holds an Ed25519 public key and exposes
//! [`Verifier::verify`], which validates signature + `exp` + `iss` + the
//! [`RevocationList`] in one pass and returns parsed [`LicenseClaims`].
//!
//! ## Why issuer is enforced even though `jsonwebtoken` already
//! checks it
//!
//! `jsonwebtoken::Validation::set_issuer` rejects tokens whose `iss`
//! claim does not appear in the supplied set, but it reports the
//! failure as a generic `InvalidIssuer` lumped together with other
//! claim-validation faults. Sentori treats issuer mismatch as a
//! distinct security event ([`LicenseError::IssuerMismatch`]) so
//! middleware can log + alert on it separately.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use ed25519_dalek::VerifyingKey;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, errors::ErrorKind};

use crate::claims::LicenseClaims;
use crate::error::{LicenseError, LicenseResult};
use crate::{SENTORI_ISSUER, error::LicenseError as LE};

/// Clock-skew tolerance applied to `exp` validation. 30 s matches the
/// Sentori SDK ingest clock skew budget so a token accepted by the
/// dashboard cannot be rejected by an ingest node started a few seconds
/// later.
pub const DEFAULT_LEEWAY_SECS: u64 = 30;

/// Shared, mutable set of revoked `jti` strings.
///
/// Cheap to clone (`Arc` under the hood). [`Verifier`] holds one and
/// the calling crate (sentori-saas / enterprise) wires it to a database
/// loader at boot plus a Valkey pub/sub channel for multi-instance
/// broadcast — see `S5` / `S17` for the lifecycle plan.
#[derive(Debug, Clone, Default)]
pub struct RevocationList {
    inner: Arc<RwLock<HashSet<String>>>,
}

impl RevocationList {
    /// Empty revocation list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // `FromIterator` is implemented below so callers can use
    // `RevocationList::from_iter(...)` and `.collect()` interchangeably.

    /// Mark a `jti` as revoked. Idempotent.
    ///
    /// # Panics
    ///
    /// Panics only if the lock is poisoned — i.e. another thread
    /// panicked while holding the write lock. Treat that as a bug.
    pub fn revoke(&self, jti: impl Into<String>) {
        let mut guard = self
            .inner
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.insert(jti.into());
    }

    /// `true` if the given `jti` has been revoked.
    #[must_use]
    pub fn contains(&self, jti: &str) -> bool {
        let guard = self
            .inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.contains(jti)
    }

    /// Number of revoked entries (useful for metrics / startup logs).
    #[must_use]
    pub fn len(&self) -> usize {
        let guard = self
            .inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.len()
    }

    /// `true` when no entries are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<S: Into<String>> FromIterator<S> for RevocationList {
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        let set: HashSet<String> = iter.into_iter().map(Into::into).collect();
        Self {
            inner: Arc::new(RwLock::new(set)),
        }
    }
}

/// Verifies license JWTs and enforces the revocation list.
///
/// Clone-cheap; one verifier instance is typically shared across all
/// axum handlers via state.
#[derive(Clone)]
pub struct Verifier {
    decoding_key: DecodingKey,
    validation: Validation,
    revocations: RevocationList,
}

impl Verifier {
    /// Construct a verifier with default leeway and an empty revocation
    /// list.
    #[must_use]
    pub fn new(verifying_key: VerifyingKey) -> Self {
        Self::with_components(verifying_key, RevocationList::new(), DEFAULT_LEEWAY_SECS)
    }

    /// Construct a verifier with an externally managed revocation list.
    /// Use this when wiring multi-instance Valkey broadcast.
    #[must_use]
    pub fn with_revocations(verifying_key: VerifyingKey, revocations: RevocationList) -> Self {
        Self::with_components(verifying_key, revocations, DEFAULT_LEEWAY_SECS)
    }

    /// Lowest-level constructor — full control over leeway and the
    /// revocation list. Prefer the simpler constructors above unless
    /// you specifically need to override leeway.
    #[must_use]
    pub fn with_components(
        verifying_key: VerifyingKey,
        revocations: RevocationList,
        leeway_secs: u64,
    ) -> Self {
        let raw = verifying_key.to_bytes();
        let decoding_key = DecodingKey::from_ed_der(&raw);

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_issuer(&[SENTORI_ISSUER]);
        validation.set_required_spec_claims(&["exp", "iss", "sub"]);
        validation.validate_exp = true;
        validation.leeway = leeway_secs;

        Self {
            decoding_key,
            validation,
            revocations,
        }
    }

    /// Accessor for the shared revocation list (e.g. for the Stripe
    /// webhook handler to call `revoke`).
    #[must_use]
    pub const fn revocations(&self) -> &RevocationList {
        &self.revocations
    }

    /// Verify a license JWT.
    ///
    /// Performs in order:
    /// 1. signature validity (Ed25519)
    /// 2. `iss` equals [`SENTORI_ISSUER`]
    /// 3. `exp` is in the future (with leeway)
    /// 4. `jti` is not in the [`RevocationList`]
    ///
    /// # Errors
    ///
    /// - [`LicenseError::Expired`] — `exp` is in the past (after leeway)
    /// - [`LicenseError::IssuerMismatch`] — `iss` does not match
    /// - [`LicenseError::Revoked`] — `jti` is in the revocation list
    /// - [`LicenseError::Invalid`] — any other malformed-token cause
    ///   (bad signature, malformed JSON, missing required claim, wrong
    ///   algorithm, etc.)
    pub fn verify(&self, token: &str) -> LicenseResult<LicenseClaims> {
        let data =
            decode::<LicenseClaims>(token, &self.decoding_key, &self.validation).map_err(|e| {
                match e.kind() {
                    ErrorKind::ExpiredSignature => LE::Expired,
                    ErrorKind::InvalidIssuer => LE::IssuerMismatch,
                    _ => LicenseError::Invalid(e.to_string()),
                }
            })?;

        if self.revocations.contains(&data.claims.jti) {
            return Err(LicenseError::Revoked);
        }
        Ok(data.claims)
    }
}

impl core::fmt::Debug for Verifier {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Verifier")
            .field("revocations_len", &self.revocations.len())
            .field("leeway_secs", &self.validation.leeway)
            .finish_non_exhaustive()
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
    use ed25519_dalek::SigningKey;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use time::{Duration, OffsetDateTime};
    use uuid::Uuid;

    use crate::claims::{Edition, LicenseClaims, Tier};
    use crate::issuer::Issuer;

    fn keypair(seed: u64) -> (SigningKey, VerifyingKey) {
        let mut rng = StdRng::seed_from_u64(seed);
        let sk = SigningKey::generate(&mut rng);
        let vk = sk.verifying_key();
        (sk, vk)
    }

    fn fresh_claim(tenant: Uuid, exp: OffsetDateTime) -> LicenseClaims {
        LicenseClaims::saas_tenant(tenant, Tier::Pro, "sub_x".into(), exp, Duration::days(7))
    }

    #[test]
    fn verify_round_trip_recovers_all_fields() {
        let (sk, vk) = keypair(1);
        let issuer = Issuer::new(sk).unwrap();
        let verifier = Verifier::new(vk);

        let tenant = Uuid::now_v7();
        let claims = fresh_claim(tenant, OffsetDateTime::now_utc() + Duration::days(30));
        let token = issuer.sign(&claims).unwrap();

        let decoded = verifier.verify(&token).unwrap();
        assert_eq!(decoded.tenant_id(), Some(tenant));
        assert_eq!(decoded.edition, Edition::Saas);
        assert_eq!(decoded.tier, Tier::Pro);
        assert_eq!(decoded.jti, claims.jti);
        assert_eq!(decoded.iss, SENTORI_ISSUER);
    }

    #[test]
    fn verify_rejects_expired() {
        let (sk, vk) = keypair(2);
        let issuer = Issuer::new(sk).unwrap();
        let verifier = Verifier::new(vk);

        // 5 minutes in the past — well beyond the 30 s leeway.
        let claims = fresh_claim(
            Uuid::now_v7(),
            OffsetDateTime::now_utc() - Duration::minutes(5),
        );
        let token = issuer.sign(&claims).unwrap();

        assert!(matches!(
            verifier.verify(&token).unwrap_err(),
            LicenseError::Expired
        ));
    }

    #[test]
    fn verify_accepts_token_within_leeway() {
        let (sk, vk) = keypair(2);
        let issuer = Issuer::new(sk).unwrap();
        let verifier = Verifier::new(vk);

        // 10 s in the past, within DEFAULT_LEEWAY_SECS = 30.
        let claims = fresh_claim(
            Uuid::now_v7(),
            OffsetDateTime::now_utc() - Duration::seconds(10),
        );
        let token = issuer.sign(&claims).unwrap();

        verifier.verify(&token).unwrap();
    }

    #[test]
    fn verify_rejects_wrong_key() {
        let (sk1, _) = keypair(3);
        let (_, vk2) = keypair(4);
        let issuer = Issuer::new(sk1).unwrap();
        let verifier = Verifier::new(vk2);

        let claims = fresh_claim(
            Uuid::now_v7(),
            OffsetDateTime::now_utc() + Duration::days(30),
        );
        let token = issuer.sign(&claims).unwrap();

        assert!(matches!(
            verifier.verify(&token).unwrap_err(),
            LicenseError::Invalid(_)
        ));
    }

    #[test]
    fn verify_rejects_wrong_issuer() {
        let (sk, vk) = keypair(5);
        let issuer = Issuer::new(sk).unwrap();
        let verifier = Verifier::new(vk);

        let mut claims = fresh_claim(
            Uuid::now_v7(),
            OffsetDateTime::now_utc() + Duration::days(30),
        );
        claims.iss = "evil.example.com".into();
        let token = issuer.sign(&claims).unwrap();

        assert!(matches!(
            verifier.verify(&token).unwrap_err(),
            LicenseError::IssuerMismatch
        ));
    }

    #[test]
    fn verify_rejects_revoked_jti() {
        let (sk, vk) = keypair(6);
        let issuer = Issuer::new(sk).unwrap();
        let revs = RevocationList::new();
        let verifier = Verifier::with_revocations(vk, revs.clone());

        let claims = fresh_claim(
            Uuid::now_v7(),
            OffsetDateTime::now_utc() + Duration::days(30),
        );
        let token = issuer.sign(&claims).unwrap();

        // Accepted before revocation.
        verifier.verify(&token).unwrap();

        revs.revoke(claims.jti);
        assert!(matches!(
            verifier.verify(&token).unwrap_err(),
            LicenseError::Revoked
        ));
    }

    #[test]
    fn verify_rejects_garbage_token() {
        let (_, vk) = keypair(7);
        let verifier = Verifier::new(vk);
        assert!(matches!(
            verifier.verify("not.a.token").unwrap_err(),
            LicenseError::Invalid(_)
        ));
        assert!(matches!(
            verifier.verify("totally garbage").unwrap_err(),
            LicenseError::Invalid(_)
        ));
        assert!(matches!(
            verifier.verify("").unwrap_err(),
            LicenseError::Invalid(_)
        ));
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        let (sk, vk) = keypair(8);
        let issuer = Issuer::new(sk).unwrap();
        let verifier = Verifier::new(vk);

        let claims = fresh_claim(
            Uuid::now_v7(),
            OffsetDateTime::now_utc() + Duration::days(30),
        );
        let token = issuer.sign(&claims).unwrap();

        // Flip one byte in the payload segment.
        let mut parts: Vec<&str> = token.split('.').collect();
        let payload = parts[1].to_owned();
        let mut bytes = payload.into_bytes();
        bytes[0] = if bytes[0] == b'A' { b'B' } else { b'A' };
        let new_payload =
            String::from_utf8(bytes).expect("payload remained ascii after single-byte flip");
        parts[1] = new_payload.as_str();
        let tampered = parts.join(".");

        assert!(matches!(
            verifier.verify(&tampered).unwrap_err(),
            LicenseError::Invalid(_)
        ));
    }

    #[test]
    fn revocation_list_apis() {
        let revs: RevocationList = ["a", "b"].into_iter().collect();
        assert_eq!(revs.len(), 2);
        assert!(!revs.is_empty());
        assert!(revs.contains("a"));
        assert!(!revs.contains("c"));
        revs.revoke("c");
        assert!(revs.contains("c"));
        assert_eq!(revs.len(), 3);

        // Re-revoke is idempotent.
        revs.revoke("c");
        assert_eq!(revs.len(), 3);

        let empty = RevocationList::new();
        assert!(empty.is_empty());
    }

    #[test]
    fn verifier_debug_includes_metadata_not_key() {
        let (_, vk) = keypair(9);
        let verifier = Verifier::new(vk);
        let repr = format!("{verifier:?}");
        assert!(repr.contains("Verifier"));
        assert!(repr.contains("revocations_len"));
        assert!(repr.contains("leeway_secs"));
        assert!(!repr.contains("decoding_key"));
    }

    #[test]
    fn verifier_clone_shares_revocations() {
        let (_, vk) = keypair(10);
        let v1 = Verifier::new(vk);
        let v2 = v1.clone();
        v1.revocations().revoke("shared-jti");
        assert!(v2.revocations().contains("shared-jti"));
    }

    #[test]
    fn enterprise_token_round_trip() {
        let (sk, vk) = keypair(11);
        let issuer = Issuer::new(sk).unwrap();
        let verifier = Verifier::new(vk);

        let claims = LicenseClaims::enterprise(
            "ACME-001".into(),
            Tier::EnterpriseSelf,
            OffsetDateTime::now_utc() + Duration::days(365),
            vec!["sso".into()],
        );
        let token = issuer.sign(&claims).unwrap();
        let decoded = verifier.verify(&token).unwrap();
        assert_eq!(decoded.edition, Edition::Enterprise);
        assert_eq!(decoded.customer_id.as_deref(), Some("ACME-001"));
        assert_eq!(decoded.features, vec!["sso".to_owned()]);
    }
}
