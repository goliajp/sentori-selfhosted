//! [`Hasher`] — per-tenant per-purpose PII hasher.

use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::ALGO_VERSION_INFO;
use crate::error::{PrivacySaltError, PrivacySaltResult};

/// Minimum master secret length accepted by [`Hasher::new`].
///
/// 32 bytes = the HKDF-SHA256 output size, which is the smallest length
/// that preserves the full collision resistance of the algorithm.
pub const MIN_MASTER_SECRET_BYTES: usize = 32;

/// Length of the raw hash output in bytes (before hex encoding).
pub const OUTPUT_BYTES: usize = 32;

/// A hex-encoded 32-byte privacy hash.
///
/// Newtype around [`String`] so the hashed value is distinguishable in
/// types from arbitrary user input. Cloning is cheap; the hash itself
/// is non-secret (it's published to the DB).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash(String);

impl Hash {
    /// Hex string view (lowercase, 64 chars).
    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.0
    }

    /// Consume into the underlying hex [`String`].
    #[must_use]
    pub fn into_hex(self) -> String {
        self.0
    }
}

impl core::fmt::Display for Hash {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Hash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Privacy-preserving PII hasher.
///
/// One [`Hasher`] per process; clone if you need to move it into async
/// tasks. The master secret is materialised into the type once at
/// construction time as an HKDF prk (32 bytes) — the original input
/// slice is no longer needed after [`Hasher::new`] returns.
#[derive(Clone)]
pub struct Hasher {
    /// HKDF pseudorandom key (32 bytes, derived from master secret).
    /// Held in a [`Zeroizing`] wrapper so it's wiped on drop.
    prk: Zeroizing<[u8; 32]>,
}

impl Hasher {
    /// Build a hasher from a master secret.
    ///
    /// The secret must be at least [`MIN_MASTER_SECRET_BYTES`] long.
    /// In production this comes from a vault / KMS, *not* a constant.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacySaltError::MasterSecretTooShort`] if the secret
    /// is shorter than the minimum.
    pub fn new(master_secret: &[u8]) -> PrivacySaltResult<Self> {
        if master_secret.len() < MIN_MASTER_SECRET_BYTES {
            return Err(PrivacySaltError::MasterSecretTooShort {
                got: master_secret.len(),
                min: MIN_MASTER_SECRET_BYTES,
            });
        }
        // HKDF-Extract with no salt — the per-tenant salt is supplied
        // at hash time via the `salt` argument of HKDF-Expand instead.
        // Using `extract(None, ikm)` is the canonical "I already have a
        // strong secret, just normalise it to 32 bytes" form.
        let (prk_bytes, _) = Hkdf::<Sha256>::extract(None, master_secret);
        let mut prk = [0u8; 32];
        prk.copy_from_slice(&prk_bytes[..]);
        Ok(Self {
            prk: Zeroizing::new(prk),
        })
    }

    /// Hash `value` under `(tenant_id, purpose)`.
    ///
    /// The same `(tenant_id, purpose, value)` triple always produces
    /// the same hash; changing any of them produces an
    /// indistinguishable-from-random different hash.
    ///
    /// `purpose` is a short tag like `"email"`, `"ip"`, `"device_id"`,
    /// `"push_token"`. There is no enum because new purposes are added
    /// frequently and a typo would just produce a different hash —
    /// failure mode is data invisibility, not silent corruption.
    ///
    /// # Panics
    ///
    /// Does not panic under any caller input. The internal HMAC-SHA256
    /// keying step accepts any key length, and HKDF-Expand for 32
    /// bytes is well below the 8160-byte algorithm cap.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn hash(&self, tenant_id: Uuid, purpose: &str, value: &[u8]) -> Hash {
        let subkey = self.derive_subkey(tenant_id, purpose);
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(subkey.as_slice())
            .expect("HMAC-SHA256 accepts any key length");
        mac.update(value);
        let tag = mac.finalize().into_bytes();
        Hash(hex::encode(tag))
    }

