//! Property tests for the envelope round-trip + tampering
//! invariants.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    missing_docs
)]

use proptest::prelude::*;

use sentori_secrets_vault::{KEY_ID_MAX_LEN, KeyId, MasterKey, OpenError, Vault, peek_key_id};

fn vault_strategy() -> impl Strategy<Value = Vault> {
    (
        prop::array::uniform32(any::<u8>()),
        // Printable ASCII, 1-32 bytes — covers the realistic
        // human-readable space without overshooting.
        prop::string::string_regex("[A-Za-z0-9._-]{1,32}").expect("regex"),
    )
        .prop_map(|(bytes, id)| {
            Vault::new(MasterKey::from_bytes(bytes), KeyId::new(id).expect("ok"))
        })
}

fn payload_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..1024)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    /// `Vault::open(seal(p)) == Ok(p)` always.
    #[test]
    fn seal_open_round_trips(
        vault in vault_strategy(),
        payload in payload_strategy(),
    ) {
        let sealed = vault.seal(&payload).expect("seal");
        let opened = vault.open(&sealed).expect("open");
        prop_assert_eq!(opened, payload);
    }

    /// `peek_key_id` always returns the vault's id for blobs we
    /// just sealed with it.
    #[test]
    fn peek_recovers_vault_key_id(
        vault in vault_strategy(),
        payload in payload_strategy(),
    ) {
        let sealed = vault.seal(&payload).expect("seal");
        prop_assert_eq!(peek_key_id(&sealed), Some(vault.key_id().as_str()));
    }

    /// Two seals of the same (vault, payload) produce distinct
    /// ciphertexts — the per-seal DEK + nonces guarantee this.
    #[test]
    fn distinct_seals_yield_distinct_ciphertexts(
        vault in vault_strategy(),
        payload in payload_strategy(),
    ) {
        // Empty payload still differs because the wrapped DEK
        // + the two nonces are all per-call random.
        let a = vault.seal(&payload).expect("seal");
        let b = vault.seal(&payload).expect("seal");
        prop_assert_ne!(a, b);
    }

    /// Tampering any single byte after the version+key_id header
    /// always causes the open to fail.
    #[test]
    fn tampering_any_byte_after_header_breaks_open(
        vault in vault_strategy(),
        payload in payload_strategy(),
        offset in 0usize..512,
    ) {
        let mut sealed = vault.seal(&payload).expect("seal");
        // Header is 2 + key_id_len bytes; skip past it. Then
        // tamper byte (offset % remaining).
        let header_len = 2 + vault.key_id().as_bytes().len();
        let payload_len = sealed.len() - header_len;
        prop_assume!(payload_len > 0);
        let target = header_len + (offset % payload_len);
        sealed[target] ^= 0x01;

        // Either WrappedDekDecryptFailed or PayloadDecryptFailed
        // — both are fail-closed outcomes. (Tampering the
        // wrapped_dek_nonce or wrapped_dek section fails the
        // first AEAD check; tampering the payload section fails
        // the second.)
        let err = vault.open(&sealed).expect_err("tampered");
        prop_assert!(matches!(
            err,
            OpenError::WrappedDekDecryptFailed | OpenError::PayloadDecryptFailed
        ));
    }

    /// Opening with a different master fails with
    /// `WrappedDekDecryptFailed`.
    #[test]
    fn wrong_master_fails_distinctly(
        bytes_a in prop::array::uniform32(any::<u8>()),
        bytes_b in prop::array::uniform32(any::<u8>()),
        id in "[A-Za-z0-9._-]{1,32}",
        payload in payload_strategy(),
    ) {
        prop_assume!(bytes_a != bytes_b);
        let key_id = KeyId::new(id).expect("ok");
        let v1 = Vault::new(MasterKey::from_bytes(bytes_a), key_id.clone());
        let v2 = Vault::new(MasterKey::from_bytes(bytes_b), key_id);
        let sealed = v1.seal(&payload).expect("seal");
        let err = v2.open(&sealed).expect_err("wrong master");
        prop_assert!(matches!(err, OpenError::WrappedDekDecryptFailed));
    }

    /// `KeyId::new` accepts iff input is 1-255 printable ASCII.
    #[test]
    fn key_id_length_invariant(
        len in 0usize..512,
        ch in 0x20u8..=0x7E,
    ) {
        let s: String = std::iter::repeat_n(ch as char, len).collect();
        let result = KeyId::new(s);
        if (1..=KEY_ID_MAX_LEN).contains(&len) {
            prop_assert!(result.is_ok());
        } else {
            prop_assert!(result.is_err());
        }
    }

    /// HKDF subkey derivation is deterministic + distinct per
    /// info string.
    #[test]
    fn subkey_derivation_invariants(
        bytes in prop::array::uniform32(any::<u8>()),
        info_a in prop::collection::vec(any::<u8>(), 1..32),
        info_b in prop::collection::vec(any::<u8>(), 1..32),
    ) {
        let master = MasterKey::from_bytes(bytes);
        let a1 = master.derive_subkey(&info_a);
        let a2 = master.derive_subkey(&info_a);
        prop_assert_eq!(&a1, &a2, "same info → same key");
        if info_a != info_b {
            let b = master.derive_subkey(&info_b);
            prop_assert_ne!(&a1, &b, "distinct info → distinct keys");
        }
    }
}
