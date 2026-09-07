//! Integration tests through the public crate surface only.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    missing_docs
)]

use std::sync::Arc;

use sentori_secrets_vault::{
    KEY_ID_MAX_LEN, KeyId, KeyIdError, MASTER_KEY_LEN, MasterKey, OpenError, Vault, peek_key_id,
};

fn vault(id: &str) -> Vault {
    Vault::new(
        MasterKey::from_bytes([0x42; MASTER_KEY_LEN]),
        KeyId::new(id).expect("ok"),
    )
}

#[test]
fn end_to_end_via_public_api() {
    let v = vault("master-v1");
    let pem = b"-----BEGIN PRIVATE KEY-----\nMIIBVQIBADANBgkq...";
    let sealed = v.seal(pem).expect("seal");
    assert_eq!(peek_key_id(&sealed), Some("master-v1"));
    let recovered = v.open(&sealed).expect("open");
    assert_eq!(recovered, pem);
}

#[test]
fn rotation_via_peek_key_id() {
    // Old vault sealed something; new vault came online with a
    // new master + id. The caller's open path uses peek_key_id
    // to route to the right vault.
    let old = vault("master-v1");
    let new = Vault::new(
        MasterKey::from_bytes([0x99; MASTER_KEY_LEN]),
        KeyId::new("master-v2").expect("ok"),
    );
    let sealed = old.seal(b"legacy secret").expect("seal");

    // Caller's rotation logic:
    let by_id: &Vault = match peek_key_id(&sealed) {
        Some("master-v2") => &new,
        Some("master-v1") => &old,
        _ => panic!("unknown key id"),
    };
    let recovered = by_id.open(&sealed).expect("open");
    assert_eq!(recovered, b"legacy secret");

    // After re-saving with the new vault, peek_key_id returns
    // the new id and only the new vault can open it.
    let resealed = new.seal(&recovered).expect("reseal");
    assert_eq!(peek_key_id(&resealed), Some("master-v2"));
    assert!(matches!(
        old.open(&resealed).expect_err("wrong id"),
        OpenError::KeyIdMismatch { .. }
    ));
}

#[test]
fn per_tenant_subkey_isolation() {
    // The K-tier `attachment-store` would derive per-tenant
    // subkeys so a leaked tenant key can't read sibling tenants.
    let master = MasterKey::from_bytes([0x42; MASTER_KEY_LEN]);
    let tenant_a = master.derive_subkey(b"tenant:acme");
    let tenant_b = master.derive_subkey(b"tenant:zenith");
    assert_ne!(tenant_a, tenant_b);

    let v_a = Vault::new(tenant_a, KeyId::new("acme-v1").expect("ok"));
    let v_b = Vault::new(tenant_b, KeyId::new("zenith-v1").expect("ok"));
    let sealed = v_a.seal(b"acme's secret").expect("seal");

    // v_b cannot open v_a's blob — KeyIdMismatch fires first
    // because the ids differ; even if the ids matched, the
    // wrapped DEK is encrypted under tenant_a, which v_b's
    // master doesn't know.
    assert!(v_b.open(&sealed).is_err());
}

#[test]
fn vault_is_send_sync_via_arc() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<Vault>();
    assert_sync::<Vault>();
    assert_send::<Arc<Vault>>();
    assert_sync::<Arc<Vault>>();
}

#[test]
fn key_id_validation_via_public_api() {
    assert!(matches!(KeyId::new(""), Err(KeyIdError::Empty)));
    assert!(matches!(
        KeyId::new("x".repeat(KEY_ID_MAX_LEN + 1)),
        Err(KeyIdError::TooLong { .. })
    ));
    assert!(matches!(
        KeyId::new("\x01"),
        Err(KeyIdError::NonPrintableAscii)
    ));
}

#[test]
fn base64_helpers_via_public_api() {
    let v = vault("master-v1");
    let sealed = v.seal_base64(b"hunter2").expect("seal");
    assert_eq!(v.open_base64(&sealed).expect("open"), b"hunter2");
    // base64 garbage must surface as Truncated, not a panic.
    assert!(matches!(
        v.open_base64("not base64!!").expect_err("garbage"),
        OpenError::Truncated
    ));
}

#[test]
fn master_key_zeroizes_on_drop_via_clone() {
    // Indirect coverage — we can't easily inspect dropped memory,
    // but we can verify Clone preserves equality (the
    // ZeroizeOnDrop derive doesn't disturb clone semantics).
    let bytes = [0xCD; MASTER_KEY_LEN];
    let m1 = MasterKey::from_bytes(bytes);
    let m2 = m1.clone();
    assert_eq!(m1, m2);
    drop(m1);
    // m2 is still usable.
    assert_eq!(m2.as_bytes(), &bytes);
}
