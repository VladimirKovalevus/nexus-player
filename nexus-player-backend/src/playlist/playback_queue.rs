use crate::audio::types::Track;
use crate::playlist::cursor::PlayerCursor;
use crate::playlist::playlist::Playlist;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl Default for RepeatMode {
    fn default() -> Self {
        Self::Off
    }
}

pub struct CursorQueue {
    pub history: Vec<Track>,
    pub current_track: Option<Track>,
    pub cursor: Option<PlayerCursor>,
    pub manual_queue: std::collections::VecDeque<Track>,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
}

use anyhow::Result;

impl CursorQueue {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            current_track: None,
            cursor: None,
            manual_queue: std::collections::VecDeque::new(),
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
        }
    }

    pub async fn next(&mut self) -> Result<Option<Track>> {
        // 1. Save current track to history before moving to next
        if let Some(track) = self.current_track.take() {
            self.history.push(track);
        }

        // 2. Try manual queue first
        if let Some(track) = self.manual_queue.pop_front() {
            self.current_track = Some(track.clone());
            return Ok(Some(track));
        }

        // 3. Fetch from cursor if available
        if let Some(cursor) = &mut self.cursor {
            let track = cursor.next_track().await?;
            self.current_track = track.clone();
            return Ok(track);
        }

        Ok(None)
    }

    pub fn previous(&mut self) -> Option<Track> {
        let prev = self.history.pop()?;

        // Push current track back to the front of manual queue to allow "going forward" again
        if let Some(current) = self.current_track.take() {
            self.manual_queue.push_front(current);
        }

        self.current_track = Some(prev.clone());
        Some(prev)
    }

    pub fn peek_next(&self) -> Option<Track> {
        // 1. Try manual queue first
        if let Some(track) = self.manual_queue.front() {
            return Some(track.clone());
        }

        // 2. We don't peek into cursor because it's async and stateful
        None
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
}

impl SearchQueue {
    pub fn new(system: String, query: String, items: Vec<SearchQueueItem>, has_more: bool) -> Self {
        let offset = items.len();
        Self {
            system,
            query,
            items,
            current_index: None,
            search_offset: offset,
            has_more_pages: has_more,
        }
    }

    pub fn jump_to(&mut self, index: usize) {
        if index < self.items.len() {
            self.current_index = Some(index);
            // Reset play_index for containers
            if let Some(SearchQueueItem::Container { play_index, .. }) = self.items.get_mut(index) {
                *play_index = 0;
            }
        }
    }

    pub fn jump_to_with_track_index(&mut self, index: usize, track_idx: usize) {
        if index < self.items.len() {
            self.current_index = Some(index);
            if let Some(SearchQueueItem::Container {
                play_index,
                expanded_tracks,
                ..
            }) = self.items.get_mut(index)
            {
                if let Some(tracks) = expanded_tracks {
                    if track_idx < tracks.len() {
                        *play_index = track_idx;
                    }
                }
            }
        }
    }

    pub fn current_track(&self) -> Option<Track> {
        let idx = self.current_index?;
        match self.items.get(idx)? {
            SearchQueueItem::Track(track) => Some(track.clone()),
            SearchQueueItem::Container {
                expanded_tracks,
                play_index,
                ..
            } => expanded_tracks.as_ref()?.get(*play_index).cloned(),
        }
    }

    pub fn peek_next(&self) -> Option<Track> {
        let current = self.current_index?;

        // 1. If current is a container, check if there's a next track within it
        if let Some(SearchQueueItem::Container {
            expanded_tracks: Some(tracks),
            play_index,
            ..
        }) = self.items.get(current)
        {
            if *play_index + 1 < tracks.len() {
                return Some(tracks[*play_index + 1].clone());
            }
        }

        // 2. Otherwise, check the next item in the queue
        let next_idx = current + 1;
        if next_idx < self.items.len() {
            match &self.items[next_idx] {
                SearchQueueItem::Track(track) => return Some(track.clone()),
                SearchQueueItem::Container {
                    expanded_tracks: Some(tracks),
                    ..
                } => {
                    if !tracks.is_empty() {
                        return Some(tracks[0].clone());
                    }
                }
                _ => {}
            }
        }

        None
    }

