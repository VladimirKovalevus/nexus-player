use super::{hash_str_to_i64, TrackSourceDto, TrackStateDto};
use nexus_player_backend::streaming::types as streaming;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum SearchResultDto {
    Track(StreamingTrackDto),
    Playlist(StreamingPlaylistDto),
    Album(StreamingAlbumDto),
    Channel(StreamingChannelDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingTrackDto {
    pub id: Option<i64>,
    pub title: String,
    pub artist: String,
    pub duration_secs: Option<f64>,
    pub thumbnail: Option<String>,
    pub source: TrackSourceDto,
    pub state: TrackStateDto,
    pub liked: bool,
}

impl StreamingTrackDto {
    pub fn to_audio_track(&self) -> nexus_player_backend::audio::types::Track {
        use nexus_player_backend::audio::types::{Track, TrackSource, TrackState};

        Track {
            id: hash_str_to_i64(&self.source.url) as usize,
            title: self.title.clone(),
            artist: Some(self.artist.clone()),
            album: None,
            duration: self.duration_secs.map(std::time::Duration::from_secs_f64),
            artwork_url: self.thumbnail.clone(),
            format: None,
            source: TrackSource::External {
                id: self.source.url.clone(),
                system: self.source.system.clone(),
            },
            state: TrackState::Streaming,
            liked: self.liked,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingPlaylistDto {
    pub id: String,
    pub title: String,
    pub creator: String,
    pub track_count: usize,
    pub thumbnail: Option<String>,
    pub tracks: Vec<StreamingTrackDto>,
    pub url: String,
    pub system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingAlbumDto {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub track_count: usize,
    pub year: Option<u32>,
    pub thumbnail: Option<String>,
    pub tracks: Vec<StreamingTrackDto>,
    pub url: String,
    pub system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingChannelDto {
    pub id: String,
    pub name: String,
    pub subscriber_count: Option<u64>,
    pub thumbnail: Option<String>,
    pub url: String,
    pub system: String,
}

impl From<streaming::SearchResult> for SearchResultDto {
    fn from(result: streaming::SearchResult) -> Self {
        match result {
            streaming::SearchResult::Track(track) => {
                let (url, system) = match &track.source {
                    nexus_player_backend::library::TrackSource::External { id, system } => {
                        (id.clone(), system.clone())
                    }
                    _ => (track.source.as_str(), "unknown".to_string()),
                };

                SearchResultDto::Track(StreamingTrackDto {
                    id: Some(hash_str_to_i64(&url)),
                    title: track.title,
                    artist: track.artist,
                    duration_secs: track.duration.map(|d| d.as_secs_f64()),
                    thumbnail: track.thumbnail,
                    source: TrackSourceDto { url, system },
                    state: TrackStateDto::Streaming,
                    liked: false,
                })
            }
            streaming::SearchResult::Playlist(playlist) => {
                SearchResultDto::Playlist(StreamingPlaylistDto {
                    id: playlist.id,
                    title: playlist.title,
                    creator: playlist.creator,
                    track_count: playlist.track_count,
                    thumbnail: playlist.thumbnail,
                    tracks: playlist.tracks.into_iter().map(|el| el.into()).collect(),
                    url: playlist.url,
                    system: playlist.service.to_string(),
                })
            }
            streaming::SearchResult::Album(album) => SearchResultDto::Album(StreamingAlbumDto {
                id: album.id,
                title: album.title,
                artist: album.artist,
                track_count: album.track_count,
                year: album.year,
                thumbnail: album.thumbnail,
                tracks: album.tracks.into_iter().map(|el| el.into()).collect(),
                url: album.url,
                system: album.service.to_string(),
            }),
            streaming::SearchResult::Channel(channel) => {
                SearchResultDto::Channel(StreamingChannelDto {
                    id: channel.id,
                    name: channel.name,
                    subscriber_count: channel.subscriber_count,
                    thumbnail: channel.thumbnail,
                    url: channel.url,
                    system: channel.service.to_string(),
                })
            }
        }
    }
}
impl From<streaming::TrackInfo> for StreamingTrackDto {
    fn from(track: streaming::TrackInfo) -> Self {
        let (url, system) = match &track.source {
            nexus_player_backend::library::TrackSource::External { id, system } => {
                (id.clone(), system.clone())
            }
            _ => (track.source.as_str(), "unknown".to_string()),
        };

        Self {
            id: Some(hash_str_to_i64(&url)),
            title: track.title,
            artist: track.artist,
            duration_secs: track.duration.map(|d| d.as_secs_f64()),
            thumbnail: track.thumbnail,
            source: TrackSourceDto { url, system },
            state: TrackStateDto::Streaming,
            liked: false,
        }
    }
}

impl From<SearchResultDto> for streaming::SearchResult {
    fn from(dto: SearchResultDto) -> Self {
        match dto {
            SearchResultDto::Track(track) => streaming::SearchResult::Track(streaming::TrackInfo {
                source: nexus_player_backend::library::TrackSource::External {
                    id: track.source.url,
                    system: track.source.system,
                },
                title: track.title,
                artist: track.artist,
                duration: track.duration_secs.map(std::time::Duration::from_secs_f64),
                thumbnail: track.thumbnail,
                description: None,
            }),
            SearchResultDto::Playlist(playlist) => {
                let service = std::str::FromStr::from_str(&playlist.system)
                    .unwrap_or(streaming::StreamingService::Bandcamp);
                streaming::SearchResult::Playlist(streaming::PlaylistInfo {
                    id: playlist.id,
                    url: playlist.url,
                    service,
                    title: playlist.title,
                    creator: playlist.creator,
                    tracks: playlist
                        .tracks
                        .into_iter()
                        .map(|t| streaming::TrackInfo {
                            source: nexus_player_backend::library::TrackSource::External {
                                id: t.source.url,
                                system: t.source.system,
                            },
                            title: t.title,
                            artist: t.artist,
                            duration: t.duration_secs.map(std::time::Duration::from_secs_f64),
                            thumbnail: t.thumbnail,
                            description: None,
                        })
                        .collect(),
                    track_count: playlist.track_count,
                    thumbnail: playlist.thumbnail,
                    description: None,
                })
            }
            SearchResultDto::Album(album) => {
                let service = std::str::FromStr::from_str(&album.system)
                    .unwrap_or(streaming::StreamingService::Bandcamp);
                streaming::SearchResult::Album(streaming::AlbumInfo {
                    id: album.id,
                    url: album.url,
                    service,
                    title: album.title,
                    artist: album.artist,
                    tracks: album
                        .tracks
                        .into_iter()
                        .map(|t| streaming::TrackInfo {
                            source: nexus_player_backend::library::TrackSource::External {
                                id: t.source.url,
                                system: t.source.system,
                            },
                            title: t.title,
                            artist: t.artist,
                            duration: t.duration_secs.map(std::time::Duration::from_secs_f64),
                            thumbnail: t.thumbnail,
                            description: None,
                        })
                        .collect(),
                    track_count: album.track_count,
                    year: album.year,
                    thumbnail: album.thumbnail,
                    description: None,
                })
            }
            SearchResultDto::Channel(channel) => {
                let service = std::str::FromStr::from_str(&channel.system)
                    .unwrap_or(streaming::StreamingService::Bandcamp);
                streaming::SearchResult::Channel(streaming::ChannelInfo {
                    id: channel.id,
                    url: channel.url,
                    service,
                    name: channel.name,
                    subscriber_count: channel.subscriber_count,
                    track_count: None,
                    thumbnail: channel.thumbnail,
                    description: None,
                })
            }
        }
    }
}
