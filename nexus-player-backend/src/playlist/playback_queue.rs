use crate::audio::types::Track;
use crate::playlist::cursor::{ContainerContext, PlayerCursor};
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

pub struct CursorQueue {
    pub history: Vec<Track>,
    pub current_track: Option<Track>,
    pub cursor: Option<PlayerCursor>,
    pub manual_queue: std::collections::VecDeque<Track>,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub context: Option<ContainerContext>,
}

use anyhow::Result;
mod search_queue;

impl Default for CursorQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorQueue {
    pub fn toggle_shuffle(&mut self) {
        self.shuffle_enabled = !self.shuffle_enabled;
        tracing::info!("CursorQueue shuffle toggled to {}", self.shuffle_enabled);
    }

    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            current_track: None,
            cursor: None,
            manual_queue: std::collections::VecDeque::new(),
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            context: None,
        }
    }

    pub async fn next(&mut self) -> Result<Option<Track>> {
        // Handle Repeat One: just return the current track if it exists
        if self.repeat_mode == RepeatMode::One {
            if let Some(track) = &self.current_track {
                return Ok(Some(track.clone()));
            }
        }

        // 1. Save current track to history before moving to next
        if let Some(track) = self.current_track.take() {
            self.history.push(track);
        }

        // 2. Try manual queue
        if !self.manual_queue.is_empty() {
            let track = if self.shuffle_enabled {
                // Pick a random track from manual queue
                let mut rng = thread_rng();
                let idx = rng.gen_range(0..self.manual_queue.len());
                self.manual_queue.remove(idx)
            } else {
                self.manual_queue.pop_front()
            };
            if let Some(track) = track {
                self.current_track = Some(track.clone());
                return Ok(Some(track));
            }
        }

        // 3. Fetch from cursor if available
        if self.cursor.is_some() {
            if self.shuffle_enabled {
                // If shuffle is enabled and manual queue is empty,
                // prefetch a batch so we have something to shuffle
                let _ = self.prefetch(50).await?;
                if !self.manual_queue.is_empty() {
                    let mut rng = thread_rng();
                    let idx = rng.gen_range(0..self.manual_queue.len());
                    if let Some(track) = self.manual_queue.remove(idx) {
                        self.current_track = Some(track.clone());
                        return Ok(Some(track));
                    }
                }
            }

            if let Some(cursor) = &mut self.cursor {
                let track = cursor.next_track().await?;
                self.current_track = track.clone();
                return Ok(track);
            }
        }

        Ok(None)
    }

    pub fn previous(&mut self) -> Option<Track> {
        // Handle Repeat One: just return the current track
        if self.repeat_mode == RepeatMode::One {
            if let Some(track) = &self.current_track {
                return Some(track.clone());
            }
        }

        let prev = self.history.pop()?;

        // Push current track back to the front of manual queue to allow "going forward" again
        if let Some(current) = self.current_track.take() {
            self.manual_queue.push_front(current);
        }

        self.current_track = Some(prev.clone());
        Some(prev)
    }

    pub fn peek_next(&self) -> Option<Track> {
        if let Some(track) = self.manual_queue.front() {
            return Some(track.clone());
        }
        None
    }

    pub fn peek_upcoming_tracks(&self, limit: usize) -> Vec<Track> {
        self.manual_queue.iter().take(limit).cloned().collect()
    }

    pub fn current(&self) -> Option<&Track> {
        self.current_track.as_ref()
    }

    pub fn add_track(&mut self, track: Track) {
        self.manual_queue.push_back(track);
    }

    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        // Index is relative to snapshot: [history] + [current] + [manual]
        let history_len = self.history.len();
        let current_len = if self.current_track.is_some() { 1 } else { 0 };

        if index < history_len {
            Some(self.history.remove(index))
        } else if index < history_len + current_len {
            self.current_track.take()
        } else {
            let manual_idx = index - history_len - current_len;
            self.manual_queue.remove(manual_idx)
        }
    }

    /// Jump to a specific index in the snapshot view, rearranging history/current/manual_queue.
    pub fn jump_to(&mut self, index: usize) {
        let (tracks, current_idx) = self.snapshot();
        if index >= tracks.len() || Some(index) == current_idx {
            return;
        }
        self.history = tracks[..index].to_vec();
        self.current_track = Some(tracks[index].clone());
        self.manual_queue = tracks[index + 1..].to_vec().into();
        // cursor stays for future prefetching when manual_queue runs out
    }

    pub fn set_cursor(&mut self, cursor: PlayerCursor) {
        tracing::info!("Setting cursor in queue");
        self.cursor = Some(cursor);
    }

    pub async fn prefetch(&mut self, limit: usize) -> Result<usize> {
        let mut count = 0;
        if let Some(cursor) = &mut self.cursor {
            while count < limit {
                if let Some(track) = cursor.next_track().await? {
                    self.manual_queue.push_back(track);
                    count += 1;
                } else {
                    break;
                }
            }
        }
        Ok(count)
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.current_track = None;
        self.cursor = None;
        self.manual_queue.clear();
        // Must be reset too: a stale context would make Repeat::All resurrect the old
        // source, and would misreport the active radio session to the UI.
        self.context = None;
    }

    /// Creates a flat list view of the queue for the UI.
    /// Returns (all_tracks, current_index)
    pub fn snapshot(&self) -> (Vec<Track>, Option<usize>) {
        let mut tracks = Vec::with_capacity(self.history.len() + 1 + self.manual_queue.len());

        tracks.extend(self.history.iter().cloned());

        let current_index = if let Some(current) = &self.current_track {
            let idx = tracks.len();
            tracks.push(current.clone());
            Some(idx)
        } else {
            None
        };

        tracks.extend(self.manual_queue.iter().cloned());

        (tracks, current_index)
    }
}

