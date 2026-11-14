import { i18n } from '@/i18n';
import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { AudioState, QueueState, AudioEvent, SearchResult } from '@/types/types';

interface NexusAudioBridge {
  stopService: () => void;
  updateState: (title: string, artist: string, playing: boolean) => void;
}
import { invoke, requestProviderAuth } from '@/utils/providerAuth';
import { errorMessage } from '@/utils/errorMessage';
import { useNotificationStore } from './notifications';

export const useAudioStore = defineStore('audio', () => {
  const notificationStore = useNotificationStore();

  const trackDurations = ref<Record<number, number>>({});

  const lastUnavailableTrackId = ref<number | null>(null);
  const lastCachedTrack = ref<{ trackId: number; success: boolean } | null>(null);

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
    radioMode: null,
  });

  // Media Session Integration
  function updateMediaSessionMetadata() {
    if ('mediaSession' in navigator && audioState.value.currentTrack) {
      const track = audioState.value.currentTrack;
      navigator.mediaSession.metadata = new MediaMetadata({
        title: track.title,
        artist: track.artist ?? '',
        album: track.album || '',
        artwork: track.artworkUrl
          ? [{ src: track.artworkUrl, sizes: '512x512', type: 'image/png' }]
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
          position: Math.min(currentPositionSecs.value, audioState.value.durationSecs),
        });
      }
    }
  }

  if ('mediaSession' in navigator) {
    navigator.mediaSession.setActionHandler('play', () => controlPlayback('play'));
    navigator.mediaSession.setActionHandler('pause', () => invoke('pause'));
    navigator.mediaSession.setActionHandler('previoustrack', () =>
      playPrevious(),
    );
    navigator.mediaSession.setActionHandler('nexttrack', () =>
      playNext(),
    );
    navigator.mediaSession.setActionHandler('seekto', (details) => {
      if (details.seekTime !== undefined) {
        invoke('seek', { positionSecs: Math.max(details.seekTime, 0) });
      }
    });
  }

  if (typeof window !== 'undefined') {
    (window as unknown as { __nexusControl: (command: string) => Promise<unknown> }).__nexusControl = (command: string) => invoke(command);
  }

  function syncNativeBridge() {
    const bridge =
      typeof window !== 'undefined' ? (window as unknown as { _NexusAudio?: NexusAudioBridge })._NexusAudio : null;
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
  const radioMode = computed(() => queueState.value.radioMode);
  const isRadioActive = computed(() => queueState.value.radioMode !== null);
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
        return 'repeat';
      case 'all':
        return 'repeat';
      case 'one':
        return 'repeat-one';
      default:
        return 'repeat';
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
      case 'DurationChanged': {
        const { trackId, durationSecs } = event.payload ?? {};
        if (trackId === undefined || durationSecs === undefined ||
            !Number.isFinite(durationSecs) || durationSecs <= 0) break;
        trackDurations.value[trackId] = durationSecs;
        for (const track of tracks.value) {
          if (track.id === trackId) track.durationSecs = durationSecs;
        }
        if (audioState.value.currentTrack?.id === trackId) {
          audioState.value.currentTrack.durationSecs = durationSecs;
          audioState.value.durationSecs = durationSecs;
          updateMediaSessionPlaybackState();
        }
        break;
      }
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
          applyKnownDurations();
          updateMediaSessionMetadata();
        }
        updateMediaSessionPlaybackState();
        syncNativeBridge();
        break;
      case 'AudioStateUpdated':
        isWaitingForAudio.value = false;
        if (event.payload?.state) {
          const { positionSecs, fft, ...rest } = event.payload.state;
          audioState.value = rest as AudioState;
          applyKnownDurations();
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
          applyKnownDurations();
        }
        break;
      case 'DownloadStarted':
        if (event.payload?.title) {
          notificationStore.addNotification(
            i18n.global.t('downloads.cacheStarted', { title: event.payload.title }),
            'success',
          );
        }
        break;
      case 'DownloadFinished':
        if (event.payload?.trackId !== undefined) {
          lastCachedTrack.value = {
            trackId: event.payload.trackId,
            success: !!event.payload.success,
          };
        }
        if (event.payload?.success) {
          notificationStore.addNotification(
            i18n.global.t('downloads.cacheFinished', { title: event.payload.title }),
            'success',
          );
        } else {
          notificationStore.addNotification(
            i18n.global.t('downloads.cacheFailed', { title: event.payload?.title }),
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
          notificationStore.addNotification(i18n.global.t('track.unavailable'), 'error');
        }
        break;
      case 'ProviderChallenge':
        if (event.payload?.provider && event.payload?.url) {
          requestProviderAuth(event.payload.provider, event.payload.url)
            .then(() => invoke('retry_playback'))
            .catch((e) => console.error('Provider challenge failed:', e));
        }
        break;
    }
  }

  function applyKnownDurations() {
    for (const track of tracks.value) {
      const duration = trackDurations.value[track.id];
      if (duration !== undefined) track.durationSecs = duration;
    }
    const track = audioState.value.currentTrack;
    if (track && trackDurations.value[track.id] !== undefined) {
      track.durationSecs = trackDurations.value[track.id];
      audioState.value.durationSecs = track.durationSecs;
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
    audioState.value = rest as AudioState;
    applyKnownDurations();
    currentPositionSecs.value = positionSecs;
  }

  function updateQueueState(newState: QueueState) {
    queueState.value = newState;
    applyKnownDurations();
  }

  function setPosition(positionSecs: number) {
    currentPositionSecs.value = positionSecs;
  }

  interface SearchPlayArgs {
    system: string;
    contentType?: string | null;
    query: string;
    limit: number;
    offset: number;
    selectedIndex: number;
    resultId?: string | null;
  }

  /** Starts playback of a search result; failures become a notification and are not rethrown. */
  async function searchAndPlay(args: SearchPlayArgs): Promise<boolean> {
    return startResultPlayback('handle_search', { ...args });
  }

  async function playResolvedResult(searchResult: SearchResult): Promise<boolean> {
    return startResultPlayback('handle_streaming_track', {
      searchResult,
      strategy: 'cache',
      play: true,
    });
  }

  async function startResultPlayback(command: string, args: Record<string, unknown>): Promise<boolean> {
    const wasWaiting = isWaitingForAudio.value;
    startLoading();
    try {
      await invoke(command, args);
      return true;
    } catch (e) {
      console.error('Failed to play search result:', e);
      isWaitingForAudio.value = wasWaiting;
      useNotificationStore().addNotification(
        i18n.global.t('search.playFailed', { error: errorMessage(e) }),
        'error',
      );
      return false;
    }
  }

  let playbackContext: (() => Promise<boolean>) | null = null;

  function setPlaybackContext(context: () => Promise<boolean>) {
    playbackContext = context;
  }

  async function controlPlayback(command: 'play' | 'play_next' | 'play_previous') {
    if (isLoading.value) return;
    startLoading();
    try {
      if (queueState.value.entries.length === 0 && !isRadioActive.value && !(command === 'play' && isPlaying.value)) {
        if (!await playbackContext?.()) stopLoading();
        return;
      }
      if (command === 'play') {
        await invoke(isPlaying.value ? 'pause' : isPaused.value ? 'resume' : 'play_at_index',
          isStopped.value ? { index: 0 } : undefined);
      } else {
        await invoke(command);
      }
    } catch (e) {
      console.error('Failed to control playback:', e);
      stopLoading();
    }
  }

  function togglePlayPause() {
    return controlPlayback('play');
  }

  function playNext() {
    return controlPlayback('play_next');
  }

  function playPrevious() {
    return controlPlayback('play_previous');
  }

  return {
    trackDurations,
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
    radioMode,
    isRadioActive,
    entries,
    currentPosition,
    isSearchQueue,
    searchContext,
    tracks,
    currentIndex,
    repeatIcon,
    lastUnavailableTrackId,
    lastCachedTrack,
    handleAudioEvent,
    startLoading,
    stopLoading,
    updateAudioState,
    updateQueueState,
    setPosition,
    searchAndPlay,
    playResolvedResult,
    setPlaybackContext,
    togglePlayPause,
    playNext,
    playPrevious,
  };
});
