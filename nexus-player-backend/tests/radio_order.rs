mod common;

use common::TempLibrary;
use nexus_player_backend::playlist::cursor::RadioMode;
use nexus_player_backend::playlist::radio::build_radio_order;

fn count(order: &[i64], id: i64) -> usize {
    order.iter().filter(|x| **x == id).count()
}

#[tokio::test]
async fn playlist_reached_through_two_branches_is_expanded_twice() {
    let lib = TempLibrary::new().await;
    let s_track = lib.track("shared").await;
    let a_track = lib.track("only-a").await;
    let shared = lib.playlist("S").await;
    let a = lib.playlist("A").await;
    let b = lib.playlist("B").await;
    let root = lib.playlist("root").await;
    lib.service.add_track_to_playlist(shared, s_track).await.expect("s");
    lib.service.add_track_to_playlist(a, a_track).await.expect("a track");
    lib.service.add_playlist_to_playlist(a, shared).await.expect("a->s");
    lib.service.add_playlist_to_playlist(b, shared).await.expect("b->s");
    lib.service.add_playlist_to_playlist(root, a).await.expect("root->a");
    lib.service.add_playlist_to_playlist(root, b).await.expect("root->b");

    let order = build_radio_order(&lib.service, RadioMode::Playlists)
        .await
        .expect("radio order");

    assert_eq!(count(&order, s_track.0), 2);
    assert_eq!(count(&order, a_track.0), 1);
    assert_eq!(order.len(), 3);
}

#[tokio::test]
async fn playlist_that_is_not_nested_appears_once() {
    let lib = TempLibrary::new().await;
    let t1 = lib.track("one").await;
    let t2 = lib.track("two").await;
    let p = lib.playlist("plain").await;
    lib.service.add_track_to_playlist(p, t1).await.expect("t1");
    lib.service.add_track_to_playlist(p, t2).await.expect("t2");

    let order = build_radio_order(&lib.service, RadioMode::Playlists)
        .await
        .expect("radio order");

    assert_eq!(count(&order, t1.0), 1);
    assert_eq!(count(&order, t2.0), 1);
    assert_eq!(order.len(), 2);
}