// === Search Queue Types ===

/// Type of container in search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerKind {
    Album,
    Playlist,
}

/// Metadata for a container (album or playlist)
#[derive(Debug, Clone)]
pub struct ContainerMeta {
    pub kind: ContainerKind,
    pub id: String,
    pub system: String,
    pub title: String,
    pub artist: String,
    pub track_count: usize,
    pub thumbnail: Option<String>,
}

/// An item in the search queue
#[derive(Debug, Clone)]
pub enum SearchQueueItem {
    Track(Track),
    Container {
        info: ContainerMeta,
        expanded_tracks: Option<Vec<Track>>,
        play_index: usize,
    },
}

/// Result of navigation in the search queue
pub enum NextResult {
    Track(Track),
    NeedExpand(usize),
    NeedMorePages,
    End,
}

/// Position within the search queue for UI
#[derive(Debug, Clone)]
pub struct CurrentPosition {
    pub item_index: usize,
    pub track_within_container: Option<usize>,
}

/// Entry for the UI snapshot
#[derive(Debug, Clone)]
pub enum QueueEntry {
    Track(Track),
    Container {
        info: ContainerMeta,
        tracks: Option<Vec<Track>>,
        play_index: usize,
    },
}

/// Search queue that preserves the full search context
pub struct SearchQueue {
    pub system: String,
    pub query: String,
    items: Vec<SearchQueueItem>,
    current_index: Option<usize>,
    pub search_offset: usize,
    pub has_more_pages: bool,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub shuffle_order: Vec<usize>,
}

/// Unified queue type
pub enum ActiveQueue {
    Cursor(Box<CursorQueue>),
    Search(SearchQueue),
}

impl ActiveQueue {
    pub fn toggle_shuffle(&mut self) {
        match self {
            ActiveQueue::Cursor(cq) => cq.toggle_shuffle(),
            ActiveQueue::Search(sq) => sq.toggle_shuffle(),
        }
    }

    pub fn new_cursor() -> Self {
        ActiveQueue::Cursor(Box::default())
    }

    pub fn new_cursor_with_settings(repeat_mode: RepeatMode, shuffle_enabled: bool) -> Self {
        let mut cq = CursorQueue::new();
        cq.repeat_mode = repeat_mode;
        cq.shuffle_enabled = shuffle_enabled;
        ActiveQueue::Cursor(Box::new(cq))
    }
    pub fn peek_next(&self) -> Option<Track> {
        match self {
            ActiveQueue::Cursor(o) => o.peek_next(),
            ActiveQueue::Search(o) => o.peek_next(),
        }
    }

    pub fn peek_upcoming_tracks(&self, limit: usize) -> Vec<Track> {
        match self {
            ActiveQueue::Cursor(o) => o.peek_upcoming_tracks(limit),
            ActiveQueue::Search(o) => o.peek_upcoming_tracks(limit),
        }
    }

