use super::*;

async fn service() -> (LibraryService, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let service = LibraryService::new(&dir.path().join("test.db"))
        .await
        .unwrap();
    (service, dir)
}

#[tokio::test]
async fn known_artwork_urls_finds_only_saved_covers() {
    let (service, _guard) = service().await;

    let saved = "https://i1.sndcdn.com/artworks-abc-large.jpg";
    let unsaved = "https://i1.sndcdn.com/artworks-xyz-large.jpg";
    service.ensure_artwork_url(saved).await.unwrap();

    let known = service
        .known_artwork_urls(&[saved.to_string(), unsaved.to_string()])
        .await
        .unwrap();

    assert!(known.contains(saved), "a saved cover must be recognised");
    assert!(
        !known.contains(unsaved),
        "a search-result cover must not be recognised"
    );
    assert_eq!(known.len(), 1);
}

/// The lookup rebuilds the same `source_data` string the writer stored, so the
/// two must agree on URLs that are awkward to embed in JSON.
#[tokio::test]
async fn known_artwork_urls_matches_the_writers_encoding() {
    let (service, _guard) = service().await;

    let awkward = [
        "https://f4.bcbits.com/img/a123_0.jpg?a=1&b=2",
        "https://example.com/cover with spaces.jpg",
        "https://example.com/café/über/歌.jpg",
    ];
    for url in awkward {
        service.ensure_artwork_url(url).await.unwrap();
    }

    let urls: Vec<String> = awkward.iter().map(|u| u.to_string()).collect();
    let known = service.known_artwork_urls(&urls).await.unwrap();

    assert_eq!(
        known.len(),
        awkward.len(),
        "round-trip lost a URL: {known:?}"
    );
}

#[tokio::test]
async fn known_artwork_urls_handles_an_empty_batch() {
    let (service, _guard) = service().await;
    assert!(service.known_artwork_urls(&[]).await.unwrap().is_empty());
}
