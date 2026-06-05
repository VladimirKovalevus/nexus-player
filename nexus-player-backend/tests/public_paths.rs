// T2 R3: compile-only. This crate sits outside the library, so a path that stops being public
// fails to build. It holds no test, which keeps the set of test names unchanged.

type _AppState = nexus_player_backend::state::state::AppState;

type _ActiveQueue = nexus_player_backend::playlist::playback_queue::ActiveQueue;
type _CursorQueue = nexus_player_backend::playlist::playback_queue::CursorQueue;
type _SearchQueue = nexus_player_backend::playlist::playback_queue::SearchQueue;
type _QueueEntry = nexus_player_backend::playlist::playback_queue::QueueEntry;
type _RepeatMode = nexus_player_backend::playlist::playback_queue::RepeatMode;
type _ContainerKind = nexus_player_backend::playlist::playback_queue::ContainerKind;
type _ContainerMeta = nexus_player_backend::playlist::playback_queue::ContainerMeta;
type _SearchQueueItem = nexus_player_backend::playlist::playback_queue::SearchQueueItem;
type _NextResult = nexus_player_backend::playlist::playback_queue::NextResult;
type _CurrentPosition = nexus_player_backend::playlist::playback_queue::CurrentPosition;

type _HlsReaderAdapter = nexus_player_backend::audio::playback::HlsReaderAdapter;
type _PlaybackEngine = nexus_player_backend::audio::playback::PlaybackEngine;
