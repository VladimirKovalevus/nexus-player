//! T3 R4: the DTO wire format, pinned against the unsplit `dto.rs`.
//!
//! Each serialization test compares `serde_json::to_value` with a full JSON literal, so keys,
//! tags, enum spellings, nesting and how `None` is written are all fixed. Each deserialization
//! test parses the camelCase JSON the frontend sends. Floats are exact in binary.

use crate::dto::{
    AudioStateDto, BufferedRangeDto, ContainerMetaDto, CreatePlaylistRequest,
    CreatePlaylistWithTracksRequest, CurrentPositionDto, EqPresetDto, EqSettingsDto, PlaylistDto,
    PlaylistItemDto, QueueEntryDto, QueueStateDto, SearchContextDto, SearchResultDto,
    StreamingAlbumDto, StreamingChannelDto, StreamingPlaylistDto, StreamingTrackDto, TaskInfoDto,
    TrackDto, TrackSourceDto, TrackStateDto, WaveformDto,
};
use serde_json::{json, Value};

fn full_track() -> TrackDto {
    TrackDto {
        id: 7,
        title: "Song".into(),
        artist: Some("Artist".into()),
        album: Some("Album".into()),
        duration_secs: Some(180.5),
        artwork_url: Some("https://img.example/a.jpg".into()),
        play_count: 3,
        liked: true,
        state: TrackStateDto::Cached,
        source: Some(TrackSourceDto {
            url: "https://example.com/t".into(),
            system: "soundcloud".into(),
        }),
        unavailable: true,
    }
}

fn full_track_json() -> Value {
    json!({
        "id": 7,
        "title": "Song",
        "artist": "Artist",
        "album": "Album",
        "durationSecs": 180.5,
        "artworkUrl": "https://img.example/a.jpg",
        "playCount": 3,
        "liked": true,
        "state": { "type": "cached" },
        "source": { "url": "https://example.com/t", "system": "soundcloud" },
        "unavailable": true
    })
}

fn bare_track() -> TrackDto {
    TrackDto {
        id: 1,
        title: "Bare".into(),
        artist: None,
        album: None,
        duration_secs: None,
        artwork_url: None,
        play_count: 0,
        liked: false,
        state: TrackStateDto::Saved,
        source: None,
        unavailable: false,
    }
}

fn bare_track_json() -> Value {
    json!({
        "id": 1,
        "title": "Bare",
        "artist": null,
        "album": null,
        "durationSecs": null,
        "artworkUrl": null,
        "playCount": 0,
        "liked": false,
        "state": { "type": "saved" },
        "source": null,
        "unavailable": false
    })
}

fn playlist() -> PlaylistDto {
    PlaylistDto {
        id: 11,
        title: "Mix".into(),
        description: Some("Desc".into()),
        creator: Some("Me".into()),
        thumbnail: Some("thumb.png".into()),
        track_count: 4,
        created_at: 100,
        updated_at: 200,
        source: Some(TrackSourceDto {
            url: "set-id".into(),
            system: "bandcamp".into(),
        }),
    }
}

fn playlist_json() -> Value {
    json!({
        "id": 11,
        "title": "Mix",
        "description": "Desc",
        "creator": "Me",
        "thumbnail": "thumb.png",
        "trackCount": 4,
        "createdAt": 100,
        "updatedAt": 200,
        "source": { "url": "set-id", "system": "bandcamp" }
    })
}

fn streaming_track() -> StreamingTrackDto {
    StreamingTrackDto {
        id: Some(5),
        title: "Stream".into(),
        artist: "Band".into(),
        duration_secs: Some(60.25),
        thumbnail: None,
        source: TrackSourceDto {
            url: "https://sc.example/s".into(),
            system: "soundcloud".into(),
        },
        state: TrackStateDto::Streaming,
        liked: false,
    }
}

fn streaming_track_json() -> Value {
    json!({
        "id": 5,
        "title": "Stream",
        "artist": "Band",
        "durationSecs": 60.25,
        "thumbnail": null,
        "source": { "url": "https://sc.example/s", "system": "soundcloud" },
        "state": { "type": "streaming" },
        "liked": false
    })
}

fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

#[test]
fn track_dto_serializes_every_field_when_set() {
    assert_eq!(to_json(&full_track()), full_track_json());
}

#[test]
fn track_dto_serializes_none_as_null() {
    assert_eq!(to_json(&bare_track()), bare_track_json());
}

