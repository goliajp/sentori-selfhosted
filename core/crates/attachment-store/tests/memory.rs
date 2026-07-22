//! Integration tests for [`MemoryBlobStore`].

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use sentori_attachment_store::{BlobError, BlobHash, BlobStore, MemoryBlobStore};

#[tokio::test]
async fn put_then_get_round_trip() {
    let store = MemoryBlobStore::new();
    let hash = store.put(b"hello mem").await.unwrap();
    assert_eq!(hash, BlobHash::of(b"hello mem"));
    assert_eq!(store.get(&hash).await.unwrap(), b"hello mem");
}

#[tokio::test]
async fn count_and_total_bytes_track_state() {
    let store = MemoryBlobStore::new();
    assert_eq!(store.count(), 0);
    assert_eq!(store.total_bytes(), 0);
    store.put(b"abc").await.unwrap();
    store.put(b"defgh").await.unwrap();
    assert_eq!(store.count(), 2);
    assert_eq!(store.total_bytes(), 3 + 5);
    // Same-bytes put doesn't double-count.
    store.put(b"abc").await.unwrap();
    assert_eq!(store.count(), 2);
}

#[tokio::test]
async fn delete_drops_from_map() {
    let store = MemoryBlobStore::new();
    let h = store.put(b"x").await.unwrap();
    assert_eq!(store.count(), 1);
    store.delete(&h).await.unwrap();
    assert_eq!(store.count(), 0);
    assert!(matches!(
        store.get(&h).await.unwrap_err(),
        BlobError::NotFound
    ));
}

#[tokio::test]
async fn delete_missing_is_silent() {
    let store = MemoryBlobStore::new();
    let phantom = BlobHash::of(b"never");
    store.delete(&phantom).await.expect("idempotent");
}

#[tokio::test]
async fn exists_and_len_consistent() {
    let store = MemoryBlobStore::new();
    let payload = b"sized";
    assert!(!store.exists(&BlobHash::of(payload)).await.unwrap());
    assert_eq!(store.len(&BlobHash::of(payload)).await.unwrap(), None);
    let h = store.put(payload).await.unwrap();
    assert!(store.exists(&h).await.unwrap());
    assert_eq!(store.len(&h).await.unwrap(), Some(5));
}

#[tokio::test]
async fn default_is_empty() {
    let store = MemoryBlobStore::default();
    assert_eq!(store.count(), 0);
}