    pub fn current_track(&self) -> Option<Track> {
        match self {
            ActiveQueue::Cursor(o) => o.current().cloned(),
            ActiveQueue::Search(o) => o.current_track(),
        }
    }
}

#[cfg(test)]
mod search_queue_tests {
    use super::*;
    use crate::audio::types::{TrackSource, TrackState};

    // --- fixtures ---

    fn track(id: usize) -> Track {
        Track {
            id,
            title: format!("Track {}", id),
            artist: None,
            album: None,
            duration: None,
            artwork_url: None,
            format: None,
            source: TrackSource::local(format!("/track{}.mp3", id)),
            state: TrackState::Saved,
            liked: false,
        }
    }

    fn container_meta(title: &str) -> ContainerMeta {
        ContainerMeta {
            kind: ContainerKind::Album,
            id: title.to_string(),
            system: "test".to_string(),
            title: title.to_string(),
            artist: "artist".to_string(),
            track_count: 0,
            thumbnail: None,
        }
    }

    fn track_item(id: usize) -> SearchQueueItem {
        SearchQueueItem::Track(track(id))
    }

    fn container_item(title: &str, expanded: Option<Vec<Track>>) -> SearchQueueItem {
        SearchQueueItem::Container {
            info: container_meta(title),
            expanded_tracks: expanded,
            play_index: 0,
        }
    }

    fn queue_with(items: Vec<SearchQueueItem>) -> SearchQueue {
        SearchQueue::new("sys".into(), "q".into(), items, false)
    }

    // --- new() ---

    #[test]
    fn new_sets_search_offset_to_item_count() {
        let sq = queue_with(vec![track_item(1), track_item(2)]);
        assert_eq!(sq.search_offset, 2);
    }

    #[test]
    fn new_has_no_current() {
        let sq = queue_with(vec![track_item(1)]);
        assert!(sq.current_track().is_none());
    }

    #[test]
    fn new_reflects_has_more_pages() {
        let sq = SearchQueue::new("s".into(), "q".into(), vec![], true);
        assert!(sq.has_more_pages);
    }

    // --- jump_to() ---

    #[test]
    fn jump_to_sets_current_index() {
        let mut sq = queue_with(vec![track_item(1), track_item(2), track_item(3)]);
        sq.jump_to(2);
        assert_eq!(sq.current_track().unwrap().id, 3);
    }

    #[test]
    fn jump_to_out_of_bounds_does_nothing() {
        let mut sq = queue_with(vec![track_item(1)]);
        sq.jump_to(5);
        assert!(sq.current_track().is_none());
    }

    #[test]
    fn jump_to_container_resets_play_index() {
        let mut sq = queue_with(vec![container_item("c", Some(vec![track(1), track(2)]))]);
        // Manually set play_index to 1 first
        if let Some(SearchQueueItem::Container { play_index, .. }) = sq.items.get_mut(0) {
            *play_index = 1;
        }
        sq.jump_to(0);
        assert_eq!(sq.current_track().unwrap().id, 1);
    }

    // --- jump_to_with_track_index() ---

    #[test]
    fn jump_to_with_track_index_positions_within_container() {
        let mut sq = queue_with(vec![container_item(
            "c",
            Some(vec![track(10), track(20), track(30)]),
        )]);
        sq.jump_to_with_track_index(0, 2);
        assert_eq!(sq.current_track().unwrap().id, 30);
    }

    #[test]
    fn jump_to_with_track_index_ignores_out_of_bounds() {
        let mut sq = queue_with(vec![container_item("c", Some(vec![track(10)]))]);
        sq.jump_to_with_track_index(0, 99);
        // play_index unchanged at 0
        assert_eq!(sq.current_track().unwrap().id, 10);
    }

    #[test]
    fn jump_to_with_track_index_noop_on_unexpanded_container() {
        let mut sq = queue_with(vec![container_item("c", None)]);
        sq.jump_to_with_track_index(0, 1);
        // No panic, current_track remains None (no expanded tracks)
        assert!(sq.current_track().is_none());
    }

    // --- current_track() ---

    #[test]
    fn current_track_returns_none_without_current_index() {
        let sq = queue_with(vec![track_item(1)]);
        assert!(sq.current_track().is_none());
    }

