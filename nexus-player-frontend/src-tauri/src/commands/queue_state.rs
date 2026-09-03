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

fn entry_dto(entry: nexus_player_backend::playlist::playback_queue::QueueEntry) -> QueueEntryDto {
    use nexus_player_backend::playlist::playback_queue::{ContainerKind, QueueEntry};

    match entry {
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
    }
}

fn repeat_mode_label(mode: nexus_player_backend::playlist::playback_queue::RepeatMode) -> String {
    use nexus_player_backend::playlist::playback_queue::RepeatMode;
    match mode {
        RepeatMode::Off => "off",
        RepeatMode::All => "all",
        RepeatMode::One => "one",
    }
    .to_string()
}

/// The one place that turns the active queue into the DTO the UI shows.
pub(crate) fn build_queue_state_dto(
    queue: &nexus_player_backend::playlist::playback_queue::ActiveQueue,
) -> Result<QueueStateDto> {
    use nexus_player_backend::playlist::playback_queue::ActiveQueue;

    let ActiveQueue::Cursor(cq) = queue;
    let (rows, current_index) = cq.snapshot();
    let search_context = match &cq.context {
        Some(ContainerContext::SearchQuery { system, query, .. }) => Some(SearchContextDto {
            system: system.clone(),
            query: query.clone(),
        }),
        _ => None,
    };

    Ok(QueueStateDto {
        entries: rows.into_iter().map(entry_dto).collect(),
        current_position: current_index.map(|idx| CurrentPositionDto {
            item_index: idx,
            track_within_container: None,
        }),
        repeat_mode: repeat_mode_label(cq.repeat_mode),
        shuffle_enabled: cq.shuffle_enabled,
        search_context,
        radio_mode: radio_mode_label(cq),
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn handle_search(
    state: State<'_, Arc<AppState>>,
    system: String,
    content_type: Option<String>,
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
    selected_index: usize,
    result_id: Option<String>,
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
            result_id,
        )
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn expand_queue_container(
    state: State<'_, Arc<AppState>>,
    item_index: usize,
) -> Result<QueueStateDto> {
    state.expand_queue_container(item_index).await?;

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
