//! Integration tests for [`LocalFsBlobStore`].

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::missing_panics_doc
)]

use sentori_attachment_store::{BlobError, BlobHash, BlobStore, LocalFsBlobStore};
use tempfile::TempDir;

async fn store() -> (TempDir, LocalFsBlobStore) {
    let dir = TempDir::new().expect("tempdir");
    let store = LocalFsBlobStore::new(dir.path()).await.expect("store");
    (dir, store)
}

#[tokio::test]
async fn put_then_get_round_trip() {
    let (_dir, store) = store().await;
    let hash = store.put(b"hello world").await.expect("put");
    let back = store.get(&hash).await.expect("get");
    assert_eq!(back, b"hello world");
    assert_eq!(hash, BlobHash::of(b"hello world"));
}

#[tokio::test]
async fn get_missing_is_not_found() {
    let (_dir, store) = store().await;
    let phantom = BlobHash::of(b"never written");
    let err = store.get(&phantom).await.unwrap_err();
    assert!(matches!(err, BlobError::NotFound));
}

#[tokio::test]
async fn exists_true_after_put_false_before() {
    let (_dir, store) = store().await;
    let hash = BlobHash::of(b"payload");
    assert!(!store.exists(&hash).await.unwrap());
    store.put(b"payload").await.unwrap();
    assert!(store.exists(&hash).await.unwrap());
}

#[tokio::test]
async fn len_returns_size_or_none() {
    let (_dir, store) = store().await;
    let hash = BlobHash::of(b"abcdef");
    assert_eq!(store.len(&hash).await.unwrap(), None);
    store.put(b"abcdef").await.unwrap();
    assert_eq!(store.len(&hash).await.unwrap(), Some(6));
}

#[tokio::test]
async fn delete_then_gone() {
    let (_dir, store) = store().await;
    let hash = store.put(b"deletable").await.unwrap();
    assert!(store.exists(&hash).await.unwrap());
    store.delete(&hash).await.unwrap();
    assert!(!store.exists(&hash).await.unwrap());
    let err = store.get(&hash).await.unwrap_err();
    assert!(matches!(err, BlobError::NotFound));
}

#[tokio::test]
async fn delete_missing_is_silent() {
    let (_dir, store) = store().await;
    let phantom = BlobHash::of(b"never seen");
    store.delete(&phantom).await.expect("idempotent delete");
}

#[tokio::test]
async fn put_is_idempotent_same_bytes_same_hash() {
    let (_dir, store) = store().await;
    let h1 = store.put(b"twice").await.unwrap();
    let h2 = store.put(b"twice").await.unwrap();
    assert_eq!(h1, h2);
    assert_eq!(store.get(&h1).await.unwrap(), b"twice");
}

#[tokio::test]
async fn fanout_path_layout() {
    let (dir, store) = store().await;
    let hash = store.put(b"layout-check").await.unwrap();
    let hex = hash.to_hex();
    let expected = dir
        .path()
        .join(&hex[..2])
        .join(format!("{}.bin", &hex[2..]));
    assert!(
        tokio::fs::try_exists(&expected).await.unwrap(),
        "expected blob at {}",
        expected.display()
    );
}

#[tokio::test]
async fn get_verified_passes_for_intact_blob() {
    let (_dir, store) = store().await;
    let hash = store.put(b"intact").await.unwrap();
    let bytes = store.get_verified(&hash).await.unwrap();
    assert_eq!(bytes, b"intact");
}

#[tokio::test]
async fn get_verified_catches_corruption() {
    let (dir, store) = store().await;
    let hash = store.put(b"original").await.unwrap();
    // Overwrite the on-disk bytes with different content (same
    // path — simulating disk corruption / tampering).
    let hex = hash.to_hex();
    let path = dir
        .path()
        .join(&hex[..2])
        .join(format!("{}.bin", &hex[2..]));
    tokio::fs::write(&path, b"corrupted").await.unwrap();
    let err = store.get_verified(&hash).await.unwrap_err();
    assert!(matches!(err, BlobError::HashMismatch));
    // The unverified get still returns the corrupted bytes.
    assert_eq!(store.get(&hash).await.unwrap(), b"corrupted");
}

#[tokio::test]
async fn empty_payload_round_trips() {
    let (_dir, store) = store().await;
    let hash = store.put(&[]).await.unwrap();
    assert_eq!(hash, BlobHash::of(&[]));
    assert_eq!(store.get(&hash).await.unwrap(), b"");
}

#[tokio::test]
async fn new_unchecked_works_when_dir_exists() {
    let dir = TempDir::new().unwrap();
    let store = LocalFsBlobStore::new_unchecked(dir.path());
    let hash = store.put(b"unchecked").await.unwrap();
    assert_eq!(store.get(&hash).await.unwrap(), b"unchecked");
}

#[tokio::test]
async fn root_accessor_returns_configured_path() {
    let dir = TempDir::new().unwrap();
    let store = LocalFsBlobStore::new(dir.path()).await.unwrap();
    assert_eq!(store.root(), dir.path());
}
