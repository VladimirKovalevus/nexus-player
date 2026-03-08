<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useAudioStore } from '../stores/audio';
import WaveformBar from './common/WaveformBar.vue';

const audioStore = useAudioStore();
const localVolume = ref(audioStore.volume);

// Keep local volume in sync with store (e.g. if changed elsewhere)
watch(() => audioStore.volume, (newVol) => {
  localVolume.value = newVol;
});

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
  console.log(audioStore.isPlaying,audioStore.isStopped,"STORE");
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
  localVolume.value = volume; // Immediate UI update
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
        <div v-if="audioStore.currentTrack?.artworkUrl" class="track-art">
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

      <div class="controls">
        <button
          class="control-btn medium-btn"
          @click="playPrevious"
          title="Previous"
          :disabled="isLoading"
        >
            <img  src="/icon/prev.svg">
        </button>
        <button
          class="control-btn play-btn"
          @click="togglePlayPause"
          title="Play/Pause"
          :disabled="isLoading"
        >
          <span v-if="isLoading" class="loader">●●●</span>
          <span v-else>
            <img v-if="!audioStore.isPlaying" src="/icon/play.svg" alt="Play">
            <span v-else>
                <img src="/icon/pause.svg">
            </span>
          </span>
        </button>
        <button
          class="control-btn medium-btn"
          @click="playNext"
          title="Next"
          :disabled="isLoading"
        >
            <img src="/icon/next.svg">
        </button>
      </div>
      <div class="waveform-section">
        <span class="time">{{ formatTime(audioStore.position) }}</span>
        <div class="waveform-wrapper">
          <WaveformBar />
        </div>
        <span class="time">{{ formatTime(audioStore.duration) }}</span>
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
        <img src="/icon/volume.svg">
        </button>
        <Transition name="volume-popup">
          <div v-if="volumePopupOpen" class="volume-popup" @click.stop>
            <span class="volume-percent">{{ Math.round(audioStore.volume * 100) }}%</span>
            <input
              type="range"
              class="volume-slider-vertical"
              :style="{
                background: `linear-gradient(to right, white ${localVolume * 100}%, black ${localVolume * 100}%)`
              }"
              :value="localVolume"
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
      <img src="/icon/queue.svg">
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
  font-weight: 400;
  font-size: 1.45rem;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-art {
    width:64px;

}
.track-artist {
  font-size: 1.3rem;
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
  border-radius: 50%;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  color: var(--color-text-primary);
}

.control-btn:hover:not(:disabled) {
  background-color: var(--color-bg-hover);
  border-color: var(--color-bg-hover);
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
  // color: var(--color-accent-primary);
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
  color: black;
  border-color: var(--color-bg-hover);
}

.volume-popup {
  position: absolute;
  bottom: calc(100% + 8px);
  right: 0;
  background-color: var(--color-bg-sidebar);
  border: 1px solid var(--color-border);
  border-radius: 10px;
  padding: 12px 10px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  box-shadow: 0 -4px 20px rgba(0, 0, 0, 0.4);
  z-index: 1000 !important;
  min-width: 44px;
}

.volume-percent {
  font-size: 1.5rem;
  color: white;
  background-color: none;
  white-space: nowrap;
}

.volume-slider-vertical {
  -webkit-appearance: none;
  appearance: none;
  width: 100px; /* Длина ползунка (станет высотой после поворота) */
  height: 4px;   /* Толщина линии */
  outline: none;
  cursor: pointer;
  padding: 0;
  margin: 45px -48px;
  display: block;
  
  /* Поворачиваем горизонтальный слайдер вертикально */
  transform: rotate(-90deg);

  &::-webkit-slider-runnable-track {
    width: 100%;
    height: 100%;
    background: transparent;
  }

  &::-moz-range-track {
    width: 100%;
    height: 100%;
    background: black;
  }

  &::-moz-range-progress {
    background: white;
    height: 4px;
  }


  &::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    background: white;
    border: none;
    border-radius: 0;
    cursor: pointer;
    box-shadow: none;
    /* Выравнивание по центру линии 4px: (12 - 4) / 2 = 4 */
    margin-top: -5px; 
  }

  &::-moz-range-track {
    width: 100%;
    height: 100%;
    background: transparent;
  }

  &::-moz-range-thumb {
    width: 12px;
    height: 12px;
    background: white;
    border: none;
    border-radius: 0;
    cursor: pointer;
  }
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
  font-size: 2rem;
  color: white;
  min-width: 35px;
}
</style>

