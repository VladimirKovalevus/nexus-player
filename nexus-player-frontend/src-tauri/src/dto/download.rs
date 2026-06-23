use super::{hash_str_to_i64, StreamingTrackDto, TrackSourceDto, TrackStateDto};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfoDto {
    pub track_id: u64,
    pub url: String,
    pub state: String,
    pub size_human: String,
    pub byte_size: u64,
    pub progress: f64,
    pub meta: Option<StreamingTrackDto>,
}

impl From<::streaming::state::types::TaskInfo> for TaskInfoDto {
    fn from(info: ::streaming::state::types::TaskInfo) -> Self {
        let meta = info.meta.map(|m| {
            let (url, system) = (info.url.clone(), "unknown".to_string()); // We don't have system here, but TaskInfo usually has URL
            StreamingTrackDto {
                id: Some(hash_str_to_i64(&url)),
                title: m.title.unwrap_or_default(),
                artist: m.artist.unwrap_or_default(),
                duration_secs: m.duration.map(|d| d.as_secs_f64()),
                thumbnail: m.artwork_url,
                source: TrackSourceDto { url, system },
                state: TrackStateDto::Streaming,
                liked: false,
            }
        });

        Self {
            track_id: info.track_id,
            url: info.url,
            state: info.state,
            size_human: info.size_human,
            byte_size: info.byte_size,
            progress: info.progress,
            meta,
        }
    }
}
