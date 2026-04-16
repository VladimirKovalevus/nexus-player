import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { AudioState, QueueState, AudioEvent } from '@/types/types';
import { invoke } from '@tauri-apps/api/core';
import { useNotificationStore } from './notifications';

export const useAudioStore = defineStore('audio', () => {
  const notificationStore = useNotificationStore();

  const lastUnavailableTrackId = ref<number | null>(null);

  // High-frequency state isolated to prevent unnecessary re-renders of the whole app
  const currentPositionSecs = ref(0);
  const currentFft = ref<number[]>([]);
  const currentPcm = ref<number[]>([]);
  let _lastPosUpdate = 0;
  const isWaitingForAudio = ref(false);

  const audioState = ref<AudioState>({
    state: 'stopped',
    positionSecs: 0,
    durationSecs: 0,
    volume: 0.1,
    currentTrack: null,
    bufferedRanges: [],
    waveforms: [],
    fft: [],
    pcm: [],
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
        artwork: track.thumbnail
          ? [{ src: track.thumbnail, sizes: '512x512', type: 'image/png' }]
          : [],
      });
    }
  }

  function updateMediaSessionPlaybackState() {
    if ('mediaSession' in navigator) {
      navigator.mediaSession.playbackState =
        audioState.value.state === 'playing'
          ? 'playing'
          : audioState.value.state === 'paused'
            ? 'paused'
            : 'none';

      if (audioState.value.durationSecs > 0) {
        navigator.mediaSession.setPositionState({
          duration: audioState.value.durationSecs,
          playbackRate: 1,
          position: currentPositionSecs.value,
        });
      }
    }
  }

  if ('mediaSession' in navigator) {
    navigator.mediaSession.setActionHandler('play', () => invoke('resume'));
    navigator.mediaSession.setActionHandler('pause', () => invoke('pause'));
    navigator.mediaSession.setActionHandler('previoustrack', () =>
      invoke('play_previous'),
    );
    navigator.mediaSession.setActionHandler('nexttrack', () =>
      invoke('play_next'),
    );
    navigator.mediaSession.setActionHandler('seekto', (details) => {
      if (details.seekTime !== undefined) {
        invoke('seek', { positionSecs: Math.max(details.seekTime, 0) });
      }
    });
  }

  if (typeof window !== 'undefined') {
    (window as any).__nexusControl = (command: string) => invoke(command);
  }

  function syncNativeBridge() {
    const bridge =
      typeof window !== 'undefined' ? (window as any)._NexusAudio : null;
    if (!bridge) return;
    const track = audioState.value.currentTrack;
    const playing =
      audioState.value.state === 'playing' ||
      audioState.value.state === 'loading';
    if (audioState.value.state === 'stopped') {
      bridge.stopService();
    } else {
      bridge.updateState(
        track?.title ?? 'Nexus Player',
        track?.artist ?? '',
        playing,
      );
    }
  }

  const isPlaying = computed(() => audioState.value.state === 'playing');
  const isPaused = computed(() => audioState.value.state === 'paused');
  const isStopped = computed(() => audioState.value.state === 'stopped');
  const isLoading = computed(
    () => audioState.value.state === 'loading' || isWaitingForAudio.value,
  );
  const currentTrack = computed(() => audioState.value.currentTrack);
  const position = computed(() => currentPositionSecs.value);
  const fft = computed(() => currentPcm.value);
  const pcm = computed(() => currentPcm.value);
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
  const currentIndex = computed(
    () => queueState.value.currentPosition?.itemIndex ?? null,
  );

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
      case 'PlaybackResumed':
        audioState.value.state = 'playing';
        isWaitingForAudio.value = false;
        updateMediaSessionPlaybackState();
        syncNativeBridge();
        break;
      case 'PlaybackPaused':
        audioState.value.state = 'paused';
        isWaitingForAudio.value = false;
        updateMediaSessionPlaybackState();
        syncNativeBridge();
        break;
      case 'PlaybackStopped':
        audioState.value.state = 'stopped';
        isWaitingForAudio.value = false;
        audioState.value.currentTrack = null;
        currentPositionSecs.value = 0;
        updateMediaSessionPlaybackState();
        syncNativeBridge();
        break;
      case 'PositionUpdated':
        if (event.payload?.positionSecs !== undefined) {
          const now = Date.now();
          // Throttle reactive updates to 20fps max, but allow big jumps
          if (
            now - _lastPosUpdate > 50 ||
            Math.abs(event.payload.positionSecs - currentPositionSecs.value) > 1
          ) {
            currentPositionSecs.value = event.payload.positionSecs;
            if (event.payload.fft) {
              currentFft.value = event.payload.fft;
            }
            if (event.payload.pcm) {
              currentPcm.value = event.payload.pcm;
            }
            _lastPosUpdate = now;
          }
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
        currentPositionSecs.value = 0;
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
        isWaitingForAudio.value = false;
        if (event.payload?.state) {
          const { positionSecs, fft, ...rest } = event.payload.state;
          audioState.value = rest;
          currentPositionSecs.value = positionSecs;
          if (fft) {
            currentFft.value = fft;
          }
          updateMediaSessionMetadata();
          updateMediaSessionPlaybackState();
          syncNativeBridge();
        }
        if (event.payload?.queue) {
          queueState.value = event.payload.queue;
        }
        break;
      case 'DownloadStarted':
        if (event.payload?.title) {
          notificationStore.addNotification(
            `Download started: ${event.payload.title}`,
            'success',
          );
        }
        break;
      case 'DownloadFinished':
        if (event.payload?.success) {
          notificationStore.addNotification(
            `Download finished: ${event.payload.title}`,
            'success',
          );
        } else {
          notificationStore.addNotification(
            `Download failed: ${event.payload.title}`,
            'error',
          );
        }
        break;
      case 'Error':
        isWaitingForAudio.value = false;
        console.error('Audio error:', event.payload?.message);
        notificationStore.addNotification(
          `Audio Error: ${event.payload?.message}`,
          'error',
        );
        break;
      case 'TrackUnavailable':
        if (event.payload?.trackId !== undefined) {
          lastUnavailableTrackId.value = event.payload.trackId;
          notificationStore.addNotification('Трек недоступен и помечен как недоступный', 'error');
        }
        break;
    }
  }

  function startLoading() {
    isWaitingForAudio.value = true;
  }

  function stopLoading() {
    isWaitingForAudio.value = false;
  }

  function updateAudioState(newState: AudioState) {
    const { positionSecs, ...rest } = newState;
    audioState.value = rest as any;
    currentPositionSecs.value = positionSecs;
  }

  function updateQueueState(newState: QueueState) {
    queueState.value = newState;
  }

  function setPosition(positionSecs: number) {
    currentPositionSecs.value = positionSecs;
  }

  function playNext() {
    startLoading();
    invoke('play_next').catch((e) => {
      console.error('Failed to play next:', e);
      stopLoading();
    });
  }

  function playPrevious() {
    startLoading();
    invoke('play_previous').catch((e) => {
      console.error('Failed to play previous:', e);
      stopLoading();
    });
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
    fft,
    pcm,
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
    lastUnavailableTrackId,
    handleAudioEvent,
    startLoading,
    stopLoading,
    updateAudioState,
    updateQueueState,
    setPosition,
    playNext,
    playPrevious,
  };
});
