use crate::audio::types::Track;
use crate::playlist::cursor::{ContainerContext, CursorRow, PlayerCursor};
use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use std::collections::VecDeque;

mod search_queue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

/// One row of the upcoming part of a `CursorQueue`.
#[derive(Debug, Clone)]
enum UpcomingRow {
    Track(Track),
    /// An album or playlist from the root of a search that playback has not reached yet.
    Container {
        context: ContainerContext,
        info: ContainerMeta,
        tracks: Option<Vec<Track>>,
    },
}

/// Where a snapshot row index points.
enum RowLocation {
    History(usize),
    Current,
    Upcoming(usize),
}

pub struct CursorQueue {
    pub history: Vec<Track>,
    pub current_track: Option<Track>,
    pub cursor: Option<PlayerCursor>,
    upcoming: VecDeque<UpcomingRow>,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub context: Option<ContainerContext>,
}

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
            upcoming: VecDeque::new(),
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            context: None,
        }
    }

    /// Search contexts keep album and playlist results as container rows.
    fn keeps_root_containers(&self) -> bool {
        matches!(self.context, Some(ContainerContext::SearchQuery { .. }))
    }

    /// The one place that translates a snapshot row index (history, current, upcoming).
    fn locate(&self, row: usize) -> Option<RowLocation> {
        let history_len = self.history.len();
        let current_len = usize::from(self.current_track.is_some());
        if row < history_len {
            Some(RowLocation::History(row))
        } else if row < history_len + current_len {
            Some(RowLocation::Current)
        } else {
            let i = row - history_len - current_len;
            (i < self.upcoming.len()).then_some(RowLocation::Upcoming(i))
        }
    }

    /// Pulls one more row from the cursor into the upcoming part.
    async fn fetch_one(&mut self) -> Result<bool> {
        let keep = self.keeps_root_containers();
        let Some(cursor) = &mut self.cursor else {
            return Ok(false);
        };
        match cursor.next_row(keep).await? {
            Some(CursorRow::Track(track)) => self.upcoming.push_back(UpcomingRow::Track(track)),
            Some(CursorRow::Container { context, meta }) => {
                self.upcoming.push_back(UpcomingRow::Container {
                    context,
                    info: meta,
                    tracks: None,
                })
            }
            None => return Ok(false),
        }
        Ok(true)
    }

    /// The tracks of container row `i`, fetched through the resolver unless previewed.
    async fn container_tracks(&self, i: usize) -> Result<Vec<Track>> {
        let Some(UpcomingRow::Container {
            context, tracks, ..
        }) = self.upcoming.get(i)
        else {
            anyhow::bail!("Row is not a container");
        };
        if let Some(tracks) = tracks {
            return Ok(tracks.clone());
        }
        let cursor = self.cursor.as_ref().context("Queue has no cursor")?;
        cursor.collect_tracks(context.clone()).await
    }

    /// Replaces container row `i` in place by its tracks as track rows. Returns how many
    /// tracks it held (the row is removed when that is zero).
    async fn expand_in_place(&mut self, i: usize) -> Result<usize> {
        let tracks = self.container_tracks(i).await?;
        let count = tracks.len();
        self.upcoming.remove(i);
        for (offset, track) in tracks.into_iter().enumerate() {
            self.upcoming.insert(i + offset, UpcomingRow::Track(track));
        }
        Ok(count)
    }

    /// Takes upcoming row `i` as the current track. A container is replaced by its
    /// tracks and its first one is taken; an empty container yields `None`.
    async fn enter_row(&mut self, i: usize) -> Result<Option<Track>> {
        if matches!(self.upcoming.get(i), Some(UpcomingRow::Container { .. })) {
            self.expand_in_place(i).await?;
        }
        match self.upcoming.remove(i) {
            Some(UpcomingRow::Track(track)) => Ok(Some(track)),
            _ => Ok(None),
        }
    }

    pub async fn next(&mut self) -> Result<Option<Track>> {
        // Handle Repeat One: just return the current track if it exists
        if self.repeat_mode == RepeatMode::One {
            if let Some(track) = &self.current_track {
                return Ok(Some(track.clone()));
            }
        }

        // Save current track to history before moving to next
        if let Some(track) = self.current_track.take() {
            self.history.push(track);
        }

        loop {
            if self.upcoming.is_empty() {
                if self.shuffle_enabled {
                    // Prefetch a window so there is something to shuffle
                    self.prefetch(50).await?;
                } else {
                    self.fetch_one().await?;
                }
                if self.upcoming.is_empty() {
                    return Ok(None);
                }
            }
            let idx = if self.shuffle_enabled {
                thread_rng().gen_range(0..self.upcoming.len())
            } else {
                0
            };
            // An empty container yields nothing: go on with the next row.
            if let Some(track) = self.enter_row(idx).await? {
                self.current_track = Some(track.clone());
                return Ok(Some(track));
            }
        }
    }

    pub fn previous(&mut self) -> Option<Track> {
        // Handle Repeat One: just return the current track
        if self.repeat_mode == RepeatMode::One {
            if let Some(track) = &self.current_track {
                return Some(track.clone());
            }
        }

        let prev = self.history.pop()?;

        // Push current track back to the front of the upcoming rows to allow "going forward" again
        if let Some(current) = self.current_track.take() {
            self.upcoming.push_front(UpcomingRow::Track(current));
        }

        self.current_track = Some(prev.clone());
        Some(prev)
    }

    pub fn peek_next(&self) -> Option<Track> {
        match self.upcoming.front() {
            Some(UpcomingRow::Track(track)) => Some(track.clone()),
            _ => None,
        }
    }

    pub fn peek_upcoming_tracks(&self, limit: usize) -> Vec<Track> {
        self.upcoming
            .iter()
            .filter_map(|row| match row {
                UpcomingRow::Track(track) => Some(track.clone()),
                UpcomingRow::Container { .. } => None,
            })
            .take(limit)
            .collect()
    }

    pub fn current(&self) -> Option<&Track> {
        self.current_track.as_ref()
    }

    pub fn add_track(&mut self, track: Track) {
        self.upcoming.push_back(UpcomingRow::Track(track));
    }

    /// Removes snapshot row `index`; a container row removes the whole container.
    /// Returns the removed track when the row was a track.
    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        match self.locate(index)? {
            RowLocation::History(i) => Some(self.history.remove(i)),
            RowLocation::Current => self.current_track.take(),
            RowLocation::Upcoming(i) => match self.upcoming.remove(i)? {
                UpcomingRow::Track(track) => Some(track),
                UpcomingRow::Container { .. } => None,
            },
        }
    }

    /// Jump to a track row of the snapshot, rearranging history/current/upcoming.
    /// Container rows are not valid targets (see `play_queue_item`) and, when jumped
    /// over, are dropped: history holds tracks only.
    pub fn jump_to(&mut self, index: usize) {
        let is_track = match self.locate(index) {
            Some(RowLocation::History(_)) => true,
            Some(RowLocation::Upcoming(i)) => matches!(self.upcoming[i], UpcomingRow::Track(_)),
            Some(RowLocation::Current) | None => false,
        };
        if !is_track {
            return;
        }
        let mut tracks: Vec<Track> = self.history.drain(..).collect();
        tracks.extend(self.current_track.take());
        let upcoming: Vec<UpcomingRow> = self.upcoming.drain(..).collect();
        let mut rows: Vec<UpcomingRow> = tracks.into_iter().map(UpcomingRow::Track).collect();
        rows.extend(upcoming);
        let after = rows.split_off(index + 1);
        let Some(UpcomingRow::Track(target)) = rows.pop() else {
            return;
        };
        self.history = rows
            .into_iter()
            .filter_map(|row| match row {
                UpcomingRow::Track(track) => Some(track),
                UpcomingRow::Container { .. } => None,
            })
            .collect();
        self.current_track = Some(target);
        self.upcoming = after.into();
    }

    /// Plays snapshot row `index`. A container row is replaced in place by its tracks and
    /// `track_index` (default: the first) picks which one starts. An empty container is
    /// removed and nothing plays.
    pub async fn play_queue_item(
        &mut self,
        index: usize,
        track_index: Option<usize>,
    ) -> Result<Option<Track>> {
        let mut target = index;
        if let Some(RowLocation::Upcoming(i)) = self.locate(index) {
            if matches!(self.upcoming.get(i), Some(UpcomingRow::Container { .. })) {
                let count = self.expand_in_place(i).await?;
                if count == 0 {
                    return Ok(None);
                }
                target = index + track_index.filter(|t| *t < count).unwrap_or(0);
            }
        }
        self.jump_to(target);
        Ok(self.current_track.clone())
    }

    /// Previews container row `index`: fetches its tracks and attaches them to the row
    /// without changing row indices or playback.
    pub async fn expand_container(&mut self, index: usize) -> Result<()> {
        let Some(RowLocation::Upcoming(i)) = self.locate(index) else {
            anyhow::bail!("Row {index} is not a container");
        };
        let fetched = self.container_tracks(i).await?;
        if let Some(UpcomingRow::Container { tracks, .. }) = self.upcoming.get_mut(i) {
            *tracks = Some(fetched);
        }
        Ok(())
    }

    pub fn set_cursor(&mut self, cursor: PlayerCursor) {
        tracing::info!("Setting cursor in queue");
        self.cursor = Some(cursor);
    }

    /// Fills the upcoming part from the cursor until it holds `limit` rows (a container
    /// row counts as one). Returns how many rows were added.
    pub async fn prefetch(&mut self, limit: usize) -> Result<usize> {
        let mut count = 0;
        while self.upcoming.len() < limit && self.fetch_one().await? {
            count += 1;
        }
        Ok(count)
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.current_track = None;
        self.cursor = None;
        self.upcoming.clear();
        // Must be reset too: a stale context would make Repeat::All resurrect the old
        // source, and would misreport the active radio session to the UI.
        self.context = None;
    }

    /// Creates the row list the UI shows: history tracks, the current track, then the
    /// upcoming part. Returns `(rows, current_row_index)`.
    ///
    /// Row model: a row is a track or a container. Container rows exist only in the
    /// upcoming part, only for albums and playlists the root of a search context yielded,
    /// and only until playback reaches them (then they become track rows). Every row index
    /// the UI sends back (`remove_track`, `jump_to`, `play_queue_item`, `expand_container`)
    /// refers to this list.
    pub fn snapshot(&self) -> (Vec<QueueEntry>, Option<usize>) {
        let mut rows = Vec::with_capacity(self.history.len() + 1 + self.upcoming.len());
        rows.extend(self.history.iter().cloned().map(QueueEntry::Track));
        let current_index = self.current_track.as_ref().map(|current| {
            rows.push(QueueEntry::Track(current.clone()));
            rows.len() - 1
        });
        rows.extend(self.upcoming.iter().map(|row| match row {
            UpcomingRow::Track(track) => QueueEntry::Track(track.clone()),
            UpcomingRow::Container { info, tracks, .. } => QueueEntry::Container {
                info: info.clone(),
                tracks: tracks.clone(),
                play_index: 0,
            },
        }));
        (rows, current_index)
    }
}

// === Search Queue Types ===

pub use crate::playlist::cursor::{ContainerKind, ContainerMeta};

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