    pub fn next(&mut self) -> NextResult {
        let current = match self.current_index {
            Some(idx) => idx,
            None => {
                // Start from the beginning
                if self.items.is_empty() {
                    if self.has_more_pages {
                        return NextResult::NeedMorePages;
                    }
                    return NextResult::End;
                }
                self.current_index = Some(0);
                return self.return_current_or_expand(0);
            }
        };

        // If current is a container with tracks, try advancing within it
        if let Some(SearchQueueItem::Container {
            expanded_tracks: Some(tracks),
            play_index,
            ..
        }) = self.items.get(current)
        {
            if *play_index + 1 < tracks.len() {
                // Advance within container
                if let Some(SearchQueueItem::Container {
                    play_index,
                    expanded_tracks: Some(tracks),
                    ..
                }) = self.items.get_mut(current)
                {
                    *play_index += 1;
                    return NextResult::Track(tracks[*play_index].clone());
                }
            }
        }

        // Move to next item
        let next_idx = current + 1;
        if next_idx >= self.items.len() {
            if self.has_more_pages {
                return NextResult::NeedMorePages;
            }
            return NextResult::End;
        }

        self.current_index = Some(next_idx);

        // Ensure that if the next item is a container, it starts from the beginning
        if let Some(SearchQueueItem::Container { play_index, .. }) = self.items.get_mut(next_idx) {
            *play_index = 0;
        }

        self.return_current_or_expand(next_idx)
        }

    pub fn previous(&mut self) -> NextResult {
        let current = match self.current_index {
            Some(idx) => idx,
            None => return NextResult::End,
        };

        // If current is a container with tracks, try going back within it
        if let Some(SearchQueueItem::Container {
            expanded_tracks: Some(_),
            play_index,
            ..
        }) = self.items.get(current)
        {
            if *play_index > 0 {
                if let Some(SearchQueueItem::Container {
                    play_index,
                    expanded_tracks: Some(tracks),
                    ..
                }) = self.items.get_mut(current)
                {
                    *play_index -= 1;
                    return NextResult::Track(tracks[*play_index].clone());
                }
            }
        }

        // Move to previous item
        if current == 0 {
            return NextResult::End;
        }

        let prev_idx = current - 1;
        self.current_index = Some(prev_idx);

        // If the previous item is a container, we usually want its LAST track when going backwards
        if let Some(SearchQueueItem::Container {
            play_index,
            expanded_tracks,
            ..
        }) = self.items.get_mut(prev_idx)
        {
            if let Some(tracks) = expanded_tracks {
                *play_index = tracks.len().saturating_sub(1);
            } else {
                *play_index = 0; // Will be expanded and we should probably handle it then
            }
        }

        self.return_current_or_expand(prev_idx)
    }

    fn return_current_or_expand(&self, idx: usize) -> NextResult {
        match &self.items[idx] {
            SearchQueueItem::Track(track) => NextResult::Track(track.clone()),
            SearchQueueItem::Container {
                expanded_tracks: None,
                ..
            } => NextResult::NeedExpand(idx),
            SearchQueueItem::Container {
                expanded_tracks: Some(tracks),
                play_index,
                ..
            } => {
                if tracks.is_empty() {
                    // Empty container, skip conceptually — but return End for now
                    NextResult::End
                } else {
                    NextResult::Track(tracks[*play_index].clone())
                }
            }
        }
    }

    pub fn set_container_tracks(&mut self, index: usize, tracks: Vec<Track>) {
        if let Some(SearchQueueItem::Container {
            expanded_tracks,
            play_index,
            ..
        }) = self.items.get_mut(index)
        {
            *expanded_tracks = Some(tracks);
            *play_index = 0;
        }
    }

    pub fn append_results(&mut self, new_items: Vec<SearchQueueItem>, has_more: bool) {
        self.search_offset += new_items.len();
        self.items.extend(new_items);
        self.has_more_pages = has_more;
    }

