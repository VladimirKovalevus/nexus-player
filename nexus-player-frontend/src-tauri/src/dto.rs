pub use nexus_player_backend::library::types::CacheStatus;

mod download;
mod eq;
mod playback;
mod playlist;
mod queue;
mod search;
mod track;

pub use download::TaskInfoDto;
pub use eq::{EqPresetDto, EqSettingsDto};
pub use playback::{
    AudioStateDto, BufferedRangeDto, CleanupResultDto, PlayHistoryEntryDto, ResolvedArtworkDto,
    WaveformDto,
};
pub use playlist::{
    CreatePlaylistRequest, CreatePlaylistWithTracksRequest, PlaylistDto, PlaylistItemDto,
    PlaylistWithItemsDto,
};
pub use queue::{
    ContainerMetaDto, CurrentPositionDto, LibraryPageDto, QueueEntryDto, QueueStateDto,
    SearchContextDto,
};
pub use search::{SearchResultDto, StreamingTrackDto};
#[cfg(test)]
pub(crate) use search::{StreamingAlbumDto, StreamingChannelDto, StreamingPlaylistDto};
pub use track::{CachedAlbumDto, TrackDto, TrackSourceDto, TrackStateDto};

pub(crate) fn hash_str_to_i64(s: &str) -> i64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    (hasher.finish() & i64::MAX as u64) as i64
}

#[cfg(test)]
mod local_source_tests {
    use super::*;
    use nexus_player_backend::audio::types::TrackSource;

    fn dto(source: Option<TrackSourceDto>) -> TrackDto {
        TrackDto {
            id: 42,
            title: "Local track".into(),
            artist: None,
            album: None,
            duration_secs: None,
            artwork_url: None,
            play_count: 0,
            liked: false,
            state: TrackStateDto::Saved,
            source,
            unavailable: false,
        }
    }

    #[test]
    fn local_source_round_trips_through_dto() {
        let expected = TrackSource::Local("/music/track.flac".into());
        let source_dto: TrackSourceDto = expected.clone().into();
        assert_eq!(source_dto.system, "local");
        assert_eq!(source_dto.url, "/music/track.flac");

        let runtime = dto(Some(source_dto)).try_to_audio_track().unwrap();
        assert_eq!(runtime.source, expected);
    }

    #[test]
    fn missing_source_is_rejected() {
        assert!(dto(None).try_to_audio_track().is_err());
    }
}