    #[test]
    fn current_track_returns_track_item() {
        let mut sq = queue_with(vec![track_item(42)]);
        sq.jump_to(0);
        assert_eq!(sq.current_track().unwrap().id, 42);
    }

    #[test]
    fn current_track_returns_none_for_unexpanded_container() {
        let mut sq = queue_with(vec![container_item("c", None)]);
        sq.jump_to(0);
        assert!(sq.current_track().is_none());
    }

    #[test]
    fn current_track_returns_track_at_play_index() {
        let mut sq = queue_with(vec![container_item("c", Some(vec![track(1), track(2)]))]);
        sq.jump_to_with_track_index(0, 1);
        assert_eq!(sq.current_track().unwrap().id, 2);
    }

    // --- peek_next() ---

    #[test]
    fn peek_next_returns_none_without_current() {
        let sq = queue_with(vec![track_item(1), track_item(2)]);
        assert!(sq.peek_next().is_none());
    }

    #[test]
    fn peek_next_returns_none_at_last_track_item() {
        let mut sq = queue_with(vec![track_item(1)]);
        sq.jump_to(0);
        assert!(sq.peek_next().is_none());
    }

    #[test]
    fn peek_next_returns_next_track_item() {
        let mut sq = queue_with(vec![track_item(1), track_item(2)]);
        sq.jump_to(0);
        assert_eq!(sq.peek_next().unwrap().id, 2);
    }

    #[test]
    fn peek_next_returns_first_track_of_next_expanded_container() {
        let mut sq = queue_with(vec![
            track_item(1),
            container_item("c", Some(vec![track(10), track(20)])),
        ]);
        sq.jump_to(0);
        assert_eq!(sq.peek_next().unwrap().id, 10);
    }

    // This test documents the bug: unexpanded container ⇒ peek returns None
    // (preload impossible until expand happens)
    #[test]
    fn peek_next_returns_none_for_unexpanded_next_container() {
        let mut sq = queue_with(vec![track_item(1), container_item("c", None)]);
        sq.jump_to(0);
        assert!(sq.peek_next().is_none());
    }

    #[test]
    fn peek_next_inside_container_returns_next_internal_track() {
        let mut sq = queue_with(vec![container_item(
            "c",
            Some(vec![track(1), track(2), track(3)]),
        )]);
        sq.jump_to(0);
        // play_index=0, next within container is track(2)
        assert_eq!(sq.peek_next().unwrap().id, 2);
    }

    #[test]
    fn peek_next_at_last_container_track_returns_next_queue_item() {
        let mut sq = queue_with(vec![
            container_item("c", Some(vec![track(1), track(2)])),
            track_item(99),
        ]);
        sq.jump_to_with_track_index(0, 1); // at last track of container
        assert_eq!(sq.peek_next().unwrap().id, 99);
    }

    #[test]
    fn peek_next_does_not_advance_state() {
        let mut sq = queue_with(vec![track_item(1), track_item(2)]);
        sq.jump_to(0);
        let _ = sq.peek_next();
        let _ = sq.peek_next();
        assert_eq!(sq.current_track().unwrap().id, 1);
    }

    // --- next() ---

    #[test]
    fn next_with_no_current_starts_from_first_item() {
        let mut sq = queue_with(vec![track_item(5)]);
        let result = sq.advance();
        assert!(matches!(result, NextResult::Track(t) if t.id == 5));
    }

    #[test]
    fn next_on_empty_queue_returns_end() {
        let mut sq = queue_with(vec![]);
        assert!(matches!(sq.advance(), NextResult::End));
    }

    #[test]
    fn next_on_empty_queue_with_more_pages_returns_need_more() {
        let mut sq = SearchQueue::new("s".into(), "q".into(), vec![], true);
        assert!(matches!(sq.advance(), NextResult::NeedMorePages));
    }

    #[test]
    fn next_advances_through_track_items() {
        let mut sq = queue_with(vec![track_item(1), track_item(2), track_item(3)]);
        sq.jump_to(0);
        let r1 = sq.advance();
        let r2 = sq.advance();
        assert!(matches!(r1, NextResult::Track(t) if t.id == 2));
        assert!(matches!(r2, NextResult::Track(t) if t.id == 3));
    }