    pub fn snapshot(&self) -> (Vec<QueueEntry>, Option<CurrentPosition>) {
        let entries: Vec<QueueEntry> = self
            .items
            .iter()
            .map(|item| match item {
                SearchQueueItem::Track(track) => QueueEntry::Track(track.clone()),
                SearchQueueItem::Container {
                    info,
                    expanded_tracks,
                    play_index,
                } => QueueEntry::Container {
                    info: info.clone(),
                    tracks: expanded_tracks.clone(),
                    play_index: *play_index,
                },
            })
            .collect();

        let position = self.current_index.map(|idx| {
            let track_within = match &self.items[idx] {
                SearchQueueItem::Container {
                    expanded_tracks: Some(_),
                    play_index,
                    ..
                } => Some(*play_index),
                _ => None,
            };
            CurrentPosition {
                item_index: idx,
                track_within_container: track_within,
            }
        });

        (entries, position)
    }
}

/// Unified queue type
pub enum ActiveQueue {
    Cursor(CursorQueue),
    Search(SearchQueue),
}

impl ActiveQueue {
    pub fn new_cursor() -> Self {
        ActiveQueue::Cursor(CursorQueue::new())
    }
    pub fn peek_next(&self) -> Option<Track> {
        match self {
            ActiveQueue::Cursor(o) => o.peek_next(),
            ActiveQueue::Search(o) => o.peek_next(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlaybackQueueOld {
    tracks: Vec<Track>,
    current_index: Option<usize>,
    repeat_mode: RepeatMode,
    shuffle_enabled: bool,
    shuffle_order: Vec<usize>,
}

pub type PlaybackQueue = PlaybackQueueOld;

impl PlaybackQueueOld {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current_index: None,
            repeat_mode: RepeatMode::default(),
            shuffle_enabled: false,
            shuffle_order: Vec::new(),
        }
    }

    pub fn add_track(&mut self, track: Track) -> usize {
        self.tracks.push(track);

        if self.shuffle_enabled {
            self.regenerate_shuffle();
        }

        if self.current_index.is_none() && !self.tracks.is_empty() {
            self.current_index = Some(0);
        }

        self.tracks.len() - 1
    }

    pub fn add_tracks(&mut self, tracks: Vec<Track>) {
        for track in tracks {
            self.add_track(track);
        }
    }

    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        if index >= self.tracks.len() {
            return None;
        }

        let removed = self.tracks.remove(index);

        if self.shuffle_enabled {
            self.regenerate_shuffle();
        }

        if let Some(current) = self.current_index {
            if current == index {
                self.current_index = if self.tracks.is_empty() {
                    None
                } else if current >= self.tracks.len() {
                    Some(self.tracks.len().saturating_sub(1))
                } else {
                    Some(current)
                };
            } else if current > index {
                self.current_index = Some(current - 1);
            }
        }

        Some(removed)
    }

    pub fn remove_track_by_id(&mut self, track_id: usize) -> Option<Track> {
        if let Some(index) = self.tracks.iter().position(|t| t.id == track_id) {
            self.remove_track(index)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.shuffle_order.clear();
        self.current_index = None;
    }

    pub fn current(&self) -> Option<&Track> {
        let index = self.current_index?;

        if self.shuffle_enabled {
            let track_index = *self.shuffle_order.get(index)?;
            self.tracks.get(track_index)
        } else {
            self.tracks.get(index)
        }
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn next(&mut self) -> Option<&Track> {
        if self.tracks.is_empty() {
            return None;
        }

        let next_index = self.calculate_next_index()?;
        self.current_index = Some(next_index);
        self.current()
    }

    pub fn peek_next(&self) -> Option<&Track> {
        if self.tracks.is_empty() {
            return None;
        }

        let next_index = self.calculate_next_index()?;

        if self.shuffle_enabled {
            let track_index = *self.shuffle_order.get(next_index)?;
            self.tracks.get(track_index)
        } else {
            self.tracks.get(next_index)
        }
    }

    pub fn previous(&mut self) -> Option<&Track> {
        if self.tracks.is_empty() {
            return None;
        }

        let prev_index = self.calculate_previous_index()?;
        self.current_index = Some(prev_index);
        self.current()
    }

    pub fn jump_to(&mut self, index: usize) -> Option<&Track> {
        let max_index = if self.shuffle_enabled {
            self.shuffle_order.len()
        } else {
            self.tracks.len()
        };

        if index < max_index {
            self.current_index = Some(index);
            self.current()
        } else {
            None
        }
    }

    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat_mode = mode;
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle_enabled = !self.shuffle_enabled;

        if self.shuffle_enabled {
            self.regenerate_shuffle();
        } else {
            self.shuffle_order.clear();

            if let Some(current_track) = self.current() {
                let current_id = current_track.id;
                self.current_index = self.tracks.iter().position(|t| t.id == current_id);
            }
        }
    }

    pub fn shuffle_enabled(&self) -> bool {
        self.shuffle_enabled
    }

    pub fn load_from_playlist(&mut self, playlist: Playlist, start_index: Option<usize>) {
        self.clear();

        self.tracks = playlist.tracks().to_vec();

        if self.shuffle_enabled {
            self.regenerate_shuffle();
        }

        self.current_index = start_index.or_else(|| {
            if !self.tracks.is_empty() {
                Some(0)
            } else {
                None
            }
        });
    }

    pub fn get_track(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    pub fn get_track_mut(&mut self, index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(index)
    }

    pub fn find_track_by_id(&self, track_id: usize) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == track_id)
    }

    pub fn find_index_by_id(&self, track_id: usize) -> Option<usize> {
        self.tracks.iter().position(|t| t.id == track_id)
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn len(&self) -> usize {
        if self.shuffle_enabled {
            self.shuffle_order.len()
        } else {
            self.tracks.len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    fn calculate_next_index(&self) -> Option<usize> {
        let current = self.current_index?;
        let len = self.len();

        match self.repeat_mode {
            RepeatMode::One => Some(current),
            RepeatMode::All => {
                if current + 1 >= len {
                    Some(0)
                } else {
                    Some(current + 1)
                }
            }
            RepeatMode::Off => {
                if current + 1 >= len {
                    None
                } else {
                    Some(current + 1)
                }
            }
        }
    }

    fn calculate_previous_index(&self) -> Option<usize> {
        let current = self.current_index?;
        let len = self.len();

        match self.repeat_mode {
            RepeatMode::One => Some(current),
            RepeatMode::All => {
                if current == 0 {
                    Some(len.saturating_sub(1))
                } else {
                    Some(current - 1)
                }
            }
            RepeatMode::Off => {
                if current == 0 {
                    Some(0)
                } else {
                    Some(current - 1)
                }
            }
        }
    }

    fn regenerate_shuffle(&mut self) {
        if self.tracks.is_empty() {
            self.shuffle_order.clear();
            return;
        }

        let current_track_id = self.current().map(|t| t.id);

        let mut indices: Vec<usize> = (0..self.tracks.len()).collect();
        indices.shuffle(&mut thread_rng());

        self.shuffle_order = indices;

        if let Some(track_id) = current_track_id {
            self.current_index = self
                .shuffle_order
                .iter()
                .position(|&idx| self.tracks[idx].id == track_id);
        }
    }
}

impl Default for PlaybackQueueOld {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Playlist> for PlaybackQueueOld {
    fn from(playlist: Playlist) -> Self {
        let mut queue = Self::new();
        queue.tracks = playlist.tracks().to_vec();

        if !queue.tracks.is_empty() {
            queue.current_index = Some(0);
        }

        queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::types::TrackSource;

    fn create_test_track(id: usize) -> Track {
        Track {
            id,
            format: None,
            source: TrackSource::local(format!("/path/to/track{}.mp3", id)),
        }
    }

    #[test]
    fn test_new_queue_is_empty() {
        let queue = PlaybackQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.current_index(), None);
    }

    #[test]
    fn test_add_track() {
        let mut queue = PlaybackQueue::new();
        let track = create_test_track(1);

        queue.add_track(track.clone());

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.current_index(), Some(0));
        assert_eq!(queue.current().unwrap().id, 1);
    }

    #[test]
    fn test_next_with_repeat_off() {
        let mut queue = PlaybackQueue::new();
        queue.add_track(create_test_track(1));
        queue.add_track(create_test_track(2));
        queue.add_track(create_test_track(3));

        queue.set_repeat_mode(RepeatMode::Off);

        assert_eq!(queue.current().unwrap().id, 1);
        assert_eq!(queue.next().unwrap().id, 2);
        assert_eq!(queue.next().unwrap().id, 3);
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn test_next_with_repeat_all() {
        let mut queue = PlaybackQueue::new();
        queue.add_track(create_test_track(1));
        queue.add_track(create_test_track(2));

        queue.set_repeat_mode(RepeatMode::All);

        assert_eq!(queue.current().unwrap().id, 1);
        assert_eq!(queue.next().unwrap().id, 2);
        assert_eq!(queue.next().unwrap().id, 1);
    }

    #[test]
    fn test_next_with_repeat_one() {
        let mut queue = PlaybackQueue::new();
        queue.add_track(create_test_track(1));
        queue.add_track(create_test_track(2));

        queue.set_repeat_mode(RepeatMode::One);

        assert_eq!(queue.current().unwrap().id, 1);
        assert_eq!(queue.next().unwrap().id, 1);
        assert_eq!(queue.next().unwrap().id, 1);
    }

    #[test]
    fn test_previous() {
        let mut queue = PlaybackQueue::new();
        queue.add_track(create_test_track(1));
        queue.add_track(create_test_track(2));
        queue.add_track(create_test_track(3));

        queue.jump_to(2);
        assert_eq!(queue.current().unwrap().id, 3);
        assert_eq!(queue.previous().unwrap().id, 2);
        assert_eq!(queue.previous().unwrap().id, 1);
    }

    #[test]
    fn test_shuffle_toggle() {
        let mut queue = PlaybackQueue::new();
        for i in 1..=5 {
            queue.add_track(create_test_track(i));
        }

        let original_order: Vec<usize> = queue.tracks().iter().map(|t| t.id).collect();

        queue.toggle_shuffle();

        assert_eq!(queue.shuffle_enabled(), true);
        assert_eq!(queue.shuffle_order.len(), 5);
        assert_eq!(
            queue.tracks().iter().map(|t| t.id).collect::<Vec<_>>(),
            original_order
        );

        queue.toggle_shuffle();

        assert_eq!(queue.shuffle_enabled(), false);
        assert!(queue.shuffle_order.is_empty());
    }

    #[test]
    fn test_shuffle_playback_order() {
        let mut queue = PlaybackQueue::new();
        queue.add_track(create_test_track(1));
        queue.add_track(create_test_track(2));
        queue.add_track(create_test_track(3));

        queue.set_repeat_mode(RepeatMode::Off);
        queue.toggle_shuffle();

        let first = queue.current().unwrap().id;
        let second = queue.next().unwrap().id;
        let third = queue.next().unwrap().id;

        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);

        let mut ids = vec![first, second, third];
        ids.sort();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn test_remove_track() {
        let mut queue = PlaybackQueue::new();
        queue.add_track(create_test_track(1));
        queue.add_track(create_test_track(2));
        queue.add_track(create_test_track(3));

        queue.remove_track(1);

        assert_eq!(queue.len(), 2);
        assert_eq!(queue.get_track(0).unwrap().id, 1);
        assert_eq!(queue.get_track(1).unwrap().id, 3);
    }

    #[test]
    fn test_load_from_playlist() {
        let mut playlist = Playlist::new("test".to_string());
        playlist.add_track(create_test_track(1));
        playlist.add_track(create_test_track(2));

        let mut queue = PlaybackQueue::new();
        queue.load_from_playlist(playlist, None);

        assert_eq!(queue.len(), 2);
        assert_eq!(queue.current_index(), Some(0));
    }

    #[test]
    fn test_current_track_preserved_on_shuffle_toggle() {
        let mut queue = PlaybackQueue::new();
        queue.add_track(create_test_track(1));
        queue.add_track(create_test_track(2));
        queue.add_track(create_test_track(3));

        queue.jump_to(1);
        let before_shuffle = queue.current().unwrap().id;

        queue.toggle_shuffle();
        let during_shuffle = queue.current().unwrap().id;

        queue.toggle_shuffle();
        let after_shuffle = queue.current().unwrap().id;

        assert_eq!(before_shuffle, during_shuffle);
        assert_eq!(before_shuffle, after_shuffle);
    }
}