#[test]
fn track_dto_deserializes_with_default_fields_omitted() {
    let track: TrackDto = serde_json::from_value(json!({
        "id": 2,
        "title": "Minimal",
        "liked": true,
        "state": { "type": "streaming" }
    }))
    .unwrap();
    assert_eq!(track.id, 2);
    assert_eq!(track.title, "Minimal");
    assert_eq!(track.artist, None);
    assert_eq!(track.album, None);
    assert_eq!(track.duration_secs, None);
    assert_eq!(track.artwork_url, None);
    assert_eq!(track.play_count, 0);
    assert!(track.liked);
    assert!(matches!(track.state, TrackStateDto::Streaming));
    assert!(track.source.is_none());
    assert!(!track.unavailable);
}

#[test]
fn playlist_dto_serializes_every_field() {
    assert_eq!(to_json(&playlist()), playlist_json());
}

#[test]
fn playlist_dto_serializes_none_as_null() {
    let value = PlaylistDto {
        id: 12,
        title: "Empty".into(),
        description: None,
        creator: None,
        thumbnail: None,
        track_count: 0,
        created_at: 0,
        updated_at: 0,
        source: None,
    };
    assert_eq!(
        to_json(&value),
        json!({
            "id": 12,
            "title": "Empty",
            "description": null,
            "creator": null,
            "thumbnail": null,
            "trackCount": 0,
            "createdAt": 0,
            "updatedAt": 0,
            "source": null
        })
    );
}

#[test]
fn playlist_item_dto_track_variant() {
    let value = PlaylistItemDto::Track {
        id: 21,
        item_id: 22,
        track: full_track(),
        position: 3,
    };
    assert_eq!(
        to_json(&value),
        json!({
            "itemType": "track",
            "id": 21,
            "item_id": 22,
            "track": full_track_json(),
            "position": 3
        })
    );
}

#[test]
fn playlist_item_dto_playlist_variant() {
    let value = PlaylistItemDto::Playlist {
        id: 31,
        item_id: 32,
        playlist: playlist(),
        position: 0,
    };
    assert_eq!(
        to_json(&value),
        json!({
            "itemType": "playlist",
            "id": 31,
            "item_id": 32,
            "playlist": playlist_json(),
            "position": 0
        })
    );
}

fn container_meta() -> ContainerMetaDto {
    ContainerMetaDto {
        kind: "album".into(),
        id: "alb-1".into(),
        system: "bandcamp".into(),
        title: "Record".into(),
        artist: "Band".into(),
        track_count: 9,
        thumbnail: None,
    }
}

fn container_meta_json() -> Value {
    json!({
        "kind": "album",
        "id": "alb-1",
        "system": "bandcamp",
        "title": "Record",
        "artist": "Band",
        "trackCount": 9,
        "thumbnail": null
    })
}

#[test]
fn queue_entry_dto_track_variant() {
    let value = QueueEntryDto::Track {
        track: bare_track(),
    };
    assert_eq!(
        to_json(&value),
        json!({ "entryType": "track", "track": bare_track_json() })
    );
}

#[test]
fn queue_entry_dto_container_variant_with_tracks() {
    let value = QueueEntryDto::Container {
        meta: container_meta(),
        tracks: Some(vec![bare_track()]),
        play_index: 2,
    };
    assert_eq!(
        to_json(&value),
        json!({
            "entryType": "container",
            "meta": container_meta_json(),
            "tracks": [bare_track_json()],
            "play_index": 2
        })
    );
}

#[test]
fn queue_entry_dto_container_variant_without_tracks() {
    let value = QueueEntryDto::Container {
        meta: container_meta(),
        tracks: None,
        play_index: 0,
    };
    assert_eq!(
        to_json(&value),
        json!({
            "entryType": "container",
            "meta": container_meta_json(),
            "tracks": null,
            "play_index": 0
        })
    );
}

#[test]
fn queue_state_dto_serializes_every_field() {
    let value = QueueStateDto {
        entries: vec![QueueEntryDto::Track {
            track: bare_track(),
        }],
        current_position: Some(CurrentPositionDto {
            item_index: 1,
            track_within_container: Some(4),
        }),
        repeat_mode: "all".into(),
        shuffle_enabled: true,
        search_context: Some(SearchContextDto {
            system: "soundcloud".into(),
            query: "ambient".into(),
        }),
        radio_mode: Some("tracks".into()),
    };
    assert_eq!(
        to_json(&value),
        json!({
            "entries": [{ "entryType": "track", "track": bare_track_json() }],
            "currentPosition": { "itemIndex": 1, "trackWithinContainer": 4 },
            "repeatMode": "all",
            "shuffleEnabled": true,
            "searchContext": { "system": "soundcloud", "query": "ambient" },
            "radioMode": "tracks"
        })
    );
}

