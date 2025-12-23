import { defineStore } from 'pinia';
import { ref, computed } from 'vue';

export interface TrackDto {
  id: number;
  title: string;
  artist: string | null;
  album: string | null;
  durationSecs: number | null;
  artworkUrl: string | null;
  playCount: number;
  state: string;
}

export interface AudioState {
  state: 'playing' | 'paused' | 'stopped' | 'buffering';
  positionSecs: number;
  durationSecs: number;
  volume: number;
  currentTrack: TrackDto | null;
}

export interface QueueState {
  tracks: TrackDto[];
  currentIndex: number | null;
  repeatMode: 'off' | 'all' | 'one';
  shuffleEnabled: boolean;
}

export interface AudioEvent {
  type: string;
  payload?: any;
}

export const useAudioStore = defineStore('audio', () => {
  const audioState = ref<AudioState>({
    state: 'stopped',
    positionSecs: 0,
    durationSecs: 0,
    volume: 1.0,
    currentTrack: null,
  });

  const queueState = ref<QueueState>({
    tracks: [],
    currentIndex: null,
    repeatMode: 'off',
    shuffleEnabled: false,
  });

  const isPlaying = computed(() => audioState.value.state === 'playing');
  const isPaused = computed(() => audioState.value.state === 'paused');
  const isStopped = computed(() => audioState.value.state === 'stopped');
  const currentTrack = computed(() => audioState.value.currentTrack);
  const position = computed(() => audioState.value.positionSecs);
  const duration = computed(() => audioState.value.durationSecs);
  const volume = computed(() => audioState.value.volume);
  const repeatMode = computed(() => queueState.value.repeatMode);
  const shuffleEnabled = computed(() => queueState.value.shuffleEnabled);
  const tracks = computed(() => queueState.value.tracks);
  const currentIndex = computed(() => queueState.value.currentIndex);

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
        if (event.payload?.position_secs !== undefined) {
          audioState.value.positionSecs = event.payload.position_secs;
        }
        break;
      case 'DurationChanged':
        if (event.payload?.duration_secs !== undefined) {
          audioState.value.durationSecs = event.payload.duration_secs;
        }
        break;
      case 'VolumeChanged':
        if (event.payload?.volume !== undefined) {
          audioState.value.volume = event.payload.volume;
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
    currentTrack,
    position,
    duration,
    volume,
    repeatMode,
    shuffleEnabled,
    tracks,
    currentIndex,
    repeatIcon,
    handleAudioEvent,
    updateAudioState,
    updateQueueState,
    setPosition,
  };
});
