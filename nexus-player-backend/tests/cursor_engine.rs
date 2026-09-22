mod common;

use std::sync::Arc;

use common::{track_item, FakeResolver, TempLibrary, VecSource};
use nexus_player_backend::playlist::cursor::{
    ContainerContext, CursorItem, CursorSource, LocalPlaylistSource, PlayerCursor,
};

fn local(id: i64) -> CursorItem {
    CursorItem::Container(ContainerContext::LocalPlaylist(id))
}

fn cursor(root: Vec<CursorItem>, resolver: &Arc<FakeResolver>) -> PlayerCursor {
    PlayerCursor::new(Box::new(VecSource::new(root)), resolver.clone())
}

async fn drain(cursor: &mut PlayerCursor) -> Vec<String> {
    let mut titles = Vec::new();
    while let Some(track) = cursor.next_track().await.expect("next_track") {
        titles.push(track.title);
    }
    titles
}

fn titles(numbers: &[usize]) -> Vec<String> {
    numbers.iter().map(|n| format!("Title {n}")).collect()
}

fn resolved_ids(resolver: &FakeResolver) -> Vec<i64> {
    resolver
        .resolved_contexts()
        .into_iter()
        .filter_map(|c| match c {
            ContainerContext::LocalPlaylist(id) => Some(id),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn nested_playlist_is_not_resolved_before_the_cursor_reaches_it() {
    let resolver = FakeResolver::new();
    resolver.serve_local_playlist(10, vec![track_item(10), track_item(11)]);
    let mut cursor = cursor(vec![track_item(1), track_item(2), local(10)], &resolver);

    assert!(resolved_ids(&resolver).is_empty());
    assert_eq!(
        cursor.next_track().await.expect("next").map(|t| t.title),
        Some("Title 1".into())
    );
    assert_eq!(
        cursor.next_track().await.expect("next").map(|t| t.title),
        Some("Title 2".into())
    );
    assert!(
        resolved_ids(&resolver).is_empty(),
        "the nested playlist was resolved while tracks before it were still playing"
    );

    assert_eq!(
        cursor.next_track().await.expect("next").map(|t| t.title),
        Some("Title 10".into())
    );
    assert_eq!(resolved_ids(&resolver), vec![10]);
}

#[tokio::test]
async fn container_plays_all_its_tracks_before_the_next_sibling() {
    let resolver = FakeResolver::new();
    resolver.serve_local_playlist(10, vec![track_item(10), track_item(11), track_item(12)]);
    let mut cursor = cursor(vec![local(10), track_item(2)], &resolver);

    assert_eq!(drain(&mut cursor).await, titles(&[10, 11, 12, 2]));
}

#[tokio::test]
async fn three_nested_levels_play_in_order() {
    let resolver = FakeResolver::new();
    resolver.serve_local_playlist(10, vec![track_item(10), local(20), track_item(11)]);
    resolver.serve_local_playlist(20, vec![track_item(20), local(30), track_item(21)]);
    resolver.serve_local_playlist(30, vec![track_item(30)]);
    let mut cursor = cursor(vec![track_item(1), local(10), track_item(2)], &resolver);

    assert_eq!(
        drain(&mut cursor).await,
        titles(&[1, 10, 20, 30, 21, 11, 2])
    );
}

#[tokio::test]
async fn empty_container_is_skipped_without_ending_playback() {
    let resolver = FakeResolver::new();
    resolver.serve_local_playlist(10, vec![]);
    let mut cursor = cursor(vec![track_item(1), local(10), track_item(2)], &resolver);

    assert_eq!(drain(&mut cursor).await, titles(&[1, 2]));
}

#[tokio::test]
async fn playlist_reachable_through_two_branches_plays_twice() {
    let resolver = FakeResolver::new();
    resolver.serve_local_playlist(1, vec![local(2), local(3)]);
    resolver.serve_local_playlist(2, vec![local(4)]);
    resolver.serve_local_playlist(3, vec![local(4)]);
    resolver.serve_local_playlist(4, vec![track_item(40), track_item(41)]);
    let mut cursor = cursor(vec![local(1)], &resolver);

    assert_eq!(drain(&mut cursor).await, titles(&[40, 41, 40, 41]));
}

#[tokio::test]
async fn local_playlist_source_yields_nested_playlist_as_container_in_order() {
    let lib = TempLibrary::new().await;
    let t1 = lib.track("one").await;
    let t2 = lib.track("two").await;
    let t3 = lib.track("three").await;
    let child = lib.playlist("child").await;
    let root = lib.playlist("root").await;
    lib.service
        .add_track_to_playlist(child, t3)
        .await
        .expect("child track");
    lib.service
        .add_track_to_playlist(root, t1)
        .await
        .expect("t1");
    lib.service
        .add_playlist_to_playlist(root, child)
        .await
        .expect("nest");
    lib.service
        .add_track_to_playlist(root, t2)
        .await
        .expect("t2");

    let mut source = LocalPlaylistSource::new(lib.service.clone(), root);
    let mut items = Vec::new();
    loop {
        let batch = source.next_batch(10).await.expect("batch");
        if batch.is_empty() {
            break;
        }
        items.extend(batch);
    }

    assert_eq!(items.len(), 3, "child tracks must not be inlined");
    match &items[0] {
        CursorItem::Track(t) => assert_eq!(t.title, "one"),
        other => panic!("expected track, got {other:?}"),
    }
    match &items[1] {
        CursorItem::Container(ContainerContext::LocalPlaylist(id)) => assert_eq!(*id, child.0),
        other => panic!("expected nested playlist container, got {other:?}"),
    }
    match &items[2] {
        CursorItem::Track(t) => assert_eq!(t.title, "two"),
        other => panic!("expected track, got {other:?}"),
    }
}