#[test]
fn queue_state_dto_serializes_none_as_null() {
    let value = QueueStateDto {
        entries: vec![],
        current_position: Some(CurrentPositionDto {
            item_index: 0,
            track_within_container: None,
        }),
        repeat_mode: "off".into(),
        shuffle_enabled: false,
        search_context: None,
        radio_mode: None,
    };
    assert_eq!(
        to_json(&value),
        json!({
            "entries": [],
            "currentPosition": { "itemIndex": 0, "trackWithinContainer": null },
            "repeatMode": "off",
            "shuffleEnabled": false,
            "searchContext": null,
            "radioMode": null
        })
    );
}

#[test]
fn audio_state_dto_serializes_every_field() {
    let value = AudioStateDto {
        state: "playing".into(),
        position_secs: 12.5,
        duration_secs: 240.0,
        volume: 0.75,
        current_track: Some(bare_track()),
        buffered_ranges: vec![BufferedRangeDto {
            start: 0.0,
            end: 0.5,
        }],
        waveforms: vec![WaveformDto {
            data: vec![0.25, 1.0],
            start_time: 0.0,
            end_time: 10.5,
            byte_start: Some(0),
            byte_end: None,
        }],
        fft: vec![0.125, 0.5],
    };
    assert_eq!(
        to_json(&value),
        json!({
            "state": "playing",
            "positionSecs": 12.5,
            "durationSecs": 240.0,
            "volume": 0.75,
            "currentTrack": bare_track_json(),
            "bufferedRanges": [{ "start": 0.0, "end": 0.5 }],
            "waveforms": [{
                "data": [0.25, 1.0],
                "startTime": 0.0,
                "endTime": 10.5,
                "byteStart": 0,
                "byteEnd": null
            }],
            "fft": [0.125, 0.5]
        })
    );
}

#[test]
fn audio_state_dto_without_track() {
    let value = AudioStateDto {
        state: "stopped".into(),
        position_secs: 0.0,
        duration_secs: 0.0,
        volume: 1.0,
        current_track: None,
        buffered_ranges: vec![],
        waveforms: vec![],
        fft: vec![],
    };
    assert_eq!(
        to_json(&value),
        json!({
            "state": "stopped",
            "positionSecs": 0.0,
            "durationSecs": 0.0,
            "volume": 1.0,
            "currentTrack": null,
            "bufferedRanges": [],
            "waveforms": [],
            "fft": []
        })
    );
}

#[test]
fn search_result_dto_track_variant() {
    let value = SearchResultDto::Track(streaming_track());
    let mut expected = streaming_track_json();
    expected["type"] = json!("track");
    assert_eq!(to_json(&value), expected);
}

#[test]
fn search_result_dto_playlist_variant() {
    let value = SearchResultDto::Playlist(StreamingPlaylistDto {
        id: "pl-1".into(),
        title: "Set".into(),
        creator: "DJ".into(),
        track_count: 1,
        thumbnail: Some("t.png".into()),
        tracks: vec![streaming_track()],
        url: "https://sc.example/sets/1".into(),
        system: "soundcloud".into(),
    });
    assert_eq!(
        to_json(&value),
        json!({
            "type": "playlist",
            "id": "pl-1",
            "title": "Set",
            "creator": "DJ",
            "trackCount": 1,
            "thumbnail": "t.png",
            "tracks": [streaming_track_json()],
            "url": "https://sc.example/sets/1",
            "system": "soundcloud"
        })
    );
}

#[test]
fn search_result_dto_album_variant() {
    let value = SearchResultDto::Album(StreamingAlbumDto {
        id: "al-1".into(),
        title: "LP".into(),
        artist: "Band".into(),
        track_count: 0,
        year: Some(2001),
        thumbnail: None,
        tracks: vec![],
        url: "https://band.example/album/lp".into(),
        system: "bandcamp".into(),
    });
    assert_eq!(
        to_json(&value),
        json!({
            "type": "album",
            "id": "al-1",
            "title": "LP",
            "artist": "Band",
            "trackCount": 0,
            "year": 2001,
            "thumbnail": null,
            "tracks": [],
            "url": "https://band.example/album/lp",
            "system": "bandcamp"
        })
    );
}

#[test]
fn search_result_dto_channel_variant() {
    let value = SearchResultDto::Channel(StreamingChannelDto {
        id: "ch-1".into(),
        name: "Label".into(),
        subscriber_count: Some(42),
        thumbnail: None,
        url: "https://sc.example/label".into(),
        system: "soundcloud".into(),
    });
    assert_eq!(
        to_json(&value),
        json!({
            "type": "channel",
            "id": "ch-1",
            "name": "Label",
            "subscriberCount": 42,
            "thumbnail": null,
            "url": "https://sc.example/label",
            "system": "soundcloud"
        })
    );
}

#[test]
fn track_state_dto_variants() {
    assert_eq!(to_json(&TrackStateDto::Saved), json!({ "type": "saved" }));
    assert_eq!(
        to_json(&TrackStateDto::Streaming),
        json!({ "type": "streaming" })
    );
    assert_eq!(to_json(&TrackStateDto::Cached), json!({ "type": "cached" }));
}

