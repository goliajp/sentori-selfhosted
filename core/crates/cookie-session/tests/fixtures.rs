//! Integration tests through the public crate surface only —
//! catches re-export regressions and locks in wire-format
//! choices.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    missing_docs
)]

use sentori_cookie_session::{
    CsrfToken, EncryptedCookie, KEY_LEN, NONCE_LEN, PasswordHash, SIGNED_TAG_LEN, SecretKey,
    SignedCookie,
};

#[test]
fn signed_cookie_size_is_payload_plus_tag_overhead() {
    // base64-url-no-pad ratio: 4 chars per 3 bytes (rounded up).
    let key = SecretKey::from_bytes([1u8; KEY_LEN]);
    let payload = [0u8; 100];
    let sealed = SignedCookie::seal(&key, &payload);
    let raw_bytes = payload.len() + SIGNED_TAG_LEN; // 132
    let expected_b64 = raw_bytes.div_ceil(3) * 4; // 176
    // base64-url-no-pad strips the trailing `=` padding.
    let max_unpadded = expected_b64;
    let min_unpadded = expected_b64 - 2; // 0–2 chars trimmed
    assert!(
        (min_unpadded..=max_unpadded).contains(&sealed.len()),
        "sealed len {} not in [{}, {}]",
        sealed.len(),
        min_unpadded,
        max_unpadded
    );
}

#[test]
fn encrypted_cookie_size_includes_nonce_and_tag() {
    let key = SecretKey::from_bytes([1u8; KEY_LEN]);
    let payload = [0u8; 100];
    let sealed = EncryptedCookie::seal(&key, &payload).expect("seal");
    // 12 nonce + 100 ciphertext + 16 tag = 128 bytes
    // → base64-url-no-pad ≈ 171 chars.
    assert!(sealed.len() >= 170);
    assert!(sealed.len() <= 172);
}

#[test]
fn signed_then_encrypted_can_nest() {
    // The 钢筋 layer might want a signed-AND-encrypted cookie
    // (defense in depth: even if an attacker breaks one layer
    // the other still holds). Verify nesting works.
    let outer = SecretKey::from_bytes([1u8; KEY_LEN]);
    let inner = SecretKey::from_bytes([2u8; KEY_LEN]);

    let session_id = b"session-id-abc";
    let inner_sealed = SignedCookie::seal(&inner, session_id);
    let outer_sealed = EncryptedCookie::seal(&outer, inner_sealed.as_bytes()).expect("outer seal");

    // Unwrap, layer by layer.
    let middle = EncryptedCookie::open(&outer, &outer_sealed).expect("outer open");
    let middle_str = std::str::from_utf8(&middle).expect("utf8 between layers");
    let recovered = SignedCookie::open(&inner, middle_str).expect("inner open");
    assert_eq!(recovered, session_id);
}

#[test]
fn password_hash_persists_across_serialisation() {
    // Hash → store as string → re-load → verify. Simulates the
    // full DB round-trip path.
    let stored = PasswordHash::hash_with_cost("hunter2", PasswordHash::COST_MIN).expect("hash");
    // Imagine `stored` round-tripped through a `SELECT password_hash`
    // here — the bcrypt string is just text, so a simulated DB
    // round-trip is a no-op.
    assert!(PasswordHash::verify("hunter2", &stored).expect("verify"));
}

#[test]
fn csrf_token_constants_match_doc() {
    // The crate documents the CSRF token as 32 bytes / 43-char
    // base64-url-no-pad. Lock that.
    let t = CsrfToken::generate().expect("ok");
    assert_eq!(t.as_bytes().len(), 32);
    assert_eq!(t.encode().len(), 43);
}

#[test]
fn nonce_len_constant_is_aes_gcm_standard() {
    assert_eq!(NONCE_LEN, 12);
}

/// Sealed by aes-gcm 0.10.3 on 2026-08-15, under `[7u8; KEY_LEN]`.
///
/// Session cookies live in browsers, not in the database, so a
/// framing change does not corrupt anything — it signs every logged-in
/// user out at once, on deploy, with no error anyone would connect to
/// a dependency bump. Round-tripping inside one build cannot see that.
const SEALED_BY_0_10: &str =
    "up1dutg822jW7dfUR0sBxuEP4C7C_q-ugoq8iC8y4pBFFqfISAOmwbKw6lnLWSOg3gw0VAfIglR7i6IADNdKBoaek1Q";

#[test]
fn a_cookie_sealed_by_the_previous_release_still_opens() {
    let key = SecretKey::from_bytes([7u8; KEY_LEN]);
    let opened = EncryptedCookie::open(&key, SEALED_BY_0_10).unwrap();
    assert_eq!(
        opened, b"sid=01a00000-0000-7000-8000-000000000001",
        "a session cookie from the previous release no longer opens — \
         this deploy signs every user out"
    );
}