    #[test]
    fn next_returns_end_after_last_track() {
        let mut sq = queue_with(vec![track_item(1)]);
        sq.jump_to(0);
        assert!(matches!(sq.advance(), NextResult::End));
    }

    #[test]
    fn next_returns_need_more_pages_after_last_item() {
        let mut sq = SearchQueue::new("s".into(), "q".into(), vec![track_item(1)], true);
        sq.jump_to(0);
        assert!(matches!(sq.advance(), NextResult::NeedMorePages));
    }

    #[test]
    fn next_on_unexpanded_container_returns_need_expand() {
        let mut sq = queue_with(vec![track_item(1), container_item("c", None)]);
        sq.jump_to(0);
        assert!(matches!(sq.advance(), NextResult::NeedExpand(1)));
    }

    #[test]
    fn next_on_expanded_container_returns_first_track() {
        let mut sq = queue_with(vec![
            track_item(1),
            container_item("c", Some(vec![track(10), track(20)])),
        ]);
        sq.jump_to(0);
        let result = sq.advance();
        assert!(matches!(result, NextResult::Track(t) if t.id == 10));
    }

    #[test]
    fn next_advances_within_container() {
        let mut sq = queue_with(vec![container_item(
            "c",
            Some(vec![track(1), track(2), track(3)]),
        )]);
        sq.jump_to(0);
        let r1 = sq.advance();
        let r2 = sq.advance();
        assert!(matches!(r1, NextResult::Track(t) if t.id == 2));
        assert!(matches!(r2, NextResult::Track(t) if t.id == 3));
    }

    #[test]
    fn next_exits_container_and_moves_to_next_item() {
        let mut sq = queue_with(vec![
            container_item("c", Some(vec![track(1), track(2)])),
            track_item(99),
        ]);
        sq.jump_to(0);
        sq.advance(); // → track(2)
        let result = sq.advance(); // → track(99)
        assert!(matches!(result, NextResult::Track(t) if t.id == 99));
    }

    #[test]
    fn next_resets_play_index_when_entering_container() {
        let mut sq = queue_with(vec![
            track_item(1),
            container_item("c", Some(vec![track(10), track(20)])),
        ]);
        // Dirty play_index
        if let Some(SearchQueueItem::Container { play_index, .. }) = sq.items.get_mut(1) {
            *play_index = 1;
        }
        sq.jump_to(0);
        let result = sq.advance();
        assert!(matches!(result, NextResult::Track(t) if t.id == 10));
    }

    // --- previous() ---

    #[test]
    fn previous_without_current_returns_end() {
        let mut sq = queue_with(vec![track_item(1)]);
        assert!(matches!(sq.previous(), NextResult::End));
    }

    #[test]
    fn previous_at_first_item_returns_end() {
        let mut sq = queue_with(vec![track_item(1)]);
        sq.jump_to(0);
        assert!(matches!(sq.previous(), NextResult::End));
    }

    #[test]
    fn previous_moves_to_previous_track_item() {
        let mut sq = queue_with(vec![track_item(1), track_item(2)]);
        sq.jump_to(1);
        let result = sq.previous();
        assert!(matches!(result, NextResult::Track(t) if t.id == 1));
    }

    #[test]
    fn previous_goes_back_within_container() {
        let mut sq = queue_with(vec![container_item(
            "c",
            Some(vec![track(1), track(2), track(3)]),
        )]);
        sq.jump_to_with_track_index(0, 2);
        let r1 = sq.previous();
        let r2 = sq.previous();
        assert!(matches!(r1, NextResult::Track(t) if t.id == 2));
        assert!(matches!(r2, NextResult::Track(t) if t.id == 1));
    }

    #[test]
    fn previous_at_container_first_track_moves_to_preceding_item() {
        let mut sq = queue_with(vec![
            track_item(99),
            container_item("c", Some(vec![track(1), track(2)])),
        ]);
        sq.jump_to(1); // play_index=0 → track(1)
        let result = sq.previous();
        assert!(matches!(result, NextResult::Track(t) if t.id == 99));
    }

    #[test]
    fn previous_into_expanded_container_lands_on_last_track() {
        let mut sq = queue_with(vec![
            container_item("c", Some(vec![track(10), track(20), track(30)])),
            track_item(99),
        ]);
        sq.jump_to(1);
        let result = sq.previous();
        assert!(matches!(result, NextResult::Track(t) if t.id == 30));
    }

