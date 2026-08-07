//! WebPush payload encryption per RFC 8291 + RFC 8188 (aes128gcm).
//!
//! Operator concern: this is needed for the browser to actually
//! display a notification with title/body. Without encryption, the
//! Push service strips the body and the browser SW fires with
//! nothing useful to show.
//!
//! Algorithm summary (RFC 8291):
//!   1. Receive client's p256dh public key + auth secret from the
//!      SDK during subscription
//!   2. Generate ephemeral P-256 key pair (server-side, fresh per send)
//!   3. ECDH(ephemeral_priv, client_p256dh_pub) → IKM_ecdh
//!   4. HKDF-Extract(auth_secret, IKM_ecdh) → IKM
//!   5. salt = 16 random bytes
//!   6. HKDF-Extract(salt, IKM) → PRK
//!   7. HKDF-Expand(PRK, "Content-Encoding: aes128gcm" info) → CEK + nonce
//!      (CEK = 16 bytes, nonce = 12 bytes)
//!   8. AES-128-GCM encrypt(CEK, nonce, payload || 0x02)
//!      (the 0x02 padding byte marks the last record per RFC 8188)
//!   9. Body framing (RFC 8188 §2.1):
//!      salt (16) || rs (4, big-endian, default 4096) ||
//!      idlen (1) || keyid (idlen) || ciphertext
//!      keyid = the ephemeral server public key in uncompressed
//!      SEC1 form (65 bytes for P-256: 0x04 || X(32) || Y(32))

#![allow(dead_code)]

use aes_gcm::{
    Aes128Gcm,
    aead::{Aead, KeyInit, Payload},
};
use base64::Engine;
use hkdf::Hkdf;
use p256::{PublicKey, SecretKey, ecdh::diffie_hellman, elliptic_curve::sec1::ToEncodedPoint};
use rand_core::{OsRng, RngCore};
use sha2::Sha256;

pub struct EncryptedPayload {
    /// Body to POST as-is.
    pub body: Vec<u8>,
    /// `Content-Encoding: aes128gcm` literal.
    pub content_encoding: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum EncryptError {
    #[error("invalid client public key: {0}")]
    InvalidClientKey(String),
    #[error("invalid auth secret length: expected 16, got {0}")]
    InvalidAuthLen(usize),
    #[error("hkdf expand: {0}")]
    HkdfExpand(String),
    #[error("aes-gcm encrypt: {0}")]
    AesEncrypt(String),
    #[error("server key length out of range for RFC 8188 idlen: {0}")]
    InvalidServerKeyLen(usize),
}

/// Encrypt `payload` for a subscription identified by its p256dh
/// (uncompressed SEC1 base64url) and auth secret (16 bytes base64url).
pub fn encrypt(
    payload: &[u8],
    p256dh_b64url: &str,
    auth_secret_b64url: &str,
) -> Result<EncryptedPayload, EncryptError> {
    // 1. Decode client's keys.
    let p256dh_bytes =
        b64url_decode(p256dh_b64url).map_err(|e| EncryptError::InvalidClientKey(e.to_string()))?;
    let client_pub = PublicKey::from_sec1_bytes(&p256dh_bytes)
        .map_err(|e| EncryptError::InvalidClientKey(e.to_string()))?;
    let auth_secret =
        b64url_decode(auth_secret_b64url).map_err(|_e| EncryptError::InvalidAuthLen(0))?;
    if auth_secret.len() != 16 {
        return Err(EncryptError::InvalidAuthLen(auth_secret.len()));
    }

    // 2. Generate ephemeral server key pair.
    let ephemeral_priv = SecretKey::random(&mut OsRng);
    let ephemeral_pub = ephemeral_priv.public_key();
    let ephemeral_pub_sec1 = ephemeral_pub.to_encoded_point(false);
    let server_pub_bytes = ephemeral_pub_sec1.as_bytes(); // 65 bytes: 0x04 || X || Y

    // 3. ECDH shared secret (32 bytes).
    let ecdh = diffie_hellman(ephemeral_priv.to_nonzero_scalar(), client_pub.as_affine());
    let ikm_ecdh = ecdh.raw_secret_bytes();

    // 4. HKDF-Extract(auth_secret, ikm_ecdh) → IKM
    //    Info string per RFC 8291:
    //      "WebPush: info\0" || client_p256dh || server_pub
    let mut info = Vec::with_capacity(14 + 65 + 65);
    info.extend_from_slice(b"WebPush: info\x00");
    info.extend_from_slice(&p256dh_bytes);
    info.extend_from_slice(server_pub_bytes);

    let prk_key = Hkdf::<Sha256>::new(Some(&auth_secret), ikm_ecdh);
    let mut ikm = [0u8; 32];
    prk_key
        .expand(&info, &mut ikm)
        .map_err(|e| EncryptError::HkdfExpand(e.to_string()))?;

    // 5. salt = 16 random bytes.
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);

    // 6+7. HKDF-Extract+Expand to derive CEK (16) and nonce (12).
    let hkdf = Hkdf::<Sha256>::new(Some(&salt), &ikm);

    let mut cek = [0u8; 16];
    hkdf.expand(b"Content-Encoding: aes128gcm\x00", &mut cek)
        .map_err(|e| EncryptError::HkdfExpand(e.to_string()))?;

    let mut nonce = [0u8; 12];
    hkdf.expand(b"Content-Encoding: nonce\x00", &mut nonce)
        .map_err(|e| EncryptError::HkdfExpand(e.to_string()))?;

    // 8. AES-128-GCM encrypt(payload || 0x02).
    //    The 0x02 record-delimiter byte tells the browser this is
    //    the last (and only) record.
    let mut plaintext = Vec::with_capacity(payload.len() + 1);
    plaintext.extend_from_slice(payload);
    plaintext.push(0x02);

    let cipher = Aes128Gcm::new(&cek.into());
    let ciphertext = cipher
        .encrypt(
            &nonce.into(),
            Payload {
                msg: &plaintext,
                aad: &[],
            },
        )
        .map_err(|e| EncryptError::AesEncrypt(e.to_string()))?;

    // 9. Body framing per RFC 8188 §2.1.
    let rs: u32 = 4096;
    // Always 65 for an uncompressed SEC1 P-256 key; erroring rather
    // than truncating keeps a bad length from silently corrupting
    // the RFC 8188 framing.
    let key_id_len: u8 = u8::try_from(server_pub_bytes.len())
        .map_err(|_| EncryptError::InvalidServerKeyLen(server_pub_bytes.len()))?;
    let mut body = Vec::with_capacity(16 + 4 + 1 + server_pub_bytes.len() + ciphertext.len());
    body.extend_from_slice(&salt);
    body.extend_from_slice(&rs.to_be_bytes());
    body.push(key_id_len);
    body.extend_from_slice(server_pub_bytes);
    body.extend_from_slice(&ciphertext);

    Ok(EncryptedPayload {
        body,
        content_encoding: "aes128gcm",
    })
}

fn b64url_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)
}
