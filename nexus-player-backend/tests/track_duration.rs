mod common;

use common::TempLibrary;

#[tokio::test]
async fn decoded_duration_updates_only_when_different() {
    let lib = TempLibrary::new().await;
    let id = lib.track("Bandcamp search track").await;
    let before = lib.service.get_track(id).await.unwrap().unwrap();
    assert!(lib
        .service
        .update_track_duration(id, 123.456)
        .await
        .unwrap());
    let saved = lib.service.get_track(id).await.unwrap().unwrap();
    assert_eq!(saved.audio_info.unwrap().duration_secs, Some(123.456));
    assert_eq!(saved.metadata.title, before.metadata.title);
    assert!(!lib
        .service
        .update_track_duration(id, 123.456)
        .await
        .unwrap());
    assert!(lib.service.update_track_duration(id, 124.0).await.unwrap());
    for duration in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(!lib
            .service
            .update_track_duration(id, duration)
            .await
            .unwrap());
    }
    let saved = lib.service.get_track(id).await.unwrap().unwrap();
    assert_eq!(saved.audio_info.unwrap().duration_secs, Some(124.0));
}
