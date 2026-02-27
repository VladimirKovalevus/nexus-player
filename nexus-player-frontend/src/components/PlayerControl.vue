<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useAudioStore } from '../stores/audio';
import WaveformBar from './common/WaveformBar.vue';

const audioStore = useAudioStore();

interface Props {
  queueVisible?: boolean;
}

withDefaults(defineProps<Props>(), {
  queueVisible: true,
});

const isLoading = ref(false);
const volumePopupOpen = ref(false);
const volumeWrapperRef = ref<HTMLElement | null>(null);
let updateInterval: number | null = null;
let unlistenFn: (() => void) | null = null;

const volumeIcon = computed(() => {
  const v = audioStore.volume;
  if (v === 0) return '🔇';
  if (v < 0.35) return '🔈';
  if (v < 0.7) return '🔉';
  return '🔊';
});

const toggleVolumePopup = () => {
  volumePopupOpen.value = !volumePopupOpen.value;
};

const handleDocumentClick = (e: MouseEvent) => {
  if (volumeWrapperRef.value && !volumeWrapperRef.value.contains(e.target as Node)) {
    volumePopupOpen.value = false;
  }
};

const emit = defineEmits<{
  toggleQueue: [];
}>();

const fetchAudioState = async () => {
  try {
    const state = await invoke<any>('get_audio_state');
    audioStore.updateAudioState(state);
  } catch (e) {
    console.error('Failed to fetch audio state:', e);
  }
};
defineExpose({ fetchAudioState });

const fetchQueueState = async () => {
  try {
    const state = await invoke<any>('get_queue_state');
    audioStore.updateQueueState(state);
  } catch (e) {
    console.error('Failed to fetch queue state:', e);
  }
};

const togglePlayPause = async () => {
  isLoading.value = true;
  console.log(audioStore,"STORE");
  try {
    if (audioStore.isPlaying) {
      await invoke('pause');
    } else if (audioStore.isPaused) {
      await invoke('resume');
      }
    if (audioStore.isStopped) {

      await invoke('play_at_index', { index: 0 });
    }
  } catch (e) {
    console.error('Failed to toggle play/pause:', e);
  } finally {
    isLoading.value = false;
  }
};

const playNext = async () => {
  isLoading.value = true;
  try {
    await invoke('play_next');
  } catch (e) {
    console.error('Failed to play next:', e);
  } finally {
    isLoading.value = false;
  }
};

const playPrevious = async () => {
  isLoading.value = true;
  try {
    await invoke('play_previous');
  } catch (e) {
    console.error('Failed to play previous:', e);
  } finally {
    isLoading.value = false;
  }
};

const cycleRepeatMode = async () => {
  const modes: Array<'off' | 'all' | 'one'> = ['off', 'all', 'one'];
  const currentIndex = modes.indexOf(audioStore.repeatMode);
  const nextMode = modes[(currentIndex + 1) % modes.length];

  try {
    await invoke('set_repeat_mode', { mode: nextMode });
    audioStore.queueState.repeatMode = nextMode;
  } catch (e) {
    console.error('Failed to set repeat mode:', e);
  }
};

const toggleShuffleMode = async () => {
  try {
    await invoke('toggle_shuffle');
    audioStore.queueState.shuffleEnabled = !audioStore.shuffleEnabled;
  } catch (e) {
    console.error('Failed to toggle shuffle:', e);
  }
};

const setVolume = async (event: Event) => {
  const target = event.target as HTMLInputElement;
  const volume = parseFloat(target.value);
  try {
    await invoke('set_volume', { volume });
  } catch (e) {
    console.error('Failed to set volume:', e);
  }
};

