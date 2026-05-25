use serde::{Deserialize, Serialize};

use crate::audio::types::Track;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerContext {
    // Local DB sources
    LocalPlaylist(i64),
    LocalAlbum(i64),

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
