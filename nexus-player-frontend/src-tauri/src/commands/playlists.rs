use super::Result;
use crate::dto::{
    CreatePlaylistRequest, CreatePlaylistWithTracksRequest, PlaylistDto, PlaylistItemDto,
    PlaylistWithItemsDto, TrackDto,
};
use nexus_player_backend::library::types::{PlaylistId, TrackId};
use nexus_player_backend::state::state::AppState;
use nexus_player_backend::streaming::types::ContentType;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_all_playlists(state: State<'_, Arc<AppState>>) -> Result<Vec<PlaylistDto>> {
    let playlists = state.get_all_playlists().await.map_err(|e| e.to_string())?;

    Ok(playlists.into_iter().map(|p| p.into()).collect())
}

#[tauri::command]
pub async fn get_playlist_with_tracks(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
) -> Result<Option<PlaylistWithItemsDto>> {
    let pwi = state
        .get_playlist_with_items(PlaylistId(playlist_id))
        .await
        .map_err(|e| e.to_string())?;

    let Some(pwi) = pwi else {
        return Ok(None);
    };

    let mut items = Vec::new();
    for item in &pwi.items {
        match item.item_kind {
            nexus_player_backend::library::models::PlaylistItemKind::Track => {
                if let Some(track) = state
                    .get_library_track(TrackId(item.item_id))
                    .await
                    .map_err(|e| e.to_string())?
                {
                    items.push(PlaylistItemDto::Track {
                        id: item.id,
                        item_id: item.item_id,
                        track: track.into(),
                        position: item.position,
                    });
                }
            }
            nexus_player_backend::library::models::PlaylistItemKind::Playlist => {
                if let Some(playlist) = state
                    .get_playlist(PlaylistId(item.item_id))
                    .await
                    .map_err(|e| e.to_string())?
                {
                    items.push(PlaylistItemDto::Playlist {
                        id: item.id,
                        item_id: item.item_id,
                        playlist: playlist.into(),
                        position: item.position,
                    });
                }
            }
        }
    }

    let mut playlist_dto: PlaylistDto = pwi.playlist.into();
    playlist_dto.track_count = items.len();

    Ok(Some(PlaylistWithItemsDto {
        playlist: playlist_dto,
        items,
    }))
}

#[tauri::command]
pub async fn add_playlist_to_playlist(
    state: State<'_, Arc<AppState>>,
    parent_playlist_id: i64,
    child_playlist_id: i64,
) -> Result<()> {
    state
        .add_playlist_to_playlist(
            PlaylistId(parent_playlist_id),
            PlaylistId(child_playlist_id),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn remove_playlist_from_playlist(
    state: State<'_, Arc<AppState>>,
    parent_playlist_id: i64,
    child_playlist_id: i64,
) -> Result<()> {
    state
        .remove_playlist_from_playlist(
            PlaylistId(parent_playlist_id),
            PlaylistId(child_playlist_id),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn create_playlist(
    state: State<'_, Arc<AppState>>,
    request: CreatePlaylistRequest,
) -> Result<i64> {
    let playlist_id = state
        .create_playlist(
            request.title,
            request.description,
            request.creator,
            request.artwork_url,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(playlist_id.0)
}

#[tauri::command]
pub async fn delete_playlist(state: State<'_, Arc<AppState>>, playlist_id: i64) -> Result<()> {
    state
        .delete_playlist(PlaylistId(playlist_id))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// The parameter list is the invoke contract with the frontend, so it cannot be grouped.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn create_playlist_from_container(
    state: State<'_, Arc<AppState>>,
    provider: String,
    content_type: ContentType,
    id: String,
    title: String,
    description: Option<String>,
    _creator: Option<String>,
    artwork_url: Option<String>,
) -> Result<i64> {
    state
        .create_playlist_from_container(provider, content_type, id, title, description, artwork_url)
        .await
        .map(|id| id.0)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_playlist_with_tracks(
    state: State<'_, Arc<AppState>>,
    request: CreatePlaylistWithTracksRequest,
) -> Result<i64> {
    let audio_tracks: Vec<_> = request
        .tracks
        .iter()
        .map(|t| t.try_to_audio_track())
        .collect::<std::result::Result<_, _>>()?;

    let playlist_id = state
        .create_playlist_with_tracks(
            request.title,
            request.description,
            request.artwork_url,
            audio_tracks,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(playlist_id.0)
}

#[tauri::command]
pub async fn add_track_to_playlist(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
    track: TrackDto,
) -> Result<()> {
    state
        .add_track_to_playlist(PlaylistId(playlist_id), track.try_to_audio_track()?)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn remove_track_from_playlist(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
    track_id: i64,
) -> Result<()> {
    state
        .remove_track_from_playlist(PlaylistId(playlist_id), TrackId(track_id))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
