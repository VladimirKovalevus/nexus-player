import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { AudioState, QueueState, AudioEvent } from '@/types/types';

export const useAudioStore = defineStore('audio', () => {
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
        break;
      case 'PlaybackPaused':
        audioState.value.state = 'paused';
        break;
      case 'PlaybackResumed':
        audioState.value.state = 'playing';
        break;
      case 'PlaybackStopped':
        audioState.value.state = 'stopped';
        audioState.value.currentTrack = null;
        break;
      case 'PositionUpdated':
        if (event.payload?.positionSecs !== undefined) {
          audioState.value.positionSecs = event.payload.positionSecs;
        }
        break;
      case 'DurationChanged':
        if (event.payload?.durationSecs !== undefined) {
          audioState.value.durationSecs = event.payload.durationSecs;
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
        }
        break;
      case 'AudioStateUpdated':
        if (event.payload?.state) {
          audioState.value = event.payload.state;
        }
        if (event.payload?.queue) {
          queueState.value = event.payload.queue;
        }
        break;
      case 'Error':
        console.error('Audio error:', event.payload?.message);
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
