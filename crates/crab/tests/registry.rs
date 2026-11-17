use berm_host::{Images, Stored};
#[tokio::test]
async fn local_images_are_kept_and_read_by_verified_digest() {
    let root = tempfile::tempdir().unwrap();
    let stored = Stored::open(root.path());
    let bytes = b"an image stored locally";
    let digest = stored.put(bytes).await.unwrap();
    assert!(stored.holds(&digest).await);
    let at = "alice/weather:1.0.0".parse().unwrap();
    stored.bind(&at, &digest).await.unwrap();
    let reopened = Stored::open(root.path());
    assert_eq!(reopened.resolve(&at).await.unwrap(), Some(digest.clone()));
    assert_eq!(
        reopened
            .resolve(&"alice/weather".parse().unwrap())
            .await
            .unwrap(),
        Some(digest.clone())
    );
    assert_eq!(
        stored.pull("alice/weather:1.0.0", &digest).await.unwrap(),
        bytes
    );
    assert!(
        stored
            .pull("alice/weather", "sha256:missing")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_local_version_cannot_be_rebound_by_concurrent_pulls() {
    let root = tempfile::tempdir().unwrap();
    let stored = Stored::open(root.path());
    let first = stored.put(b"first image").await.unwrap();
    let second = stored.put(b"second image").await.unwrap();
    let at = "alice/weather:1.0.0".parse().unwrap();
    let (a, b) = tokio::join!(stored.bind(&at, &first), stored.bind(&at, &second));
    assert_ne!(a.is_ok(), b.is_ok());
    let kept = stored.resolve(&at).await.unwrap().unwrap();
    assert_eq!(kept, if a.is_ok() { first } else { second });
    stored.bind(&at, &kept).await.unwrap();
}