    #[test]
    fn previous_into_unexpanded_container_returns_need_expand() {
        let mut sq = queue_with(vec![container_item("c", None), track_item(99)]);
        sq.jump_to(1);
        let result = sq.previous();
        assert!(matches!(result, NextResult::NeedExpand(0)));
    }

    // --- set_container_tracks() ---

    #[test]
    fn set_container_tracks_populates_expanded_tracks() {
        let mut sq = queue_with(vec![container_item("c", None)]);
        sq.set_container_tracks(0, vec![track(1), track(2)]);
        sq.jump_to(0);
        assert_eq!(sq.current_track().unwrap().id, 1);
    }

    #[test]
    fn set_container_tracks_resets_play_index_to_zero() {
        let mut sq = queue_with(vec![container_item("c", Some(vec![track(1), track(2)]))]);
        if let Some(SearchQueueItem::Container { play_index, .. }) = sq.items.get_mut(0) {
            *play_index = 1;
        }
        sq.set_container_tracks(0, vec![track(10), track(20)]);
        sq.jump_to(0);
        assert_eq!(sq.current_track().unwrap().id, 10);
    }

    #[test]
    fn set_container_tracks_noop_on_track_item() {
        let mut sq = queue_with(vec![track_item(1)]);
        sq.set_container_tracks(0, vec![track(99)]); // should not panic
        sq.jump_to(0);
        assert_eq!(sq.current_track().unwrap().id, 1);
    }

    // --- append_results() ---

    #[test]
    fn append_results_adds_items_and_updates_offset() {
        let mut sq = queue_with(vec![track_item(1)]);
        sq.append_results(vec![track_item(2), track_item(3)], false);
        assert_eq!(sq.search_offset, 3);
        assert!(!sq.has_more_pages);
    }

    #[test]
    fn append_results_updates_has_more_pages() {
        let mut sq = queue_with(vec![]);
        sq.append_results(vec![track_item(1)], true);
        assert!(sq.has_more_pages);
    }

    #[test]
    fn appended_items_are_navigable() {
        let mut sq = queue_with(vec![track_item(1)]);
        sq.append_results(vec![track_item(2)], false);
        sq.jump_to(0);
        let result = sq.advance();
        assert!(matches!(result, NextResult::Track(t) if t.id == 2));
    }

    // --- snapshot() ---

    #[test]
    fn snapshot_returns_correct_entry_types() {
        let sq = queue_with(vec![
            track_item(1),
            container_item("c", Some(vec![track(10)])),
        ]);
        let (entries, _) = sq.snapshot();
        assert!(matches!(entries[0], QueueEntry::Track(_)));
        assert!(matches!(entries[1], QueueEntry::Container { .. }));
    }

    #[test]
    fn snapshot_position_is_none_without_current() {
        let sq = queue_with(vec![track_item(1)]);
        let (_, pos) = sq.snapshot();
        assert!(pos.is_none());
    }

    #[test]
    fn snapshot_position_reflects_current_track_item() {
        let mut sq = queue_with(vec![track_item(1), track_item(2)]);
        sq.jump_to(1);
        let (_, pos) = sq.snapshot();
        let pos = pos.unwrap();
        assert_eq!(pos.item_index, 1);
        assert!(pos.track_within_container.is_none());
    }

    #[test]
    fn snapshot_position_reflects_play_index_within_container() {
        let mut sq = queue_with(vec![container_item(
            "c",
            Some(vec![track(1), track(2), track(3)]),
        )]);
        sq.jump_to_with_track_index(0, 2);
        let (_, pos) = sq.snapshot();
        let pos = pos.unwrap();
        assert_eq!(pos.item_index, 0);
        assert_eq!(pos.track_within_container, Some(2));
    }

    #[test]
    fn snapshot_container_exposes_expanded_tracks() {
        let sq = queue_with(vec![container_item("c", Some(vec![track(10), track(20)]))]);
        let (entries, _) = sq.snapshot();
        if let QueueEntry::Container {
            tracks, play_index, ..
        } = &entries[0]
        {
            assert!(tracks.is_some());
            assert_eq!(tracks.as_ref().unwrap().len(), 2);
            assert_eq!(*play_index, 0);
        } else {
            panic!("expected Container entry");
        }
    }

