use super::Result;
use crate::dto::{
    ContainerMetaDto, CurrentPositionDto, QueueEntryDto, QueueStateDto, SearchContextDto,
};
use nexus_player_backend::playlist::cursor::ContainerContext;
use nexus_player_backend::state::state::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_queue_state(state: State<'_, Arc<AppState>>) -> Result<QueueStateDto> {
    let queue = state.lock_queue().await;
    build_queue_state_dto(&queue)
}

/// Reads the active radio mode out of the queue context. Called on the 100ms event loop
/// too, so it only borrows — never clones the context (which carries the whole order).
pub(crate) fn radio_mode_label(
    cq: &nexus_player_backend::playlist::playback_queue::CursorQueue,
) -> Option<String> {
    match &cq.context {
        Some(ContainerContext::Radio { mode, .. }) => Some(
            match mode {
                nexus_player_backend::playlist::cursor::RadioMode::Tracks => "tracks",
                nexus_player_backend::playlist::cursor::RadioMode::Playlists => "playlists",
            }
            .to_string(),
        ),
        _ => None,
    }
}

fn build_queue_state_dto(
    queue: &nexus_player_backend::playlist::playback_queue::ActiveQueue,
) -> Result<QueueStateDto> {
    use nexus_player_backend::playlist::playback_queue::{ActiveQueue, ContainerKind, QueueEntry};

    match queue {
        ActiveQueue::Cursor(cq) => {
            let (tracks, current_index) = cq.snapshot();
            let entries: Vec<QueueEntryDto> = tracks
                .into_iter()
                .map(|t| QueueEntryDto::Track { track: t.into() })
                .collect();

            let current_position = current_index.map(|idx| CurrentPositionDto {
                item_index: idx,
                track_within_container: None,
            });

            let repeat_mode = match cq.repeat_mode {
                nexus_player_backend::playlist::playback_queue::RepeatMode::Off => "off",
                nexus_player_backend::playlist::playback_queue::RepeatMode::All => "all",
                nexus_player_backend::playlist::playback_queue::RepeatMode::One => "one",
            };

            Ok(QueueStateDto {
                entries,
                current_position,
                repeat_mode: repeat_mode.to_string(),
                shuffle_enabled: cq.shuffle_enabled,
                search_context: None,
                radio_mode: radio_mode_label(cq),
            })
        }
        ActiveQueue::Search(sq) => {
            let (entries, position) = sq.snapshot();

            let entries_dto: Vec<QueueEntryDto> = entries
                .into_iter()
                .map(|entry| match entry {
                    QueueEntry::Track(track) => QueueEntryDto::Track {
                        track: track.into(),
                    },
                    QueueEntry::Container {
                        info,
                        tracks,
                        play_index,
                    } => {
                        let kind_str = match info.kind {
                            ContainerKind::Album => "album",
                            ContainerKind::Playlist => "playlist",
                        };
                        QueueEntryDto::Container {
                            meta: ContainerMetaDto {
                                kind: kind_str.to_string(),
                                id: info.id,
                                system: info.system,
                                title: info.title,
                                artist: info.artist,
                                track_count: info.track_count,
                                thumbnail: info.thumbnail,
                            },
                            tracks: tracks.map(|ts| ts.into_iter().map(|t| t.into()).collect()),
                            play_index,
                        }
                    }
                })
                .collect();

            let current_position = position.map(|p| CurrentPositionDto {
                item_index: p.item_index,
                track_within_container: p.track_within_container,
            });

            let repeat_mode = match sq.repeat_mode {
                nexus_player_backend::playlist::playback_queue::RepeatMode::Off => "off",
                nexus_player_backend::playlist::playback_queue::RepeatMode::All => "all",
                nexus_player_backend::playlist::playback_queue::RepeatMode::One => "one",
            };

            Ok(QueueStateDto {
                entries: entries_dto,
                current_position,
                repeat_mode: repeat_mode.to_string(),
                shuffle_enabled: sq.shuffle_enabled,
                search_context: Some(SearchContextDto {
                    system: sq.system.clone(),
                    query: sq.query.clone(),
                }),
                radio_mode: None,
            })
        }
    }
}

#[tauri::command]
pub async fn handle_search(
    state: State<'_, Arc<AppState>>,
    system: String,
    content_type: Option<String>,
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
    selected_index: usize,
) -> Result<()> {
    use nexus_player_backend::streaming::types::ContentType;

    let content_type_enum = content_type.and_then(|ct| match ct.as_str() {
        "track" => Some(ContentType::Track),
        "playlist" => Some(ContentType::Playlist),
        "album" => Some(ContentType::Album),
        "channel" => Some(ContentType::Channel),
        _ => None,
    });

    state
        .search_and_play(
            system,
            query,
            content_type_enum,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
            selected_index,
        )
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn expand_queue_container(
    state: State<'_, Arc<AppState>>,
    item_index: usize,
) -> Result<QueueStateDto> {
    state.expand_search_container(item_index).await?;

    let queue = state.lock_queue().await;
    build_queue_state_dto(&queue)
}

#[tauri::command]
pub async fn play_queue_item(
    state: State<'_, Arc<AppState>>,
    item_index: usize,
    track_index_in_container: Option<usize>,
) -> Result<()> {
    state
        .play_queue_item(item_index, track_index_in_container)
        .await;
    Ok(())
}
