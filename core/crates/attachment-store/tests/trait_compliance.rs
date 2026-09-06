//! Parametrised trait-compliance suite — every test runs against
//! both `LocalFsBlobStore` and `MemoryBlobStore` from a single
//! body. New backends drop into the macro list at the bottom.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use sentori_attachment_store::{BlobError, BlobHash, BlobStore};

async fn put_returns_correct_hash<S: BlobStore>(store: &S) {
    let h = store.put(b"compliance").await.expect("put");
    assert_eq!(h, BlobHash::of(b"compliance"));
}

async fn put_get_round_trip<S: BlobStore>(store: &S) {
    let h = store.put(b"round-trip").await.expect("put");
    assert_eq!(store.get(&h).await.expect("get"), b"round-trip");
}

async fn put_is_idempotent<S: BlobStore>(store: &S) {
    let h1 = store.put(b"same").await.expect("first");
    let h2 = store.put(b"same").await.expect("second");
    assert_eq!(h1, h2);
}

async fn get_missing_is_not_found<S: BlobStore>(store: &S) {
    let phantom = BlobHash::of(b"never-existed-here");
    let err = store.get(&phantom).await.unwrap_err();
    assert!(matches!(err, BlobError::NotFound));
}

async fn delete_makes_subsequent_get_fail<S: BlobStore>(store: &S) {
    let h = store.put(b"will-die").await.expect("put");
    store.delete(&h).await.expect("delete");
    let err = store.get(&h).await.unwrap_err();
    assert!(matches!(err, BlobError::NotFound));
}

async fn delete_missing_is_silent<S: BlobStore>(store: &S) {
    let phantom = BlobHash::of(b"missing-target");
    store.delete(&phantom).await.expect("idempotent");
}

async fn exists_reflects_put_delete<S: BlobStore>(store: &S) {
    let h = BlobHash::of(b"flap");
    assert!(!store.exists(&h).await.unwrap());
    store.put(b"flap").await.unwrap();
    assert!(store.exists(&h).await.unwrap());
    store.delete(&h).await.unwrap();
    assert!(!store.exists(&h).await.unwrap());
}

async fn len_matches_payload_size<S: BlobStore>(store: &S) {
    let payload = b"twelve-bytes";
    let h = store.put(payload).await.expect("put");
    assert_eq!(store.len(&h).await.unwrap(), Some(payload.len() as u64));
}

async fn get_verified_passes_for_intact<S: BlobStore>(store: &S) {
    let h = store.put(b"intact").await.expect("put");
    assert_eq!(store.get_verified(&h).await.unwrap(), b"intact");
}

async fn empty_payload_round_trips<S: BlobStore>(store: &S) {
    let h = store.put(&[]).await.expect("put");
    assert_eq!(h, BlobHash::of(&[]));
    assert_eq!(store.get(&h).await.unwrap(), b"");
}

// Each backend exposes a `rig()` async fn returning a tuple
// `(guard, store)` — the guard is held for the test's lifetime
// (e.g. a TempDir keeping a backing directory alive). Memory's
// guard is `()`. The macro calls rig() per test body so backends
// stay isolated.
macro_rules! compliance_suite {
    ($mod:ident, $rig:path) => {
        mod $mod {
            #[allow(unused_imports)]
            use super::*;

            #[tokio::test]
            async fn put_returns_correct_hash() {
                let (_g, s) = $rig().await;
                super::put_returns_correct_hash(&s).await;
            }

            #[tokio::test]
            async fn put_get_round_trip() {
                let (_g, s) = $rig().await;
                super::put_get_round_trip(&s).await;
            }

            #[tokio::test]
            async fn put_is_idempotent() {
                let (_g, s) = $rig().await;
                super::put_is_idempotent(&s).await;
            }

            #[tokio::test]
            async fn get_missing_is_not_found() {
                let (_g, s) = $rig().await;
                super::get_missing_is_not_found(&s).await;
            }

            #[tokio::test]
            async fn delete_makes_subsequent_get_fail() {
                let (_g, s) = $rig().await;
                super::delete_makes_subsequent_get_fail(&s).await;
            }

            #[tokio::test]
            async fn delete_missing_is_silent() {
                let (_g, s) = $rig().await;
                super::delete_missing_is_silent(&s).await;
            }

            #[tokio::test]
            async fn exists_reflects_put_delete() {
                let (_g, s) = $rig().await;
                super::exists_reflects_put_delete(&s).await;
            }

            #[tokio::test]
            async fn len_matches_payload_size() {
                let (_g, s) = $rig().await;
                super::len_matches_payload_size(&s).await;
            }

            #[tokio::test]
            async fn get_verified_passes_for_intact() {
                let (_g, s) = $rig().await;
                super::get_verified_passes_for_intact(&s).await;
            }

            #[tokio::test]
            async fn empty_payload_round_trips() {
                let (_g, s) = $rig().await;
                super::empty_payload_round_trips(&s).await;
            }
        }
    };
}

async fn local_fs_rig() -> (
    tempfile::TempDir,
    sentori_attachment_store::LocalFsBlobStore,
) {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let store = sentori_attachment_store::LocalFsBlobStore::new(dir.path())
        .await
        .expect("store");
    (dir, store)
}

#[allow(clippy::unused_async)] // signature parity with local_fs_rig
async fn memory_rig() -> ((), sentori_attachment_store::MemoryBlobStore) {
    ((), sentori_attachment_store::MemoryBlobStore::new())
}

compliance_suite!(local_fs_backend, super::local_fs_rig);
compliance_suite!(memory_backend, super::memory_rig);
