//! Library writes grouped on one transaction commit or roll back as a whole.

mod common;

use std::future::Future;
use std::time::Duration;

use common::TempLibrary;
use nexus_player_backend::library::models::{Metadata, Playlist, PlaylistItemKind, Track};
use nexus_player_backend::library::{CacheStatus, PlaylistId, TrackId, TrackSource};

/// Fails the test instead of hanging when a step waits on a pool connection.
async fn step<T>(what: &str, fut: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), fut)
        .await
        .unwrap_or_else(|_| panic!("step timed out: {what}"))
}

fn source(name: &str) -> TrackSource {
    TrackSource::External {
        system: "bandcamp".into(),
        id: format!("txn-{name}"),
    }
}

fn track(name: &str) -> Track {
    let now = chrono::Utc::now();
    Track {
        id: TrackId(0),
        metadata: Metadata {
            title: name.into(),
            description: None,
            artwork: None,
        },
        artist: Some("Artist".into()),
        album: None,
        album_artist: None,
        year: None,
        genre: None,
        track_number: None,
        source: source(name),
        cache: CacheStatus::None,
        audio_info: None,
        play_count: 0,
        last_played: None,
        added_at: now,
        updated_at: now,
        liked: false,
        liked_position: None,
        unavailable: false,
    }
}

fn playlist(name: &str) -> Playlist {
    let now = chrono::Utc::now();
    Playlist {
        id: PlaylistId(0),
        metadata: Metadata {
            title: name.into(),
            description: None,
            artwork: None,
        },
        creator: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn dropping_the_write_handle_leaves_the_library_unchanged() {
    let lib = TempLibrary::new().await;
    let existing = lib.track("existing").await;
    let existing_playlist = lib.playlist("existing playlist").await;

    {
        let txn = step("begin", lib.service.begin_write())
            .await
            .expect("begin write");
        let t1 = step("add t1", txn.add_track(track("t1"))).await.expect("t1");
        step("add t2", txn.add_track(track("t2"))).await.expect("t2");
        let p1 = step("create p1", txn.create_playlist(playlist("p1")))
            .await
            .expect("p1");
        let p2 = step("create p2", txn.create_playlist(playlist("p2")))
            .await
            .expect("p2");
        step(
            "track item",
            txn.add_item_to_playlist(p1, PlaylistItemKind::Track, t1.0),
        )
        .await
        .expect("track item");
        step(
            "playlist item",
            txn.add_item_to_playlist(p1, PlaylistItemKind::Playlist, p2.0),
        )
        .await
        .expect("playlist item");
        let found = step("find", txn.find_by_source(&source("t1")))
            .await
            .expect("find")
            .expect("t1 is visible inside the transaction");
        step("like", txn.toggle_liked(found.id)).await.expect("like");
        assert_eq!(
            step("list inside", txn.list_all_tracks())
                .await
                .expect("list")
                .len(),
            3
        );
    }

    let tracks = step("list", lib.service.list_all_tracks())
        .await
        .expect("list tracks");
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].id, existing);
    let playlists = step("playlists", lib.service.get_all_playlists())
        .await
        .expect("list playlists");
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].id, existing_playlist);
}

#[tokio::test]
async fn committed_writes_are_visible_in_item_order() {
    let lib = TempLibrary::new().await;

    let txn = step("begin", lib.service.begin_write())
        .await
        .expect("begin write");
    let t1 = txn.add_track(track("t1")).await.expect("t1");
    txn.add_track(track("t2")).await.expect("t2");
    let p1 = txn.create_playlist(playlist("p1")).await.expect("p1");
    let p2 = txn.create_playlist(playlist("p2")).await.expect("p2");
    step(
        "track item",
        txn.add_item_to_playlist(p1, PlaylistItemKind::Track, t1.0),
    )
    .await
    .expect("track item");
    step(
        "playlist item",
        txn.add_item_to_playlist(p1, PlaylistItemKind::Playlist, p2.0),
    )
    .await
    .expect("playlist item");
    let found = txn
        .find_by_source(&source("t1"))
        .await
        .expect("find")
        .expect("t1 exists");
    txn.toggle_liked(found.id).await.expect("like");
    step("commit", txn.commit()).await.expect("commit");

    let tracks = step("list", lib.service.list_all_tracks_flat())
        .await
        .expect("list tracks");
    assert_eq!(tracks.len(), 2);
    let liked = lib
        .service
        .find_by_source(&source("t1"))
        .await
        .expect("find")
        .expect("t1 persisted");
    assert!(liked.liked);

    let with_items = lib
        .service
        .get_playlist_with_items(p1)
        .await
        .expect("load playlist")
        .expect("p1 persisted");
    let items: Vec<_> = with_items
        .items
        .iter()
        .map(|i| (i.item_kind, i.item_id))
        .collect();
    assert_eq!(
        items,
        vec![
            (PlaylistItemKind::Track, t1.0),
            (PlaylistItemKind::Playlist, p2.0)
        ]
    );
    assert_eq!(lib.service.get_all_playlists().await.expect("all").len(), 2);
}

#[tokio::test]
async fn a_cycle_through_the_handle_fails_and_the_handle_stays_usable() {
    let lib = TempLibrary::new().await;

    let txn = step("begin", lib.service.begin_write())
        .await
        .expect("begin write");
    let a = txn.create_playlist(playlist("a")).await.expect("a");
    let b = txn.create_playlist(playlist("b")).await.expect("b");
    step(
        "a->b",
        txn.add_item_to_playlist(a, PlaylistItemKind::Playlist, b.0),
    )
    .await
    .expect("a->b");

    let err = step(
        "b->a",
        txn.add_item_to_playlist(b, PlaylistItemKind::Playlist, a.0),
    )
    .await
    .expect_err("b->a is a cycle");
    assert!(err.to_string().contains("would create a cycle"), "{err}");

    // The failed insert must not poison the transaction.
    let t = txn.add_track(track("after")).await.expect("add after error");
    step("a->track", txn.add_item_to_playlist(a, PlaylistItemKind::Track, t.0))
        .await
        .expect("track after error");
    step("commit", txn.commit()).await.expect("commit");

    let a_items = lib
        .service
        .get_playlist_with_items(a)
        .await
        .expect("load a")
        .expect("a persisted");
    assert_eq!(a_items.items.len(), 2);
    let b_items = lib
        .service
        .get_playlist_with_items(b)
        .await
        .expect("load b")
        .expect("b persisted");
    assert!(b_items.items.is_empty());
}
