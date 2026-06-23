use super::Result;
use crate::dto::SearchResultDto;
use nexus_player_backend::playlist::cursor::ContainerContext;
use nexus_player_backend::state::state::AppState;
use nexus_player_backend::streaming::types::{ContentType, SearchResult};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn search_streaming(
    state: State<'_, Arc<AppState>>,
    provider: String,
    content_type: Option<String>,
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<SearchResultDto>> {
    let content_type_enum = content_type.and_then(|ct| match ct.as_str() {
        "track" => Some(ContentType::Track),
        "playlist" => Some(ContentType::Playlist),
        "album" => Some(ContentType::Album),
        "channel" => Some(ContentType::Channel),
        _ => None,
    });

    let results = state
        .search_streaming_cancellable(
            &provider,
            content_type_enum,
            &query,
            limit.unwrap_or(10),
            offset.unwrap_or(0),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut dtos: Vec<SearchResultDto> = Vec::new();
    let mut tracks_to_enrich: Vec<nexus_player_backend::audio::types::Track> = Vec::new();
    let mut track_indices = Vec::new();

    for (i, r) in results.into_iter().enumerate() {
        let dto = SearchResultDto::from(r);
        if let SearchResultDto::Track(ref t) = dto {
            tracks_to_enrich.push(t.to_audio_track());
            track_indices.push(i);
        }
        dtos.push(dto);
    }

    if !tracks_to_enrich.is_empty() {
        let enriched = state
            .enrich_tracks(tracks_to_enrich)
            .await
            .map_err(|e| e.to_string())?;
        for (i, track) in track_indices.into_iter().zip(enriched) {
            if let SearchResultDto::Track(ref mut t) = dtos[i] {
                t.liked = track.liked;
            }
        }
    }

    Ok(dtos)
}

#[tauri::command]
pub async fn handle_streaming_track(
    state: State<'_, Arc<AppState>>,
    search_result: SearchResultDto,
    strategy: String,
    query: Option<String>,
    search_type: Option<String>,
    play: Option<bool>,
) -> Result<Option<i64>> {
    use nexus_player_backend::library::types::DownloadStrategy;
    use nexus_player_backend::streaming::types::ContentType;

    let download_strategy = match strategy.as_str() {
        "stream" => DownloadStrategy::Stream,
        "cache" => DownloadStrategy::Cache,
        "progressive" => DownloadStrategy::Progressive,
        _ => return Err("Invalid download strategy".into()),
    };
    let should_play = play.unwrap_or(false);
    tracing::info!("SEARCH RESULT {:?}", search_result);

    match download_strategy {
        DownloadStrategy::Cache => {
            let backend_search_result: nexus_player_backend::streaming::types::SearchResult =
                search_result.into();

            match backend_search_result {
                SearchResult::Track(track) => {
                    if let Some(q) = query {
                        let audio_track: nexus_player_backend::audio::types::Track = track.into();
                        let target_id = audio_track.id;
                        let system = audio_track.source.system().to_string();

                        let content_type = match search_type.as_deref() {
                            Some("track") => ContentType::Track,
                            Some("playlist") => ContentType::Playlist,
                            Some("album") => ContentType::Album,
                            Some("channel") => ContentType::Channel,
                            _ => ContentType::Track,
                        };

                        if should_play {
                            state
                                .load_search_and_play(system, q, content_type, target_id)
                                .await
                                .map_err(|e| e.to_string())?;
                        } else {
                            state
                                .load_search_to_queue(system, q, content_type, target_id)
                                .await
                                .map_err(|e| e.to_string())?;
                        }
                    } else {
                        let audio_track: nexus_player_backend::audio::types::Track = track.into();
                        if should_play {
                            state
                                .add_track_and_play(audio_track)
                                .await
                                .map_err(|e| e.to_string())?;
                        } else {
                            use nexus_player_backend::playlist::playback_queue::{
                                ActiveQueue, CursorQueue,
                            };
                            let mut queue = state.lock_queue().await;
                            match &mut *queue {
                                ActiveQueue::Cursor(cq) => {
                                    cq.add_track(audio_track);
                                }
                                _ => {
                                    let mut cq = CursorQueue::new();
                                    cq.add_track(audio_track);
                                    *queue = ActiveQueue::Cursor(Box::new(cq));
                                }
                            }
                            tracing::info!("Track added to queue");
                        }
                    }
                }
                SearchResult::Playlist(playlist) => {
                    let context = ContainerContext::ExternalPlaylist {
                        system: playlist.service.to_string(),
                        id: playlist.id,
                    };
                    if should_play {
                        state
                            .load_context_and_play(context, 0)
                            .await
                            .map_err(|e| e.to_string())?;
                    } else {
                        state
                            .load_context_to_queue(context)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                }
                SearchResult::Album(album) => {
                    let context = ContainerContext::ExternalAlbum {
                        system: album.service.to_string(),
                        id: album.id,
                    };
                    if should_play {
                        state
                            .load_context_and_play(context, 0)
                            .await
                            .map_err(|e| e.to_string())?;
                    } else {
                        state
                            .load_context_to_queue(context)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                }
                SearchResult::Channel(_) => {
                    return Err("Channels cannot be added to the queue".into());
                }
            }
            Ok(None)
        }
        DownloadStrategy::Stream => Err("Stream strategy not yet implemented".into()),
        DownloadStrategy::Progressive => Err("Progressive strategy not yet implemented".into()),
    }
}
