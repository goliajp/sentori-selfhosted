//! License JWT issuance side (GOLIA-internal).
//!
//! The [`Issuer`] holds an Ed25519 signing key and signs
//! [`LicenseClaims`] into compact JWT strings consumable by
//! [`crate::Verifier`].
//!
//! ## Key management
//!
//! Production code constructs an [`Issuer`] once at process boot from a
//! key loaded out of a vault or KMS — *never* from a key generated
//! per-process. Test code uses [`SigningKey::generate`].
//!
//! The issuer never exposes its private key; the only outward operation
//! is [`Issuer::sign`].

use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

use crate::claims::LicenseClaims;
use crate::error::{LicenseError, LicenseResult};

/// Signs license JWTs using Ed25519.
///
/// Clone-cheap (internally just a wrapper around the prepared
/// [`EncodingKey`] bytes) but treat instances as singletons in
/// production — they hold the private key in memory.
#[derive(Clone)]
pub struct Issuer {
    encoding_key: EncodingKey,
}

impl Issuer {
    /// Build an issuer from a [`SigningKey`].
    ///
    /// The key is immediately re-encoded into the PKCS#8 DER form
    /// `jsonwebtoken` requires; the original [`SigningKey`] is consumed
    /// so callers don't accidentally retain it past Issuer construction.
    ///
    /// # Errors
    ///
    /// Returns [`LicenseError::Signing`] if PKCS#8 encoding fails. In
    /// practice this cannot happen for a valid in-memory `SigningKey`
    /// — the error path exists only because `pkcs8` returns a `Result`.
    // Take by value so callers cannot accidentally retain the
    // SigningKey past Issuer construction. The body extracts and drops
    // it; the consumption is intentional even though clippy can't see
    // it through `to_pkcs8_der(&self)`.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(signing_key: SigningKey) -> LicenseResult<Self> {
        let pkcs8_der = signing_key
            .to_pkcs8_der()
            .map_err(|e| LicenseError::Signing(format!("pkcs8 encode: {e}")))?;
        let encoding_key = EncodingKey::from_ed_der(pkcs8_der.as_bytes());
        Ok(Self { encoding_key })
    }

    /// Sign the given claims and return the compact JWT string.
    ///
    /// # Errors
    ///
    /// Returns [`LicenseError::Signing`] if JSON serialisation of the
    /// claims or the signing operation itself fails. Callers should
    /// treat any error here as a server-side bug (500), never as a
    /// client-recoverable condition.
    pub fn sign(&self, claims: &LicenseClaims) -> LicenseResult<String> {
        let header = Header::new(Algorithm::EdDSA);
        encode(&header, claims, &self.encoding_key)
            .map_err(|e| LicenseError::Signing(format!("jwt encode: {e}")))
    }
}

impl core::fmt::Debug for Issuer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never leak key material via Debug.
        f.debug_struct("Issuer").finish_non_exhaustive()
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

    use crate::claims::{LicenseClaims, Tier};

    fn deterministic_key(seed: u64) -> SigningKey {
        let mut rng = StdRng::seed_from_u64(seed);
        SigningKey::generate(&mut rng)
    }

    #[test]
    fn issuer_produces_three_segment_token() {
        let issuer = Issuer::new(deterministic_key(1)).unwrap();
        let claims = LicenseClaims::saas_tenant(
            Uuid::now_v7(),
            Tier::Pro,
            "sub".into(),
            OffsetDateTime::now_utc() + Duration::days(30),
            Duration::days(7),
        );
        let token = issuer.sign(&claims).unwrap();
        assert_eq!(
            token.split('.').count(),
            3,
            "JWT must have header.payload.signature"
        );
    }

    #[test]
    fn issuer_signatures_are_deterministic_per_key() {
        let issuer = Issuer::new(deterministic_key(42)).unwrap();
        let tenant = Uuid::nil();
        let exp = OffsetDateTime::from_unix_timestamp(2_000_000_000).unwrap();
        let mut claims =
            LicenseClaims::saas_tenant(tenant, Tier::Free, "sub".into(), exp, Duration::days(7));
        claims.iat = 1_000_000_000;
        claims.jti = "fixed-jti".into();
        let a = issuer.sign(&claims).unwrap();
        let b = issuer.sign(&claims).unwrap();
        assert_eq!(a, b, "Ed25519 is deterministic for identical input");
    }

    #[test]
    fn distinct_keys_produce_distinct_signatures() {
        let i1 = Issuer::new(deterministic_key(1)).unwrap();
        let i2 = Issuer::new(deterministic_key(2)).unwrap();
        let mut claims = LicenseClaims::saas_tenant(
            Uuid::nil(),
            Tier::Free,
            "s".into(),
            OffsetDateTime::from_unix_timestamp(2_000_000_000).unwrap(),
            Duration::days(7),
        );
        claims.iat = 1_000_000_000;
        claims.jti = "j".into();
        assert_ne!(i1.sign(&claims).unwrap(), i2.sign(&claims).unwrap());
    }

    #[test]
    fn issuer_debug_does_not_leak_key() {
        let issuer = Issuer::new(deterministic_key(0)).unwrap();
        let repr = format!("{issuer:?}");
        assert!(repr.contains("Issuer"));
        assert!(!repr.contains("encoding_key"));
    }

    #[test]
    fn issuer_is_clone() {
        let issuer = Issuer::new(deterministic_key(0)).unwrap();
        let cloned = issuer.clone();
        let claims = LicenseClaims::enterprise(
            "c".into(),
            Tier::EnterpriseSelf,
            OffsetDateTime::now_utc() + Duration::days(30),
            vec![],
        );
        assert_eq!(issuer.sign(&claims).unwrap(), cloned.sign(&claims).unwrap());
    }
}
