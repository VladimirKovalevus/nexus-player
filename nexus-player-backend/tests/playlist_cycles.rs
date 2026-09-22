//! Nested playlists never form a cycle, even when two inserts race.

mod common;

use std::collections::HashSet;
use std::sync::Arc;

use common::TempLibrary;
use nexus_player_backend::library::{LibraryService, PlaylistId};
use tokio::sync::Barrier;

/// Every playlist reachable below `root`, failing the test if a path returns to itself.
async fn assert_acyclic(service: &LibraryService, root: PlaylistId) {
    let mut path = HashSet::new();
    walk(service, root, &mut path).await;
}

fn walk<'a>(
    service: &'a LibraryService,
    id: PlaylistId,
    path: &'a mut HashSet<i64>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        assert!(path.insert(id.0), "cycle through playlist {}", id.0);
        let with_items = service
            .get_playlist_with_items(id)
            .await
            .expect("load playlist")
            .expect("playlist exists");
        for item in with_items.items {
            if item.item_kind == nexus_player_backend::library::models::PlaylistItemKind::Playlist {
                walk(service, PlaylistId(item.item_id), path).await;
            }
        }
        path.remove(&id.0);
    })
}

#[tokio::test]
async fn adding_a_playlist_to_itself_fails() {
    let lib = TempLibrary::new().await;
    let a = lib.playlist("a").await;
    assert!(lib.service.add_playlist_to_playlist(a, a).await.is_err());
}

#[tokio::test]
async fn adding_the_parent_under_its_child_fails() {
    let lib = TempLibrary::new().await;
    let a = lib.playlist("a").await;
    let b = lib.playlist("b").await;
    lib.service
        .add_playlist_to_playlist(a, b)
        .await
        .expect("a->b");
    let err = lib
        .service
        .add_playlist_to_playlist(b, a)
        .await
        .expect_err("b->a is a cycle");
    assert!(err.to_string().contains("would create a cycle"), "{err}");
    assert_acyclic(&lib.service, a).await;
}

#[tokio::test]
async fn transitive_cycle_is_rejected() {
    let lib = TempLibrary::new().await;
    let a = lib.playlist("a").await;
    let b = lib.playlist("b").await;
    let c = lib.playlist("c").await;
    lib.service
        .add_playlist_to_playlist(a, b)
        .await
        .expect("a->b");
    lib.service
        .add_playlist_to_playlist(b, c)
        .await
        .expect("b->c");
    assert!(lib.service.add_playlist_to_playlist(c, a).await.is_err());
    assert_acyclic(&lib.service, a).await;
}

#[tokio::test]
async fn diamond_is_not_a_cycle() {
    let lib = TempLibrary::new().await;
    let a = lib.playlist("a").await;
    let b = lib.playlist("b").await;
    let c = lib.playlist("c").await;
    let d = lib.playlist("d").await;
    for (p, ch) in [(a, b), (a, c), (b, d), (c, d)] {
        lib.service
            .add_playlist_to_playlist(p, ch)
            .await
            .expect("diamond edge");
    }
    assert_acyclic(&lib.service, a).await;
}

#[tokio::test]
async fn adding_tracks_is_unaffected() {
    let lib = TempLibrary::new().await;
    let a = lib.playlist("a").await;
    let t1 = lib.track("one").await;
    let t2 = lib.track("two").await;
    lib.service.add_track_to_playlist(a, t1).await.expect("t1");
    lib.service.add_track_to_playlist(a, t2).await.expect("t2");
    let loaded = lib
        .service
        .get_playlist_with_items(a)
        .await
        .expect("load")
        .expect("exists");
    assert_eq!(loaded.items.len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_opposite_inserts_admit_exactly_one() {
    let lib = TempLibrary::new().await;
    for round in 0..8 {
        let a = lib.playlist(&format!("a{round}")).await;
        let b = lib.playlist(&format!("b{round}")).await;
        let barrier = Arc::new(Barrier::new(2));

        let s1 = lib.service.clone();
        let b1 = barrier.clone();
        let t1 = tokio::spawn(async move {
            b1.wait().await;
            s1.add_playlist_to_playlist(a, b).await
        });
        let s2 = lib.service.clone();
        let b2 = barrier.clone();
        let t2 = tokio::spawn(async move {
            b2.wait().await;
            s2.add_playlist_to_playlist(b, a).await
        });

        let r1 = t1.await.expect("join");
        let r2 = t2.await.expect("join");
        let oks = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
        assert_eq!(oks, 1, "round {round}: {r1:?} / {r2:?}");
        for r in [&r1, &r2].into_iter().filter_map(|r| r.as_ref().err()) {
            assert!(
                r.to_string().contains("would create a cycle"),
                "round {round}: unexpected error {r}"
            );
        }
        assert_acyclic(&lib.service, a).await;
        assert_acyclic(&lib.service, b).await;
    }
}
