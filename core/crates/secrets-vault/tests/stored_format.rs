//! What is already in the database must still open.
//!
//! `seal`/`open` round-tripping inside one build proves the pair is
//! self-consistent and nothing else. The question that matters when a
//! crypto dependency moves is different: does a blob written by the
//! *previous* build still decrypt? Nothing asked that until aes-gcm
//! 0.10 → 0.11, when the answer had to be obtained by checking out
//! the old tree and running it.
//!
//! So the answer is frozen here. The constant below was produced by
//! aes-gcm 0.10.3 / hkdf 0.12 / sha2 0.10 and must keep opening for
//! as long as any stored credential predates the current build.
//!
//! If this test fails after a dependency bump, stored push
//! credentials are unreadable in production. That is not a test to
//! relax — it is a migration to write.

#![allow(clippy::unwrap_used, missing_docs)]

use sentori_secrets_vault::{KeyId, MasterKey, Vault};

/// Sealed by aes-gcm 0.10.3 on 2026-08-15.
const SEALED_BY_0_10: &str = "AQJrMY7kqI6BEBj5Ms85qM9Yyu5EDiRqwC7QsRNa9WK9ORCVQAOUL9I6glz2OJb0Z7Nsla0NNKvpcajnH_0b6vdBtBlCedqY98KwJ-x60ZFa03gKe_dy6hCLk38NWBupadhhE5sgn29-";

#[test]
fn an_envelope_written_by_the_previous_release_still_opens() {
    let vault = Vault::new(MasterKey::from_bytes([0x42; 32]), KeyId::new("k1").unwrap());
    let opened = vault.open_base64(SEALED_BY_0_10).unwrap();
    assert_eq!(
        opened, b"apns-p8-bytes",
        "an envelope sealed by the previous release no longer opens — \
         every stored credential is unreadable"
    );
}