#[test]
fn task_info_dto_with_meta() {
    let value = TaskInfoDto {
        track_id: 9,
        url: "https://sc.example/s".into(),
        state: "Complete".into(),
        size_human: "1.5 MB".into(),
        byte_size: 1536,
        progress: 0.5,
        meta: Some(streaming_track()),
    };
    assert_eq!(
        to_json(&value),
        json!({
            "trackId": 9,
            "url": "https://sc.example/s",
            "state": "Complete",
            "sizeHuman": "1.5 MB",
            "byteSize": 1536,
            "progress": 0.5,
            "meta": streaming_track_json()
        })
    );
}

#[test]
fn task_info_dto_without_meta() {
    let value = TaskInfoDto {
        track_id: 0,
        url: String::new(),
        state: "Cancelled".into(),
        size_human: "0 B".into(),
        byte_size: 0,
        progress: 0.0,
        meta: None,
    };
    assert_eq!(
        to_json(&value),
        json!({
            "trackId": 0,
            "url": "",
            "state": "Cancelled",
            "sizeHuman": "0 B",
            "byteSize": 0,
            "progress": 0.0,
            "meta": null
        })
    );
}

#[test]
fn eq_settings_dto_with_preset() {
    let value = EqSettingsDto {
        enabled: true,
        gains: vec![-1.5, 0.0, 2.25],
        preset_name: Some("Rock".into()),
    };
    assert_eq!(
        to_json(&value),
        json!({ "enabled": true, "gains": [-1.5, 0.0, 2.25], "presetName": "Rock" })
    );
}

#[test]
fn eq_settings_dto_without_preset() {
    let value = EqSettingsDto {
        enabled: false,
        gains: vec![],
        preset_name: None,
    };
    assert_eq!(
        to_json(&value),
        json!({ "enabled": false, "gains": [], "presetName": null })
    );
}

#[test]
fn eq_preset_dto_serializes() {
    let value = EqPresetDto {
        name: "Flat".into(),
        gains: vec![0.0, 0.5],
    };
    assert_eq!(
        to_json(&value),
        json!({ "name": "Flat", "gains": [0.0, 0.5] })
    );
}

#[test]
fn create_playlist_request_deserializes_every_field() {
    let request: CreatePlaylistRequest = serde_json::from_value(json!({
        "title": "New",
        "description": "About",
        "creator": "Me",
        "artworkUrl": "cover.png"
    }))
    .unwrap();
    assert_eq!(request.title, "New");
    assert_eq!(request.description.as_deref(), Some("About"));
    assert_eq!(request.creator.as_deref(), Some("Me"));
    assert_eq!(request.artwork_url.as_deref(), Some("cover.png"));
}

#[test]
fn create_playlist_request_deserializes_with_options_omitted() {
    let request: CreatePlaylistRequest =
        serde_json::from_value(json!({ "title": "Only title" })).unwrap();
    assert_eq!(request.title, "Only title");
    assert_eq!(request.description, None);
    assert_eq!(request.creator, None);
    assert_eq!(request.artwork_url, None);
}

#[test]
fn create_playlist_request_ignores_snake_case_keys() {
    let request: CreatePlaylistRequest =
        serde_json::from_value(json!({ "title": "T", "artwork_url": "x.png" })).unwrap();
    assert_eq!(request.artwork_url, None);
}

#[test]
fn create_playlist_with_tracks_request_deserializes_every_field() {
    let request: CreatePlaylistWithTracksRequest = serde_json::from_value(json!({
        "title": "From search",
        "description": "Picked",
        "artworkUrl": "art.png",
        "tracks": [full_track_json()]
    }))
    .unwrap();
    assert_eq!(request.title, "From search");
    assert_eq!(request.description.as_deref(), Some("Picked"));
    assert_eq!(request.artwork_url.as_deref(), Some("art.png"));
    assert_eq!(request.tracks.len(), 1);
    assert_eq!(to_json(&request.tracks[0]), full_track_json());
}

#[test]
fn create_playlist_with_tracks_request_deserializes_with_options_omitted() {
    let request: CreatePlaylistWithTracksRequest = serde_json::from_value(json!({
        "title": "Bare",
        "tracks": [{
            "id": 1,
            "title": "Bare",
            "liked": false,
            "state": { "type": "saved" }
        }]
    }))
    .unwrap();
    assert_eq!(request.title, "Bare");
    assert_eq!(request.description, None);
    assert_eq!(request.artwork_url, None);
    assert_eq!(request.tracks.len(), 1);
    assert_eq!(to_json(&request.tracks[0]), bare_track_json());
}
