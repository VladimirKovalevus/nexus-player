//! The sync applier applies a whole file on one transaction: all of it or nothing.

mod common;

use common::TempLibrary;
use nexus_player_backend::library::models::{Metadata, PlaylistItemKind, Track};
use nexus_player_backend::library::{CacheStatus, TrackId, TrackSource};
use nexus_player_backend::sync::{
    SyncApplier, SyncFile, SyncPlaylist, SyncPlaylistItem, SyncTrack, SyncTrackSource, SyncType,
};

fn sync_track(name: &str, liked: bool) -> SyncTrack {
    SyncTrack {
        title: name.into(),
        artist: Some("Artist".into()),
        album: None,
        album_artist: None,
        year: None,
        genre: None,
        track_number: None,
        duration_secs: Some(100.0),
        source: SyncTrackSource::External {
            system: "bandcamp".into(),
            id: format!("sync-{name}"),
        },
        liked,
        liked_position: None,
        artwork_url: None,
        added_at: None,
    }
}

fn sync_playlist(sync_id: usize, title: &str, items: Vec<SyncPlaylistItem>) -> SyncPlaylist {
    SyncPlaylist {
        sync_id,
        title: title.into(),
        description: None,
        creator: None,
        items,
        artwork_url: None,
        created_at: None,
    }
}

fn file(sync_type: SyncType, tracks: Vec<SyncTrack>, playlists: Vec<SyncPlaylist>) -> SyncFile {
    SyncFile {
        version: 1,
        sync_type,
        created_at: 0,
        device_name: None,
        tracks,
        playlists,
    }
}