const formatTime = (seconds: number): string => {
  if (!seconds || !isFinite(seconds)) return '0:00';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

onMounted(async () => {
  fetchAudioState();
  fetchQueueState();
  document.addEventListener('click', handleDocumentClick);

  try {
    unlistenFn = await listen('audio-event', (event: any) => {
      audioStore.handleAudioEvent(event.payload);

      if (
        event.payload.type === 'PlaybackStarted' ||
        event.payload.type === 'TrackFinished'
      ) {
        fetchAudioState();
      }
    });
    console.log('Subscribed to audio events');
  } catch (e) {
    console.error('Failed to subscribe to audio events:', e);
  }
});

onUnmounted(() => {
  document.removeEventListener('click', handleDocumentClick);

  if (unlistenFn) {
    unlistenFn();
    console.log('Unsubscribed from audio events');
  }

  if (updateInterval !== null) {
    clearInterval(updateInterval);
  }
});
</script>

<template>
  <div class="player-control">
    <!-- Left: Track Info -->
    <div class="left-section">
      <div class="track-info">
        <div v-if="audioStore.currentTrack?.artworkUrl" class="track-artist">
            <img height="64" width="64" :src="audioStore.currentTrack?.artworkUrl">
        </div>
        <div style="display:flex; flex-direction: column;">
        <div v-if="audioStore.currentTrack" class="current-track">
          {{ audioStore.currentTrack.title }}
        </div>
        <div v-else class="no-track">No track playing</div>
        <div v-if="audioStore.currentTrack" class="track-artist">
          {{ audioStore.currentTrack.artist || 'Unknown Artist' }}
        </div>
        </div>
      </div>
    </div>

    <!-- Center: Waveform + Controls -->
    <div class="center-section">
      <div class="waveform-section">
        <span class="time">{{ formatTime(audioStore.position) }}</span>
        <div class="waveform-wrapper">
          <WaveformBar />
        </div>
        <span class="time">{{ formatTime(audioStore.duration) }}</span>
      </div>

      <div class="controls">
        <button
          class="control-btn medium-btn"
          @click="playPrevious"
          title="Previous"
          :disabled="isLoading"
        >
          ⏮
        </button>
        <button
          class="control-btn play-btn"
          @click="togglePlayPause"
          title="Play/Pause"
          :disabled="isLoading"
        >
          <span v-if="isLoading" class="loader">●●●</span>
          <span v-else>{{ audioStore.isPlaying ? '⏸' : '▶' }}</span>
        </button>
        <button
          class="control-btn medium-btn"
          @click="playNext"
          title="Next"
          :disabled="isLoading"
        >
          ⏭
        </button>
      </div>
    </div>

    <!-- Right: Volume + Queue -->
    <div class="right-section">
      <div class="volume-wrapper" ref="volumeWrapperRef">
        <button
          class="volume-btn"
          :class="{ active: volumePopupOpen }"
          @click.stop="toggleVolumePopup"
          :title="'Громкость: ' + Math.round(audioStore.volume * 100) + '%'"
        >
          {{ volumeIcon }}
        </button>
        <Transition name="volume-popup">
          <div v-if="volumePopupOpen" class="volume-popup" @click.stop>
            <span class="volume-percent">{{ Math.round(audioStore.volume * 100) }}%</span>
            <input
            style="padding:0 !important;"
              type="range"
              class="volume-slider-vertical"
              :value="audioStore.volume"
              min="0"
              max="1"
              step="0.01"
              @input="setVolume"
            />
          </div>
        </Transition>
      </div>

      <button
        class="queue-toggle-btn"
        :class="{ active: queueVisible }"
        @click="emit('toggleQueue')"
        :title="queueVisible ? 'Скрыть очередь' : 'Показать очередь'"
      >
        <svg
          v-if="queueVisible"
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="9 18 15 12 9 6"></polyline>
        </svg>
        <svg
          v-else
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <line x1="8" y1="6" x2="21" y2="6"></line>
          <line x1="8" y1="12" x2="21" y2="12"></line>
          <line x1="8" y1="18" x2="21" y2="18"></line>
          <line x1="3" y1="6" x2="3.01" y2="6"></line>
          <line x1="3" y1="12" x2="3.01" y2="12"></line>
          <line x1="3" y1="18" x2="3.01" y2="18"></line>
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped lang="scss">
.player-control {
  width: 100%;
  height: 100px;
  background-color: var(--color-bg-player);
  border-top: 1px solid var(--color-border);
  padding: 0.5rem 1rem;
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  box-shadow: 0 -4px 12px rgba(0, 0, 0, 0.1);
}

@media (max-width: 480px) {
  .player-control {
    padding: 0.5rem 0.5rem;
  }
}

.left-section {
  flex: 1;
  min-width: 0;
@media (max-width: 480px){
        display:none;
}
}

.track-info {
  display: flex;
  gap: 8px;
}

.current-track {
  font-weight: 600;
  font-size: 0.95rem;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-artist {
  font-size: 0.8rem;
  color: var(--color-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.no-track {
  font-size: 0.9rem;
  color: var(--color-text-muted);
}

.center-section {
  flex: 4;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.waveform-section {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 1rem;
}

.waveform-wrapper {
  flex: 1;
  min-width: 0;
  height: 32px;
}

.controls {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 0.75rem;
}

.control-btn {
  background: none;
  border: 1px solid var(--color-border);
  border-radius: 50%;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  color: var(--color-text-primary);
}

.control-btn:hover:not(:disabled) {
  background-color: var(--color-accent-primary);
  border-color: var(--color-accent-primary);
  color: white;
  transform: scale(1.05);
}

.play-btn {
  width: 40px;
  height: 40px;
  font-size: 1.1rem;
  border-width: 2px;
  border-color: var(--color-accent-primary);
  color: var(--color-accent-primary);
}

.medium-btn {
  width: 26px;
  height: 26px;
  font-size: 0.8rem;
}

.small-btn {
  width: 28px;
  height: 28px;
  font-size: 0.8rem;
  border: none;
  opacity: 0.7;
}

.control-btn.active {
  color: var(--color-accent-primary);
  opacity: 1;
}

.loader {
  font-size: 0.6rem;
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0% { opacity: 0.5; }
  50% { opacity: 1; }
  100% { opacity: 0.5; }
}

.right-section {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.75rem;
}

/* Volume popup */
.volume-wrapper {
  position: relative;
}

.volume-btn {
  background: transparent;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  font-size: 1rem;
  color: var(--color-text-secondary);
  transition: all 0.2s;
}

.volume-btn:hover,
.volume-btn.active {
  background-color: var(--color-bg-secondary);
  color: var(--color-accent-primary);
  border-color: var(--color-accent-primary);
}

.volume-popup {
  position: absolute;
  bottom: calc(100% + 8px);
  right: 0;
  background-color: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 10px;
  padding: 12px 10px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  box-shadow: 0 -4px 20px rgba(0, 0, 0, 0.4);
  z-index: 200;
  min-width: 44px;
}

.volume-percent {
  font-size: 0.65rem;
  color: var(--color-text-secondary);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.volume-slider-vertical {
  -webkit-appearance: slider-vertical;
  writing-mode: vertical-lr;
  direction: rtl;
  width: 6px;
  height: 100px;
  cursor: pointer;
  accent-color: var(--color-accent-primary);
  background: transparent;
}

/* Popup transition */
.volume-popup-enter-active,
.volume-popup-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.volume-popup-enter-from,
.volume-popup-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

.queue-toggle-btn {
  background: transparent;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s;
  color: var(--color-text-secondary);
}

.queue-toggle-btn.active, .queue-toggle-btn:hover {
  background-color: var(--color-bg-secondary);
  color: var(--color-accent-primary);
  border-color: var(--color-accent-primary);
}

.time {
  font-family: 'Courier New', monospace;
  font-size: 0.75rem;
  color: var(--color-text-secondary);
  min-width: 35px;
}
</style>