    /// Hash `value` interpreted as UTF-8 text. Equivalent to
    /// `hash(tenant_id, purpose, value.as_bytes())`; this convenience
    /// exists because the SaaS / selfhosted call sites are virtually
    /// always hashing a [`str`].
    #[must_use]
    pub fn hash_str(&self, tenant_id: Uuid, purpose: &str, value: &str) -> Hash {
        self.hash(tenant_id, purpose, value.as_bytes())
    }

    /// Derive the 32-byte HMAC subkey for a given `(tenant, purpose)`
    /// without invoking the inner HMAC. Exposed for tests + benches
    /// (cross-pair uniqueness probing) and not part of the supported
    /// stable surface.
    #[doc(hidden)]
    #[must_use]
    pub fn subkey_for_testing(&self, tenant_id: Uuid, purpose: &str) -> [u8; 32] {
        let zeroizing = self.derive_subkey(tenant_id, purpose);
        let mut out = [0u8; 32];
        out.copy_from_slice(zeroizing.as_slice());
        out
    }

    /// Constant-time comparison of the derived subkey for two
    /// `(tenant, purpose)` pairs.
    ///
    /// Useful in tests / audits — equality of subkeys for distinct
    /// inputs would be a critical bug.
    #[must_use]
    pub fn same_subkey_as(&self, a: (Uuid, &str), b: (Uuid, &str)) -> bool {
        use subtle::ConstantTimeEq;
        let ka = self.derive_subkey(a.0, a.1);
        let kb = self.derive_subkey(b.0, b.1);
        ka.as_slice().ct_eq(kb.as_slice()).into()
    }

    #[allow(clippy::expect_used)]
    fn derive_subkey(&self, tenant_id: Uuid, purpose: &str) -> Zeroizing<[u8; 32]> {
        let prk_bytes: &[u8] = self.prk.as_ref();
        // PRK is 32 bytes (the output size of HKDF-Extract<Sha256>), so
        // from_prk's length check never fails.
        let hkdf = Hkdf::<Sha256>::from_prk(prk_bytes)
            .expect("PRK is 32 bytes from an HKDF-Extract output");

        // `info = ALGO_VERSION_INFO || purpose`
        // `salt` would normally go into Extract, but our PRK is already
        // extracted — so we cheat: pre-extract gave us a tenant-agnostic
        // PRK, and we differentiate via the `info` parameter of Expand.
        // To keep tenant separation cryptographically clean we include
        // the tenant UUID bytes in the info string as well.
        let mut info: Vec<u8> =
            Vec::with_capacity(ALGO_VERSION_INFO.len() + purpose.len() + 1 + 16);
        info.extend_from_slice(ALGO_VERSION_INFO.as_bytes());
        info.extend_from_slice(purpose.as_bytes());
        info.push(b'|');
        info.extend_from_slice(tenant_id.as_bytes());

        let mut okm = Zeroizing::new([0u8; 32]);
        // 32 bytes <<<<< HKDF-SHA256 cap of 8160 bytes; never fails.
        hkdf.expand(&info, okm.as_mut())
            .expect("32 bytes is below HKDF-SHA256's 8160-byte expand limit");
        okm
    }
}

impl core::fmt::Debug for Hasher {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never leak key material.
        f.debug_struct("Hasher").finish_non_exhaustive()
    }
}

