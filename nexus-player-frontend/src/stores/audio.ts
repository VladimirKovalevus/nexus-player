import { defineStore } from 'pinia';
import { ref, computed, watch } from 'vue';
import type { AudioState, QueueState, AudioEvent } from '@/types/types';
import { invoke } from '@tauri-apps/api/core';
import { useNotificationStore } from './notifications';

export const useAudioStore = defineStore('audio', () => {
  const notificationStore = useNotificationStore();
  const audioState = ref<AudioState>({
    state: 'stopped',
    positionSecs: 0,
    durationSecs: 0,
    volume: 1.0,
    currentTrack: null,
    bufferedRanges: [],
    waveforms: [],
  });

  const queueState = ref<QueueState>({
    entries: [],
    currentPosition: null,
    repeatMode: 'off',
    shuffleEnabled: false,
    searchContext: null,
  });

  // Media Session Integration
  function updateMediaSessionMetadata() {
    if ('mediaSession' in navigator && audioState.value.currentTrack) {
      const track = audioState.value.currentTrack;
      navigator.mediaSession.metadata = new MediaMetadata({
        title: track.title,
        artist: track.artist,
        album: track.album || '',
        artwork: track.thumbnail ? [{ src: track.thumbnail, sizes: '512x512', type: 'image/png' }] : []
      });
    }
  }

  function updateMediaSessionPlaybackState() {
    if ('mediaSession' in navigator) {
      navigator.mediaSession.playbackState = 
        audioState.value.state === 'playing' ? 'playing' : 
        audioState.value.state === 'paused' ? 'paused' : 'none';
      
      // Update position state for seek bar in notification
      if (audioState.value.durationSecs > 0) {
        navigator.mediaSession.setPositionState({
          duration: audioState.value.durationSecs,
          playbackRate: 1,
          position: audioState.value.positionSecs
        });
      }
    }
  }

  // Setup Media Session action handlers once
  if ('mediaSession' in navigator) {
    navigator.mediaSession.setActionHandler('play', () => invoke('resume'));
    navigator.mediaSession.setActionHandler('pause', () => invoke('pause'));
    navigator.mediaSession.setActionHandler('previoustrack', () => invoke('play_previous'));
    navigator.mediaSession.setActionHandler('nexttrack', () => invoke('play_next'));
    navigator.mediaSession.setActionHandler('seekto', (details) => {
      if (details.seekTime !== undefined) {
        invoke('seek', { positionSecs: details.seekTime });
      }
    });
  }

  // Android native bridge: invoked by notification media buttons
  if (typeof window !== 'undefined') {
    (window as any).__nexusControl = (command: string) => invoke(command);
  }

  function syncNativeBridge() {
    const bridge = typeof window !== 'undefined' ? (window as any)._NexusAudio : null;
    if (!bridge) return;
    const track = audioState.value.currentTrack;
    const playing = audioState.value.state === 'playing' || audioState.value.state === 'loading';
    if (audioState.value.state === 'stopped') {
      bridge.stopService();
    } else {
      bridge.updateState(track?.title ?? 'Nexus Player', track?.artist ?? '', playing);
    }
  }

  const isPlaying = computed(() => audioState.value.state === 'playing');
  const isPaused = computed(() => audioState.value.state === 'paused');
  const isStopped = computed(() => audioState.value.state === 'stopped');
  const isLoading = computed(() => audioState.value.state === 'loading');
  const currentTrack = computed(() => audioState.value.currentTrack);
  const position = computed(() => audioState.value.positionSecs);
  const duration = computed(() => audioState.value.durationSecs);
  const bufferedRanges = computed(() => audioState.value.bufferedRanges);
  const waveforms = computed(() => audioState.value.waveforms);
  const volume = computed(() => audioState.value.volume);
  const repeatMode = computed(() => queueState.value.repeatMode);
  const shuffleEnabled = computed(() => queueState.value.shuffleEnabled);
  const entries = computed(() => queueState.value.entries);
  const currentPosition = computed(() => queueState.value.currentPosition);
  const isSearchQueue = computed(() => queueState.value.searchContext !== null);
  const searchContext = computed(() => queueState.value.searchContext);

  // Compatibility: flat list of tracks from entries
  const tracks = computed(() => {
    const result: import('@/types/types').TrackDto[] = [];
    for (const entry of queueState.value.entries) {
      if (entry.entryType === 'track') {
        result.push(entry.track);
      } else if (entry.entryType === 'container' && entry.tracks) {
        result.push(...entry.tracks);
      }
    }
    return result;
  });
  const currentIndex = computed(() => queueState.value.currentPosition?.itemIndex ?? null);

  const repeatIcon = computed((): string => {
    switch (queueState.value.repeatMode) {
      case 'off':
        return '🔁';
      case 'all':
        return '🔁';
      case 'one':
        return '🔂';
      default:
        return '🔁';
    }
  });

  function handleAudioEvent(event: AudioEvent) {
    switch (event.type) {
      case 'PlaybackStarted':
        audioState.value.state = 'playing';
        updateMediaSessionPlaybackState();
        syncNativeBridge();
        break;
      case 'PlaybackPaused':
        audioState.value.state = 'paused';
        updateMediaSessionPlaybackState();
        syncNativeBridge();
        break;
      case 'PlaybackResumed':
        audioState.value.state = 'playing';
        updateMediaSessionPlaybackState();
        syncNativeBridge();
        break;
      case 'PlaybackStopped':
        audioState.value.state = 'stopped';
        audioState.value.currentTrack = null;
        updateMediaSessionPlaybackState();
        syncNativeBridge();
        break;
      case 'PositionUpdated':
        if (event.payload?.positionSecs !== undefined) {
          audioState.value.positionSecs = event.payload.positionSecs;
          // Don't update MediaSession on every position tick to avoid overhead, 
          // but maybe every few seconds or when explicitly asked.
          // Mobile systems usually interpolate position themselves.
        }
        break;
      case 'DurationChanged':
        if (event.payload?.durationSecs !== undefined) {
          audioState.value.durationSecs = event.payload.durationSecs;
          updateMediaSessionPlaybackState();
        }
        break;
      case 'VolumeChanged':
        if (event.payload?.volume !== undefined) {
          audioState.value.volume = event.payload.volume;
        }
        break;
      case 'BufferUpdated':
        if (event.payload?.ranges !== undefined) {
          audioState.value.bufferedRanges = event.payload.ranges;
        }
        break;
      case 'WaveformUpdated':
        if (event.payload?.waveforms !== undefined) {
          audioState.value.waveforms = event.payload.waveforms;
        }
        break;
      case 'LoadingStarted':
        audioState.value.state = 'loading';
        audioState.value.positionSecs = 0;
        audioState.value.bufferedRanges = [];
        audioState.value.waveforms = [];
        if (event.payload?.track) {
          audioState.value.currentTrack = event.payload.track;
          updateMediaSessionMetadata();
        }
        updateMediaSessionPlaybackState();
        syncNativeBridge();
        break;
      case 'AudioStateUpdated':
        if (event.payload?.state) {
          audioState.value = event.payload.state;
          updateMediaSessionMetadata();
          updateMediaSessionPlaybackState();
          syncNativeBridge();
        }
        if (event.payload?.queue) {
          queueState.value = event.payload.queue;
        }
        break;
      case 'Error':
        console.error('Audio error:', event.payload?.message);
        notificationStore.addNotification(`Audio Error: ${event.payload?.message}`, 'error');
        break;
    }
  }

  function updateAudioState(newState: AudioState) {
    audioState.value = newState;
  }

  function updateQueueState(newState: QueueState) {
    queueState.value = newState;
  }

  function setPosition(positionSecs: number) {
    audioState.value.positionSecs = positionSecs;
  }

  return {
    audioState,
    queueState,
    isPlaying,
    isPaused,
    isStopped,
    isLoading,
    currentTrack,
    position,
    duration,
    bufferedRanges,
    waveforms,
    volume,
    repeatMode,
    shuffleEnabled,
    entries,
    currentPosition,
    isSearchQueue,
    searchContext,
    tracks,
    currentIndex,
    repeatIcon,
    handleAudioEvent,
    updateAudioState,
    updateQueueState,
    setPosition,
  };
});