fn local_track(name: &str) -> Track {
    let now = chrono::Utc::now();
    Track {
        id: TrackId(0),
        metadata: Metadata {
            title: name.into(),
            description: None,
            artwork: None,
        },
        artist: None,
        album: None,
        album_artist: None,
        year: None,
        genre: None,
        track_number: None,
        source: TrackSource::Local(format!("/music/{name}.mp3").into()),
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

/// Tracks (id, title, liked) and playlists (id, title, items) sorted for comparison.
async fn snapshot(lib: &TempLibrary) -> (Vec<String>, Vec<String>) {
    let mut tracks: Vec<String> = lib
        .service
        .list_all_tracks_flat()
        .await
        .expect("list tracks")
        .into_iter()
        .map(|t| format!("{}|{}|{}", t.id.0, t.metadata.title, t.liked))
        .collect();
    tracks.sort();
    let mut playlists = Vec::new();
    for p in lib.service.get_all_playlists().await.expect("playlists") {
        let full = lib
            .service
            .get_playlist_with_items(p.id)
            .await
            .expect("load playlist")
            .expect("playlist exists");
        let items: Vec<String> = full
            .items
            .iter()
            .map(|i| {
                let kind = match i.item_kind {
                    PlaylistItemKind::Track => "t",
                    PlaylistItemKind::Playlist => "p",
                };
                format!("{kind}{}@{}", i.item_id, i.position)
            })
            .collect();
        playlists.push(format!(
            "{}|{}|{}",
            p.id.0,
            p.metadata.title,
            items.join(",")
        ));
    }
    playlists.sort();
    (tracks, playlists)
}

async fn track_titles(lib: &TempLibrary) -> Vec<String> {
    let mut v: Vec<String> = lib
        .service
        .list_all_tracks_flat()
        .await
        .expect("list tracks")
        .into_iter()
        .map(|t| t.metadata.title)
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn full_sync_replaces_streaming_data_keeps_local_tracks_and_orders_items() {
    let lib = TempLibrary::new().await;
    lib.track("old streaming").await;
    lib.playlist("old playlist").await;
    lib.service
        .add_track(local_track("mine"))
        .await
        .expect("add local");

    let f = file(
        SyncType::Full,
        vec![
            sync_track("a", false),
            sync_track("b", true),
            sync_track("c", false),
        ],
        vec![
            sync_playlist(
                0,
                "outer",
                vec![
                    SyncPlaylistItem::Track { track_index: 2 },
                    SyncPlaylistItem::Playlist { sync_id: 1 },
                    SyncPlaylistItem::Track { track_index: 0 },
                ],
            ),
            sync_playlist(1, "inner", vec![SyncPlaylistItem::Track { track_index: 1 }]),
        ],
    );
    SyncApplier::new(&lib.service)
        .apply(f)
        .await
        .expect("apply full");

    assert_eq!(track_titles(&lib).await, vec!["a", "b", "c", "mine"]);
    let playlists = lib.service.get_all_playlists().await.expect("playlists");
    assert_eq!(playlists.len(), 2, "old playlist replaced");
    let outer = playlists
        .iter()
        .find(|p| p.metadata.title == "outer")
        .expect("outer");
    let inner = playlists
        .iter()
        .find(|p| p.metadata.title == "inner")
        .expect("inner");
    let tracks = lib.service.list_all_tracks_flat().await.expect("tracks");
    let id_of = |t: &str| {
        tracks
            .iter()
            .find(|x| x.metadata.title == t)
            .expect("track")
            .id
            .0
    };

    let loaded = lib
        .service
        .get_playlist_with_items(outer.id)
        .await
        .expect("load")
        .expect("exists");
    let got: Vec<(PlaylistItemKind, i64)> = loaded
        .items
        .iter()
        .map(|i| (i.item_kind, i.item_id))
        .collect();
    assert_eq!(
        got,
        vec![
            (PlaylistItemKind::Track, id_of("c")),
            (PlaylistItemKind::Playlist, inner.id.0),
            (PlaylistItemKind::Track, id_of("a")),
        ]
    );
}

#[tokio::test]
async fn diff_sync_adds_missing_tracks_and_playlists_and_updates_liked() {
    let lib = TempLibrary::new().await;
    let existing = lib.track("kept").await;
    // Same source as the library's helper track, now liked in the sync file.
    let mut liked = sync_track("kept", true);
    liked.source = SyncTrackSource::External {
        system: "bandcamp".into(),
        id: "lib-kept".into(),
    };
    let f = file(
        SyncType::Diff,
        vec![liked, sync_track("new", false)],
        vec![sync_playlist(
            0,
            "added",
            vec![
                SyncPlaylistItem::Track { track_index: 0 },
                SyncPlaylistItem::Track { track_index: 1 },
            ],
        )],
    );
    SyncApplier::new(&lib.service)
        .apply(f)
        .await
        .expect("apply diff");

    let tracks = lib.service.list_all_tracks_flat().await.expect("tracks");
    assert_eq!(tracks.len(), 2, "existing track is not duplicated");
    let kept = tracks.iter().find(|t| t.id == existing).expect("kept");
    assert!(kept.liked);
    let playlists = lib.service.get_all_playlists().await.expect("playlists");
    assert_eq!(playlists.len(), 1);
    let loaded = lib
        .service
        .get_playlist_with_items(playlists[0].id)
        .await
        .expect("load")
        .expect("exists");
    assert_eq!(loaded.items.len(), 2);
}

#[tokio::test]
async fn full_sync_failing_midway_leaves_the_library_unchanged() {
    let lib = TempLibrary::new().await;
    lib.track("old streaming").await;
    let old = lib.playlist("old playlist").await;
    let t = lib.track("in playlist").await;
    lib.service
        .add_track_to_playlist(old, t)
        .await
        .expect("fill old playlist");
    lib.service
        .add_track(local_track("mine"))
        .await
        .expect("add local");
    let before = snapshot(&lib).await;

    // Two playlists containing each other: the second nested add_item fails the cycle check.
    let f = file(
        SyncType::Full,
        vec![sync_track("a", false)],
        vec![
            sync_playlist(
                0,
                "x",
                vec![
                    SyncPlaylistItem::Track { track_index: 0 },
                    SyncPlaylistItem::Playlist { sync_id: 1 },
                ],
            ),
            sync_playlist(1, "y", vec![SyncPlaylistItem::Playlist { sync_id: 0 }]),
        ],
    );
    let result = SyncApplier::new(&lib.service).apply(f).await;
    assert!(result.is_err(), "cyclic sync file must fail");
    assert_eq!(snapshot(&lib).await, before);
}

#[tokio::test]
async fn diff_sync_failing_midway_leaves_the_library_unchanged() {
    let lib = TempLibrary::new().await;
    lib.track("existing").await;
    let before = snapshot(&lib).await;

    let f = file(
        SyncType::Diff,
        vec![sync_track("a", true)],
        vec![
            sync_playlist(0, "x", vec![SyncPlaylistItem::Playlist { sync_id: 1 }]),
            sync_playlist(1, "y", vec![SyncPlaylistItem::Playlist { sync_id: 0 }]),
        ],
    );
    let result = SyncApplier::new(&lib.service).apply(f).await;
    assert!(result.is_err(), "cyclic sync file must fail");
    assert_eq!(snapshot(&lib).await, before);
}
