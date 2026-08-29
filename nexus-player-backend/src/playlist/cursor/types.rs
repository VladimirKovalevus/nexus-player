use serde::{Deserialize, Serialize};

use crate::audio::types::Track;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerContext {
    // Local DB sources
    LocalPlaylist(i64),

    // External Provider sources
    ExternalPlaylist {
        system: String,
        id: String,
    },
    ExternalAlbum {
        system: String,
        id: String,
    },

    // Dynamic sources
    SearchQuery {
        system: String,
        query: String,
        content_type: crate::streaming::types::ContentType,
    },

    // Radio: a precomputed play order. The shuffled ids live in the context itself so
    // the queue replays the exact same order when Repeat::All restarts it.
    Radio {
        mode: RadioMode,
        track_ids: Vec<i64>,
    },
}

/// What a radio session is built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadioMode {
    /// Every track in the library, shuffled.
    Tracks,
    /// Root playlists (each kept intact and in order) interleaved at random with the
    /// tracks that belong to no playlist.
    Playlists,
}

#[derive(Debug, Clone)]
pub enum CursorItem {
    Track(Track),
    Container(ContainerContext),
}

/// Type of container in search results
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerKind {
    Album,
    Playlist,
}

/// Metadata for a container (album or playlist) taken from its search result.
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

/// What `PlayerCursor::next_row` yields: a playable track, or a root-level container that
/// was left unexpanded.
#[derive(Debug, Clone)]
pub enum CursorRow {
    Track(Track),
    Container {
        context: ContainerContext,
        meta: ContainerMeta,
    },
}