impl Drop for Hasher {
    fn drop(&mut self) {
        // `Zeroizing<[u8; 32]>` zeroes on its own Drop; this call is
        // redundant but documents intent.
        self.prk.zeroize();
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

    fn h() -> Hasher {
        Hasher::new(&[7u8; 64]).unwrap()
    }

    #[test]
    fn new_rejects_short_secret() {
        let err = Hasher::new(&[0u8; 31]).unwrap_err();
        assert!(matches!(
            err,
            PrivacySaltError::MasterSecretTooShort { got: 31, min: 32 }
        ));
    }

    #[test]
    fn new_accepts_minimum_length_secret() {
        Hasher::new(&[0u8; 32]).unwrap();
    }

    #[test]
    fn hash_output_is_64_hex_chars() {
        let hasher = h();
        let out = hasher.hash(Uuid::nil(), "email", b"a@b");
        assert_eq!(out.as_hex().len(), 64);
        assert!(out.as_hex().chars().all(|c| c.is_ascii_hexdigit()));
        assert!(out.as_hex().chars().all(|c| !c.is_ascii_uppercase()));
    }

    #[test]
    fn hash_is_deterministic() {
        let hasher = h();
        let tenant = Uuid::now_v7();
        let a = hasher.hash(tenant, "email", b"alice@example.com");
        let b = hasher.hash(tenant, "email", b"alice@example.com");
        assert_eq!(a, b);
    }

    #[test]
    fn distinct_values_produce_distinct_hashes() {
        let hasher = h();
        let tenant = Uuid::now_v7();
        let a = hasher.hash(tenant, "email", b"alice@example.com");
        let b = hasher.hash(tenant, "email", b"bob@example.com");
        assert_ne!(a, b);
    }

    #[test]
    fn distinct_purposes_produce_distinct_hashes() {
        let hasher = h();
        let tenant = Uuid::now_v7();
        let a = hasher.hash(tenant, "email", b"x");
        let b = hasher.hash(tenant, "ip", b"x");
        assert_ne!(a, b);
    }

    #[test]
    fn distinct_tenants_produce_distinct_hashes() {
        let hasher = h();
        let t1 = Uuid::now_v7();
        let t2 = Uuid::now_v7();
        assert_ne!(t1, t2);
        let a = hasher.hash(t1, "email", b"x@y");
        let b = hasher.hash(t2, "email", b"x@y");
        assert_ne!(a, b);
    }

    #[test]
    fn distinct_master_secrets_produce_distinct_hashes() {
        let h1 = Hasher::new(&[1u8; 64]).unwrap();
        let h2 = Hasher::new(&[2u8; 64]).unwrap();
        let tenant = Uuid::nil();
        assert_ne!(
            h1.hash(tenant, "email", b"x"),
            h2.hash(tenant, "email", b"x")
        );
    }

    #[test]
    fn hash_str_matches_hash_bytes() {
        let hasher = h();
        let tenant = Uuid::now_v7();
        assert_eq!(
            hasher.hash_str(tenant, "email", "alice@x"),
            hasher.hash(tenant, "email", b"alice@x")
        );
    }

    #[test]
    fn empty_value_still_hashes() {
        let hasher = h();
        let out = hasher.hash(Uuid::nil(), "email", b"");
        assert_eq!(out.as_hex().len(), 64);
    }

    #[test]
    fn subkey_helper_round_trips() {
        let hasher = h();
        let tenant = Uuid::now_v7();
        let s1 = hasher.subkey_for_testing(tenant, "email");
        let s2 = hasher.subkey_for_testing(tenant, "email");
        assert_eq!(s1, s2);
        let s3 = hasher.subkey_for_testing(tenant, "ip");
        assert_ne!(s1, s3);
    }

    #[test]
    fn same_subkey_as_constant_time_compares() {
        let hasher = h();
        let tenant = Uuid::now_v7();
        assert!(hasher.same_subkey_as((tenant, "email"), (tenant, "email")));
        assert!(!hasher.same_subkey_as((tenant, "email"), (tenant, "ip")));
        assert!(!hasher.same_subkey_as((tenant, "email"), (Uuid::now_v7(), "email")));
    }

    #[test]
    fn hash_value_apis_round_trip() {
        let hasher = h();
        let out = hasher.hash(Uuid::nil(), "p", b"v");
        let hex = out.clone().into_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(out.as_ref(), hex.as_str());
        assert_eq!(format!("{out}"), hex);
        // ordering trait
        let out2 = hasher.hash(Uuid::nil(), "p", b"v2");
        let mut v = [out2, out];
        v.sort();
        assert!(v[0] <= v[1]);
    }

    #[test]
    fn debug_does_not_leak_key() {
        let hasher = h();
        let repr = format!("{hasher:?}");
        assert!(repr.contains("Hasher"));
        assert!(!repr.contains("prk"));
    }
}
