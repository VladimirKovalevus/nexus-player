use super::{hash_str_to_i64, TrackDto, TrackSourceDto};
use nexus_player_backend::library::models;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDto {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub thumbnail: Option<String>,
    pub track_count: usize,
    pub created_at: i64,
    pub updated_at: i64,
    pub source: Option<TrackSourceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "itemType")]
pub enum PlaylistItemDto {
    #[serde(rename = "track")]
    Track {
        id: i64,
        item_id: i64,
        track: TrackDto,
        position: u32,
    },
    #[serde(rename = "playlist")]
    Playlist {
        id: i64,
        item_id: i64,
        playlist: PlaylistDto,
        position: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistWithItemsDto {
    pub playlist: PlaylistDto,
    pub items: Vec<PlaylistItemDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlaylistRequest {
    pub title: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlaylistWithTracksRequest {
    pub title: String,
    pub description: Option<String>,
    pub artwork_url: Option<String>,
    pub tracks: Vec<TrackDto>,
}

impl From<models::Playlist> for PlaylistDto {
    fn from(playlist: models::Playlist) -> Self {
        let thumbnail = playlist
            .metadata
            .artwork
            .as_ref()
            .and_then(|art| match &art.source {
                nexus_player_backend::library::types::ArtworkSource::Url { url } => {
                    Some(url.clone())
                }
                _ => None,
            });

        Self {
            id: playlist.id.0,
            title: playlist.metadata.title,
            description: playlist.metadata.description,
            creator: playlist.creator,
            thumbnail,
            track_count: 0,
            created_at: playlist.created_at.timestamp(),
            updated_at: playlist.updated_at.timestamp(),
            source: None,
        }
    }
}

impl From<nexus_player_backend::streaming::types::PlaylistInfo> for PlaylistDto {
    fn from(playlist: nexus_player_backend::streaming::types::PlaylistInfo) -> Self {
        Self {
            id: hash_str_to_i64(&playlist.id),
            title: playlist.title,
            description: playlist.description,
            creator: Some(playlist.creator),
            thumbnail: playlist.thumbnail,
            track_count: playlist.track_count,
            created_at: 0,
            updated_at: 0,
            source: Some(TrackSourceDto {
                url: playlist.id,
                system: playlist.service.to_string(),
            }),
        }
    }
}
