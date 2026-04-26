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
        if let Some(track) = self.manual_queue.front() {
            return Some(track.clone());
        }
        None
    }

    pub fn peek_upcoming_ids(&self, limit: usize) -> Vec<u64> {
        self.manual_queue
            .iter()
            .take(limit)
            .map(|t| t.id as u64)
            .collect()
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

    pub fn peek_upcoming_tracks(&self, limit: usize) -> Vec<Track> {
        let current = match self.current_index {
            Some(i) => i,
            None => return vec![],
        };

        let mut result = Vec::with_capacity(limit);

        // Remaining tracks inside the current container (if any)
        if let Some(SearchQueueItem::Container {
            expanded_tracks: Some(tracks),
            play_index,
            ..
        }) = self.items.get(current)
        {
            for track in &tracks[(*play_index + 1).min(tracks.len())..] {
                if result.len() >= limit {
                    return result;
                }
                result.push(track.clone());
            }
        }

        // Subsequent items in the queue
        for item in &self.items[current + 1..] {
            if result.len() >= limit {
                break;
            }
            match item {
                SearchQueueItem::Track(track) => result.push(track.clone()),
                SearchQueueItem::Container {
                    expanded_tracks: Some(tracks),
                    ..
                } => {
                    for track in tracks {
                        if result.len() >= limit {
                            break;
                        }
                        result.push(track.clone());
                    }
                }
                SearchQueueItem::Container {
                    expanded_tracks: None,
                    ..
                } => {} // Can't peek inside unexpanded container
            }
        }

        result
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

    pub fn peek_upcoming_ids(&self, limit: usize) -> Vec<u64> {
        match self {
            ActiveQueue::Cursor(o) => o.peek_upcoming_ids(limit),
            ActiveQueue::Search(_) => vec![],
        }
    }

    pub fn peek_upcoming_tracks(&self, limit: usize) -> Vec<Track> {
        match self {
            ActiveQueue::Cursor(o) => o.peek_upcoming_tracks(limit),
            ActiveQueue::Search(o) => o.peek_upcoming_tracks(limit),
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
    use crate::audio::types::{TrackSource, TrackState};

    fn create_test_track(id: usize) -> Track {
        Track {
            id,
            title: format!("Track {}", id),
            artist: None,
            album: None,
            duration: None,
            artwork_url: None,
            format: None,
            source: TrackSource::local(format!("/path/to/track{}.mp3", id)),
            state: TrackState::Saved,
            liked: false,
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
        assert!(queue.next().is_none());
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
        let mut sq = queue_with(vec![container_item("c", Some(vec![track(10), track(20), track(30)]))]);
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
        let mut sq = queue_with(vec![container_item("c", Some(vec![track(1), track(2), track(3)]))]);
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
        let result = sq.next();
        assert!(matches!(result, NextResult::Track(t) if t.id == 5));
    }

    #[test]
    fn next_on_empty_queue_returns_end() {
        let mut sq = queue_with(vec![]);
        assert!(matches!(sq.next(), NextResult::End));
    }

    #[test]
    fn next_on_empty_queue_with_more_pages_returns_need_more() {
        let mut sq = SearchQueue::new("s".into(), "q".into(), vec![], true);
        assert!(matches!(sq.next(), NextResult::NeedMorePages));
    }

    #[test]
    fn next_advances_through_track_items() {
        let mut sq = queue_with(vec![track_item(1), track_item(2), track_item(3)]);
        sq.jump_to(0);
        let r1 = sq.next();
        let r2 = sq.next();
        assert!(matches!(r1, NextResult::Track(t) if t.id == 2));
        assert!(matches!(r2, NextResult::Track(t) if t.id == 3));
    }

    #[test]
    fn next_returns_end_after_last_track() {
        let mut sq = queue_with(vec![track_item(1)]);
        sq.jump_to(0);
        assert!(matches!(sq.next(), NextResult::End));
    }

    #[test]
    fn next_returns_need_more_pages_after_last_item() {
        let mut sq = SearchQueue::new("s".into(), "q".into(), vec![track_item(1)], true);
        sq.jump_to(0);
        assert!(matches!(sq.next(), NextResult::NeedMorePages));
    }

    #[test]
    fn next_on_unexpanded_container_returns_need_expand() {
        let mut sq = queue_with(vec![track_item(1), container_item("c", None)]);
        sq.jump_to(0);
        assert!(matches!(sq.next(), NextResult::NeedExpand(1)));
    }

    #[test]
    fn next_on_expanded_container_returns_first_track() {
        let mut sq = queue_with(vec![track_item(1), container_item("c", Some(vec![track(10), track(20)]))]);
        sq.jump_to(0);
        let result = sq.next();
        assert!(matches!(result, NextResult::Track(t) if t.id == 10));
    }

    #[test]
    fn next_advances_within_container() {
        let mut sq = queue_with(vec![container_item("c", Some(vec![track(1), track(2), track(3)]))]);
        sq.jump_to(0);
        let r1 = sq.next();
        let r2 = sq.next();
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
        sq.next(); // → track(2)
        let result = sq.next(); // → track(99)
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
        let result = sq.next();
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
        let mut sq = queue_with(vec![container_item("c", Some(vec![track(1), track(2), track(3)]))]);
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
        let result = sq.next();
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
        let mut sq = queue_with(vec![container_item("c", Some(vec![track(1), track(2), track(3)]))]);
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
        if let QueueEntry::Container { tracks, play_index, .. } = &entries[0] {
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
        assert_eq!(upcoming.iter().map(|t| t.id).collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn peek_upcoming_tracks_returns_remaining_tracks_in_current_container() {
        let mut sq = queue_with(vec![
            container_item("c", Some(vec![track(10), track(20), track(30)])),
        ]);
        sq.jump_to(0); // play_index=0, current=track(10)
        let upcoming = sq.peek_upcoming_tracks(9);
        assert_eq!(upcoming.iter().map(|t| t.id).collect::<Vec<_>>(), vec![20, 30]);
    }

    #[test]
    fn peek_upcoming_tracks_combines_container_remainder_and_next_items() {
        let mut sq = queue_with(vec![
            container_item("c", Some(vec![track(1), track(2), track(3)])),
            track_item(99),
        ]);
        sq.jump_to(0); // play_index=0
        let upcoming = sq.peek_upcoming_tracks(9);
        assert_eq!(upcoming.iter().map(|t| t.id).collect::<Vec<_>>(), vec![2, 3, 99]);
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
        assert_eq!(upcoming.iter().map(|t| t.id).collect::<Vec<_>>(), vec![10, 20, 99]);
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
        let mut sq = queue_with(vec![
            container_item("c", Some(vec![track(10), track(20), track(30)])),
        ]);
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
