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

pub use crate::playlist::cursor::{ContainerKind, ContainerMeta};

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

/// The album or playlist playback is inside. It stays one row of the snapshot (the
/// current one) while its tracks play; `current_track` is `tracks[index]`.
#[derive(Debug, Clone)]
struct ActiveContainer {
    context: ContainerContext,
    info: ContainerMeta,
    tracks: Vec<Track>,
    index: usize,
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
    /// Set while playback is inside a container row. Its played tracks join `history`
    /// only when playback leaves it, so the snapshot shows them inside the container.
    active: Option<ActiveContainer>,
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
            active: None,
        }
    }

    /// Search results and the nested playlists of a local playlist stay one row until
    /// playback reaches them. Shuffle on a local playlist flattens it instead, so it can
    /// draw from every track.
    fn keeps_root_containers(&self) -> bool {
        match self.context {
            Some(ContainerContext::SearchQuery { .. }) => true,
            Some(ContainerContext::LocalPlaylist(_)) => !self.shuffle_enabled,
            Some(ContainerContext::Radio { .. }) => true,
            _ => false,
        }
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

    /// Takes upcoming row `i` out as the next track to play. A container starts at its
    /// first track (see `activate`); an empty container yields `None`.
    async fn enter_row(&mut self, i: usize) -> Result<Option<Track>> {
        if matches!(self.lookahead.get(i), Some(UpcomingRow::Container { .. })) {
            let tracks = self.container_tracks(i).await?;
            let Some(UpcomingRow::Container { context, info, .. }) = self.lookahead.remove(i)
            else {
                return Ok(None);
            };
            return Ok(self.activate(context, info, tracks, 0, i));
        }
        match self.lookahead.remove(i) {
            Some(UpcomingRow::Track(track)) => Ok(Some(track)),
            _ => Ok(None),
        }
    }

    /// Starts playing `tracks[start]` of a container that was taken out of the lookahead
    /// at position `at`. Without shuffle the container becomes the active one and stays
    /// one row; with shuffle its tracks go back to the lookahead as track rows, so
    /// shuffle can draw from them. Returns `None` for an empty container.
    fn activate(
        &mut self,
        context: ContainerContext,
        info: ContainerMeta,
        mut tracks: Vec<Track>,
        start: usize,
        at: usize,
    ) -> Option<Track> {
        let start = start.min(tracks.len().checked_sub(1)?);
        let track = tracks[start].clone();
        if self.shuffle_enabled {
            let rest = tracks.split_off(start + 1);
            tracks.truncate(start);
            self.history.extend(tracks);
            for (offset, t) in rest.into_iter().enumerate() {
                self.lookahead.insert(at + offset, UpcomingRow::Track(t));
            }
        } else {
            self.active = Some(ActiveContainer {
                context,
                info,
                tracks,
                index: start,
            });
        }
        Some(track)
    }

    /// Leaves the active container after its current track: the tracks played so far
    /// go to history and the rest goes back to the front of the lookahead (one container
    /// row, or track rows under shuffle). Clears `current_track`.
    fn finish_active(&mut self) {
        let Some(mut active) = self.active.take() else {
            return;
        };
        self.current_track = None;
        let rest = active.tracks.split_off(active.index + 1);
        self.history.extend(active.tracks);
        self.trim_history();
        if rest.is_empty() {
            return;
        }
        if self.shuffle_enabled {
            for t in rest.into_iter().rev() {
                self.lookahead.push_front(UpcomingRow::Track(t));
            }
        } else {
            self.lookahead.push_front(UpcomingRow::Container {
                context: active.context,
                info: active.info,
                tracks: Some(rest),
            });
        }
    }

    /// Puts the active container back, whole, at the front of the lookahead (for going
    /// back to earlier tracks: replaying from there replays the container from its start).
    /// Clears `current_track`.
    fn suspend_active(&mut self) {
        let Some(active) = self.active.take() else {
            return;
        };
        self.current_track = None;
        self.lookahead.push_front(UpcomingRow::Container {
            context: active.context,
            info: active.info,
            tracks: Some(active.tracks),
        });
    }

    /// Moves everything before lookahead row `j` (current, user queue, earlier rows) to
    /// history, as if played; skipped container rows are dropped.
    fn pass_to_lookahead(&mut self, j: usize) {
        match self.active.take() {
            Some(active) => {
                self.current_track = None;
                self.history.extend(active.tracks);
            }
            None => self.history.extend(self.current_track.take()),
        }
        self.history.extend(self.user_queue.drain(..));
        for row in self.lookahead.drain(..j) {
            if let UpcomingRow::Track(track) = row {
                self.history.push(track);
            }
        }
        self.trim_history();
    }

    /// Where the current track sits inside the active container, if playback is in one.
    pub fn current_track_within_container(&self) -> Option<usize> {
        self.active.as_ref().map(|a| a.index)
    }

    pub async fn next(&mut self) -> Result<Option<Track>> {
        // Handle Repeat One: just return the current track if it exists
        if self.repeat_mode == RepeatMode::One {
            if let Some(track) = &self.current_track {
                return Ok(Some(track.clone()));
            }
        }

        // Inside a container: go on with its next track unless the user queue cuts in
        if let Some(active) = &mut self.active {
            if !self.shuffle_enabled
                && self.user_queue.is_empty()
                && active.index + 1 < active.tracks.len()
            {
                active.index += 1;
                let track = active.tracks[active.index].clone();
                self.current_track = Some(track.clone());
                return Ok(Some(track));
            }
        }

        // Save current track to history before moving to next
        if self.active.is_some() {
            self.finish_active();
        } else if let Some(track) = self.current_track.take() {
            self.history.push(track);
            self.trim_history();
        }

        if let Some(track) = self.user_queue.pop_front() {
            self.current_track = Some(track.clone());
            return Ok(Some(track));
        }

        if self.shuffle_enabled && self.is_finite_source() {
            // Flatten the remainder so shuffle draws from all of it, including container
            // rows fetched before shuffle was turned on.
            self.prefetch(usize::MAX).await?;
            let mut i = 0;
            while i < self.lookahead.len() {
                if matches!(self.lookahead[i], UpcomingRow::Container { .. }) {
                    self.expand_in_place(i).await?;
                } else {
                    i += 1;
                }
            }
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
        self.active = None;
        Ok(true)
    }

    pub fn previous(&mut self) -> Option<Track> {
        // Handle Repeat One: just return the current track
        if self.repeat_mode == RepeatMode::One {
            if let Some(track) = &self.current_track {
                return Some(track.clone());
            }
        }

        if let Some(active) = &mut self.active {
            if active.index > 0 {
                active.index -= 1;
                let track = active.tracks[active.index].clone();
                self.current_track = Some(track.clone());
                return Some(track);
            }
        }

        let prev = self.history.pop()?;
        self.suspend_active();

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
        if let Some(track) = self.active_rest().first() {
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
            .chain(self.active_rest())
            .cloned()
            .chain(self.lookahead.iter().map_while(|row| match row {
                UpcomingRow::Track(track) => Some(track.clone()),
                UpcomingRow::Container { .. } => None,
            }))
            .take(limit)
            .collect()
    }

    /// The tracks of the active container after the current one.
    fn active_rest(&self) -> &[Track] {
        match &self.active {
            Some(a) => &a.tracks[a.index + 1..],
            None => &[],
        }
    }

    pub fn current(&self) -> Option<&Track> {
        self.current_track.as_ref()
    }

    /// Replaces the queue with a finite list of tracks. They go to the lookahead, so a track
    /// added later by `add_track` plays right after the current one, and shuffle draws
    /// from the whole list.
    pub fn load_tracks(&mut self, tracks: Vec<Track>) {
        self.clear();
        self.lookahead
            .extend(tracks.into_iter().map(UpcomingRow::Track));
    }

    pub fn add_track(&mut self, track: Track) {
        self.user_queue.push_back(track);
    }

    /// Removes snapshot row `index`; a container row removes the whole container.
    /// Returns the removed track when the row was a track.
    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        match self.locate(index)? {
            RowLocation::History(i) => Some(self.history.remove(i)),
            RowLocation::Current => {
                // The active container is the current row: it goes as a whole
                self.active = None;
                self.current_track.take()
            }
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
                self.suspend_active();
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
                // A user track cuts into the active container, as in `next()`
                self.finish_active();
                self.history.extend(self.current_track.take());
                self.history.extend(self.user_queue.drain(..i));
                self.current_track = Some(target);
                self.trim_history();
            }
            Some(RowLocation::Lookahead(j)) => {
                if !matches!(self.lookahead.get(j), Some(UpcomingRow::Track(_))) {
                    return;
                }
                self.pass_to_lookahead(j);
                if let Some(UpcomingRow::Track(target)) = self.lookahead.pop_front() {
                    self.current_track = Some(target);
                }
            }
            Some(RowLocation::Current) | None => {}
        }
    }

    /// Plays snapshot row `index`. On a container row, `track_index` (default: the first)
    /// picks which of its tracks starts and the container becomes the active one; on the
    /// active container (the current row) it moves to that track. An empty container is
    /// removed and nothing plays.
    pub async fn play_queue_item(
        &mut self,
        index: usize,
        track_index: Option<usize>,
    ) -> Result<Option<Track>> {
        match self.locate(index) {
            Some(RowLocation::Current) => {
                if let (Some(active), Some(t)) = (&mut self.active, track_index) {
                    if t < active.tracks.len() {
                        active.index = t;
                        self.current_track = Some(active.tracks[t].clone());
                    }
                }
            }
            Some(RowLocation::Lookahead(i))
                if matches!(self.lookahead.get(i), Some(UpcomingRow::Container { .. })) =>
            {
                let tracks = self.container_tracks(i).await?;
                if tracks.is_empty() {
                    self.lookahead.remove(i);
                    return Ok(None);
                }
                self.pass_to_lookahead(i);
                let Some(UpcomingRow::Container { context, info, .. }) = self.lookahead.pop_front()
                else {
                    return Ok(None);
                };
                self.current_track =
                    self.activate(context, info, tracks, track_index.unwrap_or(0), 0);
            }
            _ => self.jump_to(index),
        }
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
        content_type: impl Into<Option<ContentType>>,
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
        content_type: impl Into<Option<ContentType>>,
        position: usize,
        expected_id: Option<&str>,
    ) -> Result<Track> {
        let content_type = content_type.into();
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
        self.active = None;
        // Must be reset too: a stale context would make Repeat::All resurrect the old
        // source, and would misreport the active radio session to the UI.
        self.context = None;
    }

    /// Creates the row list the UI shows: history tracks, the current track, then the
    /// upcoming part. Returns `(rows, current_row_index)`.
    ///
    /// Row model: a row is a track or a container. Containers are the albums and
    /// playlists the root of a search yielded, the nested playlists of a local playlist
    /// (without shuffle) and the playlists of a radio session. An upcoming container row
    /// stays unexpanded until previewed or reached. The container playback is in (the
    /// active one) is the current row, with its tracks attached;
    /// `current_track_within_container()` says which one plays, and its played tracks join
    /// history only when playback leaves it. Every row index the UI sends back
    /// (`remove_track`, `jump_to`, `play_queue_item`, `expand_container`) refers to this list.
    pub fn snapshot(&self) -> (Vec<QueueEntry>, Option<usize>) {
        let shown = &self.history[self.history.len() - self.history_window_len()..];
        let mut rows =
            Vec::with_capacity(shown.len() + 1 + self.user_queue.len() + self.lookahead.len());
        rows.extend(shown.iter().cloned().map(QueueEntry::Track));
        let current_index = self.current_track.as_ref().map(|current| {
            rows.push(match &self.active {
                Some(active) => QueueEntry::Container {
                    info: active.info.clone(),
                    tracks: Some(active.tracks.clone()),
                    play_index: active.index,
                },
                None => QueueEntry::Track(current.clone()),
            });
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
