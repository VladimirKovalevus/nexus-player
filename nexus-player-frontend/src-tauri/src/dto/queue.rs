use super::TrackDto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerMetaDto {
    pub kind: String,
    pub id: String,
    pub system: String,
    pub title: String,
    pub artist: String,
    pub track_count: usize,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "entryType")]
pub enum QueueEntryDto {
    #[serde(rename = "track")]
    Track { track: TrackDto },
    #[serde(rename = "container")]
    Container {
        meta: ContainerMetaDto,
        tracks: Option<Vec<TrackDto>>,
        play_index: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentPositionDto {
    pub item_index: usize,
    pub track_within_container: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchContextDto {
    pub system: String,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPageDto {
    pub current_page: usize,
    pub total_pages: usize,
    pub page_size: usize,
    pub total_tracks: usize,
    pub track_index_in_page: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStateDto {
    pub entries: Vec<QueueEntryDto>,
    pub current_position: Option<CurrentPositionDto>,
    pub repeat_mode: String,
    pub shuffle_enabled: bool,
    pub search_context: Option<SearchContextDto>,
    /// "tracks" | "playlists" when a radio session is loaded, otherwise None.
    pub radio_mode: Option<String>,
}