    // --- peek_upcoming_tracks() ---

    #[test]
    fn peek_upcoming_tracks_empty_without_current() {
        let sq = queue_with(vec![track_item(1), track_item(2)]);
        assert!(sq.peek_upcoming_tracks(9).is_empty());
    }

    #[test]
    fn peek_upcoming_tracks_returns_subsequent_track_items() {
        let mut sq = queue_with(vec![track_item(1), track_item(2), track_item(3)]);
        sq.jump_to(0);
        let upcoming = sq.peek_upcoming_tracks(9);
        assert_eq!(
            upcoming.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[test]
    fn peek_upcoming_tracks_returns_remaining_tracks_in_current_container() {
        let mut sq = queue_with(vec![container_item(
            "c",
            Some(vec![track(10), track(20), track(30)]),
        )]);
        sq.jump_to(0); // play_index=0, current=track(10)
        let upcoming = sq.peek_upcoming_tracks(9);
        assert_eq!(
            upcoming.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![20, 30]
        );
    }

    #[test]
    fn peek_upcoming_tracks_combines_container_remainder_and_next_items() {
        let mut sq = queue_with(vec![
            container_item("c", Some(vec![track(1), track(2), track(3)])),
            track_item(99),
        ]);
        sq.jump_to(0); // play_index=0
        let upcoming = sq.peek_upcoming_tracks(9);
        assert_eq!(
            upcoming.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![2, 3, 99]
        );
    }

    #[test]
    fn peek_upcoming_tracks_expands_subsequent_containers() {
        let mut sq = queue_with(vec![
            track_item(1),
            container_item("album", Some(vec![track(10), track(20)])),
            track_item(99),
        ]);
        sq.jump_to(0);
        let upcoming = sq.peek_upcoming_tracks(9);
        assert_eq!(
            upcoming.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![10, 20, 99]
        );
    }

    #[test]
    fn peek_upcoming_tracks_skips_unexpanded_containers() {
        let mut sq = queue_with(vec![
            track_item(1),
            container_item("c", None),
            track_item(99),
        ]);
        sq.jump_to(0);
        let upcoming = sq.peek_upcoming_tracks(9);
        // Unexpanded container is skipped, track(99) is still reachable
        assert_eq!(upcoming.iter().map(|t| t.id).collect::<Vec<_>>(), vec![99]);
    }

    #[test]
    fn peek_upcoming_tracks_respects_limit() {
        let mut sq = queue_with(vec![
            track_item(1),
            track_item(2),
            track_item(3),
            track_item(4),
            track_item(5),
        ]);
        sq.jump_to(0);
        let upcoming = sq.peek_upcoming_tracks(3);
        assert_eq!(upcoming.len(), 3);
        assert_eq!(upcoming[0].id, 2);
        assert_eq!(upcoming[2].id, 4);
    }

    #[test]
    fn peek_upcoming_tracks_does_not_advance_state() {
        let mut sq = queue_with(vec![track_item(1), track_item(2), track_item(3)]);
        sq.jump_to(0);
        let _ = sq.peek_upcoming_tracks(9);
        let _ = sq.peek_upcoming_tracks(9);
        assert_eq!(sq.current_track().unwrap().id, 1);
    }

    #[test]
    fn peek_upcoming_tracks_at_mid_container_excludes_past_tracks() {
        let mut sq = queue_with(vec![container_item(
            "c",
            Some(vec![track(10), track(20), track(30)]),
        )]);
        sq.jump_to_with_track_index(0, 1); // current=track(20)
        let upcoming = sq.peek_upcoming_tracks(9);
        assert_eq!(upcoming.iter().map(|t| t.id).collect::<Vec<_>>(), vec![30]);
    }

    #[test]
    fn peek_upcoming_tracks_at_last_container_track_uses_next_items() {
        let mut sq = queue_with(vec![
            container_item("c", Some(vec![track(10), track(20)])),
            track_item(99),
        ]);
        sq.jump_to_with_track_index(0, 1); // current=track(20), last in container
        let upcoming = sq.peek_upcoming_tracks(9);
        assert_eq!(upcoming.iter().map(|t| t.id).collect::<Vec<_>>(), vec![99]);
    }
}
