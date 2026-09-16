use crate::audio::types::Track;
use crate::playlist::cursor::{
    ContainerContext, CursorResolver, CursorRow, PlayerCursor, SearchPageSource,
    StreamingSearchCursor,
};
use crate::streaming::types::ContentType;
use anyhow::{Context, Result};
use rand::{thread_rng, Rng};
use std::collections::VecDeque;
use std::sync::Arc;

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

/// History entries kept in memory; older ones are dropped.
const HISTORY_CAP: usize = 500;
/// History entries `snapshot()` sends to the UI.
const HISTORY_WINDOW: usize = 50;

/// Where a snapshot row index points.
enum RowLocation {
    History(usize),
    Current,
    User(usize),
    Lookahead(usize),
}

pub struct CursorQueue {
    pub history: Vec<Track>,
    pub current_track: Option<Track>,
    pub cursor: Option<PlayerCursor>,
    /// Tracks the user added; they play before the lookahead.
    user_queue: VecDeque<Track>,
    /// Rows prefetched from the cursor.
    lookahead: VecDeque<UpcomingRow>,
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
            user_queue: VecDeque::new(),
            lookahead: VecDeque::new(),
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            context: None,
        }
    }

    /// Search contexts keep album and playlist results as container rows.
    fn keeps_root_containers(&self) -> bool {
        matches!(self.context, Some(ContainerContext::SearchQuery { .. }))
    }

    /// How many history rows a snapshot shows.
    fn history_window_len(&self) -> usize {
        self.history.len().min(HISTORY_WINDOW)
    }

    /// Drops the oldest history entries beyond the cap.
    fn trim_history(&mut self) {
        if self.history.len() > HISTORY_CAP {
            let excess = self.history.len() - HISTORY_CAP;
            self.history.drain(..excess);
        }
    }

    /// The one place that translates a snapshot row index (windowed history, current,
    /// upcoming) into a position in the queue.
    fn locate(&self, row: usize) -> Option<RowLocation> {
        let window_len = self.history_window_len();
        let offset = self.history.len() - window_len;
        let current_len = usize::from(self.current_track.is_some());
        if row < window_len {
            Some(RowLocation::History(row + offset))
        } else if row < window_len + current_len {
            Some(RowLocation::Current)
        } else {
            let i = row - window_len - current_len;
            if i < self.user_queue.len() {
                Some(RowLocation::User(i))
            } else {
                let j = i - self.user_queue.len();
                (j < self.lookahead.len()).then_some(RowLocation::Lookahead(j))
            }
        }
    }

    /// A finite local source is flattened for shuffle; anything else is paged or endless.
    fn is_finite_source(&self) -> bool {
        matches!(
            self.context,
            None | Some(ContainerContext::LocalPlaylist(_))
        )
    }

    /// Pulls one more row from the cursor into the upcoming part.
    async fn fetch_one(&mut self) -> Result<bool> {
        let keep = self.keeps_root_containers();
        let Some(cursor) = &mut self.cursor else {
            return Ok(false);
        };
        match cursor.next_row(keep).await? {
            Some(CursorRow::Track(track)) => self.lookahead.push_back(UpcomingRow::Track(track)),
            Some(CursorRow::Container { context, meta }) => {
                self.lookahead.push_back(UpcomingRow::Container {
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
        }) = self.lookahead.get(i)
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
        self.lookahead.remove(i);
        for (offset, track) in tracks.into_iter().enumerate() {
            self.lookahead.insert(i + offset, UpcomingRow::Track(track));
        }
        Ok(count)
    }

    /// Takes upcoming row `i` as the current track. A container is replaced by its
    /// tracks and its first one is taken; an empty container yields `None`.
    async fn enter_row(&mut self, i: usize) -> Result<Option<Track>> {
        if matches!(self.lookahead.get(i), Some(UpcomingRow::Container { .. })) {
            self.expand_in_place(i).await?;
        }
        match self.lookahead.remove(i) {
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
            self.trim_history();
        }

        if let Some(track) = self.user_queue.pop_front() {
            self.current_track = Some(track.clone());
            return Ok(Some(track));
        }

        if self.shuffle_enabled && self.is_finite_source() {
            // Flatten the remainder so shuffle draws from all of it
            self.prefetch(usize::MAX).await?;
        }

        let mut restarted = false;
        loop {
            if self.lookahead.is_empty() {
                if self.shuffle_enabled {
                    // Prefetch a window so there is something to shuffle
                    self.prefetch(50).await?;
                } else {
                    self.fetch_one().await?;
                }
                if self.lookahead.is_empty() {
                    if self.repeat_mode == RepeatMode::All && !restarted && self.restart().await? {
                        restarted = true;
                        continue;
                    }
                    return Ok(None);
                }
            }
            let idx = if self.shuffle_enabled {
                thread_rng().gen_range(0..self.lookahead.len())
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

    /// Repeat All: rebuilds the cursor from `context`. This is the only restart path.
    /// It calls the resolver while the caller holds the queue lock, which is acceptable
    /// because only local and search contexts restart and their roots resolve quickly.
    async fn restart(&mut self) -> Result<bool> {
        let (Some(context), Some(cursor)) = (self.context.clone(), &self.cursor) else {
            return Ok(false);
        };
        let resolver = cursor.resolver();
        self.cursor = Some(PlayerCursor::from_context(context, resolver).await?);
        self.lookahead.clear();
        Ok(true)
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
            self.user_queue.push_front(current);
        }

        self.current_track = Some(prev.clone());
        Some(prev)
    }

    pub fn peek_next(&self) -> Option<Track> {
        if let Some(track) = self.user_queue.front() {
            return Some(track.clone());
        }
        match self.lookahead.front() {
            Some(UpcomingRow::Track(track)) => Some(track.clone()),
            _ => None,
        }
    }

    pub fn peek_upcoming_tracks(&self, limit: usize) -> Vec<Track> {
        self.user_queue
            .iter()
            .cloned()
            .chain(self.lookahead.iter().map_while(|row| match row {
                UpcomingRow::Track(track) => Some(track.clone()),
                UpcomingRow::Container { .. } => None,
            }))
            .take(limit)
            .collect()
    }

    pub fn current(&self) -> Option<&Track> {
        self.current_track.as_ref()
    }

    pub fn add_track(&mut self, track: Track) {
        self.user_queue.push_back(track);
    }

    /// Removes snapshot row `index`; a container row removes the whole container.
    /// Returns the removed track when the row was a track.
    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        match self.locate(index)? {
            RowLocation::History(i) => Some(self.history.remove(i)),
            RowLocation::Current => self.current_track.take(),
            RowLocation::User(i) => self.user_queue.remove(i),
            RowLocation::Lookahead(i) => match self.lookahead.remove(i)? {
                UpcomingRow::Track(track) => Some(track),
                UpcomingRow::Container { .. } => None,
            },
        }
    }

    /// Jump to a track row of the snapshot, rearranging history/current/upcoming.
    /// Container rows are not valid targets (see `play_queue_item`) and, when jumped
    /// over, are dropped: history holds tracks only.
    pub fn jump_to(&mut self, index: usize) {
        match self.locate(index) {
            Some(RowLocation::History(i)) => {
                // Replay order: the tracks after the target, then the old current.
                let mut replay: Vec<Track> = self.history.split_off(i + 1);
                let Some(target) = self.history.pop() else {
                    return;
                };
                replay.extend(self.current_track.take());
                for track in replay.into_iter().rev() {
                    self.user_queue.push_front(track);
                }
                self.current_track = Some(target);
            }
            Some(RowLocation::User(i)) => {
                let Some(target) = self.user_queue.remove(i) else {
                    return;
                };
                self.history.extend(self.current_track.take());
                self.history.extend(self.user_queue.drain(..i));
                self.current_track = Some(target);
                self.trim_history();
            }
            Some(RowLocation::Lookahead(j)) => {
                if !matches!(self.lookahead.get(j), Some(UpcomingRow::Track(_))) {
                    return;
                }
                self.history.extend(self.current_track.take());
                self.history.extend(self.user_queue.drain(..));
                for row in self.lookahead.drain(..j) {
                    if let UpcomingRow::Track(track) = row {
                        self.history.push(track);
                    }
                }
                if let Some(UpcomingRow::Track(target)) = self.lookahead.pop_front() {
                    self.current_track = Some(target);
                }
                self.trim_history();
            }
            Some(RowLocation::Current) | None => {}
        }
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
        if let Some(RowLocation::Lookahead(i)) = self.locate(index) {
            if matches!(self.lookahead.get(i), Some(UpcomingRow::Container { .. })) {
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
        let Some(RowLocation::Lookahead(i)) = self.locate(index) else {
            anyhow::bail!("Row {index} is not a container");
        };
        let fetched = self.container_tracks(i).await?;
        if let Some(UpcomingRow::Container { tracks, .. }) = self.lookahead.get_mut(i) {
            *tracks = Some(fetched);
        }
        Ok(())
    }

    /// Starts playing a search at raw result `position`. Only the page holding the
    /// position is fetched; an album or playlist there counts as one position and its
    /// first track becomes current. Fails when there is no result at the position or it
    /// is a channel.
    pub async fn start_search_at(
        &mut self,
        pages: Arc<dyn SearchPageSource>,
        resolver: Arc<dyn CursorResolver>,
        system: &str,
        query: &str,
        content_type: ContentType,
        position: usize,
    ) -> Result<Track> {
        self.start_search_at_checked(pages, resolver, system, query, content_type, position, None)
            .await
    }

    /// Like `start_search_at`, and fails when the result at the position has an id other
    /// than `expected_id` (the results changed since the UI listed them).
    #[allow(clippy::too_many_arguments)]
    pub async fn start_search_at_checked(
        &mut self,
        pages: Arc<dyn SearchPageSource>,
        resolver: Arc<dyn CursorResolver>,
        system: &str,
        query: &str,
        content_type: ContentType,
        position: usize,
        expected_id: Option<&str>,
    ) -> Result<Track> {
        self.clear();
        let source = StreamingSearchCursor::with_source(
            pages,
            system.to_string(),
            query.to_string(),
            content_type,
        )
        .starting_at(position);
        self.context = Some(ContainerContext::SearchQuery {
            system: system.to_string(),
            query: query.to_string(),
            content_type,
        });
        self.set_cursor(PlayerCursor::new(Box::new(source), resolver));

        if !self.fetch_one().await? {
            anyhow::bail!("No search result at position {position}");
        }
        if let Some(expected) = expected_id {
            let actual = match self.lookahead.front() {
                Some(UpcomingRow::Track(track)) => match &track.source {
                    crate::audio::types::TrackSource::External { id, .. } => Some(id.clone()),
                    _ => None,
                },
                Some(UpcomingRow::Container { info, .. }) => Some(info.id.clone()),
                None => None,
            };
            if actual.as_deref() != Some(expected) {
                anyhow::bail!(
                    "Search results changed: the chosen result is no longer at position {position}"
                );
            }
        }
        // An empty album or playlist yields nothing: go on with the next result.
        loop {
            if let Some(track) = self.enter_row(0).await? {
                self.current_track = Some(track.clone());
                return Ok(track);
            }
            if !self.fetch_one().await? {
                anyhow::bail!("The chosen search result has no tracks");
            }
        }
    }

    pub fn set_cursor(&mut self, cursor: PlayerCursor) {
        tracing::info!("Setting cursor in queue");
        self.cursor = Some(cursor);
    }

    /// Fills the upcoming part from the cursor until it holds `limit` rows (a container
    /// row counts as one). Returns how many rows were added.
    pub async fn prefetch(&mut self, limit: usize) -> Result<usize> {
        let mut count = 0;
        while self.lookahead.len() < limit && self.fetch_one().await? {
            count += 1;
        }
        Ok(count)
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.current_track = None;
        self.cursor = None;
        self.user_queue.clear();
        self.lookahead.clear();
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
        let shown = &self.history[self.history.len() - self.history_window_len()..];
        let mut rows =
            Vec::with_capacity(shown.len() + 1 + self.user_queue.len() + self.lookahead.len());
        rows.extend(shown.iter().cloned().map(QueueEntry::Track));
        let current_index = self.current_track.as_ref().map(|current| {
            rows.push(QueueEntry::Track(current.clone()));
            rows.len() - 1
        });
        rows.extend(self.user_queue.iter().cloned().map(QueueEntry::Track));
        rows.extend(self.lookahead.iter().map(|row| match row {
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

// === Queue row types ===

pub use crate::playlist::cursor::{ContainerKind, ContainerMeta};

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

/// The queue the app plays from.
pub enum ActiveQueue {
    Cursor(Box<CursorQueue>),
}

impl ActiveQueue {
    pub fn toggle_shuffle(&mut self) {
        let ActiveQueue::Cursor(cq) = self;
        cq.toggle_shuffle();
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
        let ActiveQueue::Cursor(cq) = self;
        cq.peek_next()
    }

    pub fn peek_upcoming_tracks(&self, limit: usize) -> Vec<Track> {
        let ActiveQueue::Cursor(cq) = self;
        cq.peek_upcoming_tracks(limit)
    }

    pub fn current_track(&self) -> Option<Track> {
        let ActiveQueue::Cursor(cq) = self;
        cq.current().cloned()
    }
}
