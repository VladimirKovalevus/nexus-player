use serde::{Deserialize, Serialize};

use crate::audio::types::Track;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerContext {
    // Local DB sources
    LocalPlaylist(i64),
    LocalAlbum(i64),

    // External Provider sources
    ExternalPlaylist { system: String, id: String },
    ExternalAlbum { system: String, id: String },

    // Dynamic sources
    SearchQuery { system: String, query: String },
}

#[derive(Debug, Clone)]
pub enum CursorItem {
    Track(Track),
    Container(ContainerContext),
}
