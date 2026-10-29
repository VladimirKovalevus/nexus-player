<script setup lang="ts">
import type { AudioEvent, AudioState, QueueState } from '@/types/types';
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@/utils/providerAuth';
import { isAndroid } from '@/utils/platform';
import { sourceLabel } from '@/utils/sourceLabel';
import { listen } from '@tauri-apps/api/event';
import { useAudioStore } from '../stores/audio';
import { useEqStore } from '../stores/eq';
import WaveformBar from './common/WaveformBar.vue';
import LevelMeter from './deck/LevelMeter.vue';
import EqDisplay from './deck/EqDisplay.vue';
import EqPanel from './EqPanel.vue';
import AppIcon from './common/AppIcon.vue';
import AppImage from './common/AppImage.vue';

const audioStore = useAudioStore();
const eqStore = useEqStore();
const localVolume = ref(audioStore.volume);

// Keep local volume in sync with store (e.g. if changed elsewhere)
watch(
  () => audioStore.volume,
  (newVol) => {
    localVolume.value = newVol;
  },
);

interface Props {
  queueVisible?: boolean;
}

withDefaults(defineProps<Props>(), {
  queueVisible: true,
});

const volumePopupOpen = ref(false);
const volumeWrapperRef = ref<HTMLElement | null>(null);
let unlistenFn: (() => void) | null = null;

const volumeIcon = computed(() => {
  const v = audioStore.volume;
  if (v === 0) return 'volume-mute';
  if (v < 0.35) return 'volume-low';
  if (v < 0.7) return 'volume-medium';
  return 'volume-high';
});

const toggleVolumePopup = () => {
  volumePopupOpen.value = !volumePopupOpen.value;
};

const handleDocumentClick = (e: MouseEvent) => {
  if (
    volumeWrapperRef.value &&
    !volumeWrapperRef.value.contains(e.target as Node)
  ) {
    volumePopupOpen.value = false;
  }
};

const emit = defineEmits<{
  toggleQueue: [];
  toggleVisualizer: [];
}>();

const fetchAudioState = async () => {
  try {
    const state = await invoke<AudioState>('get_audio_state');
    audioStore.updateAudioState(state);
  } catch (e) {
    console.error('Failed to fetch audio state:', e);
  }
};
defineExpose({ fetchAudioState });

const fetchQueueState = async () => {
  try {
    const state = await invoke<QueueState>('get_queue_state');
    audioStore.updateQueueState(state);
  } catch (e) {
    console.error('Failed to fetch queue state:', e);
  }
};

const togglePlayPause = async () => {
  audioStore.startLoading();
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
    audioStore.stopLoading();
  }
};

const playNext = async () => {
  audioStore.startLoading();
  try {
    await invoke('play_next');
  } catch (e) {
    console.error('Failed to play next:', e);
    audioStore.stopLoading();
  }
};

const playPrevious = async () => {
  audioStore.startLoading();
  try {
    await invoke('play_previous');
  } catch (e) {
    console.error('Failed to play previous:', e);
    audioStore.stopLoading();
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

const handleVolumeWheel = async (event: WheelEvent) => {
  // Prevent page scroll when hovering volume
  event.preventDefault();

  const step = 0.05;
  const delta = event.deltaY < 0 ? step : -step;
  const newVolume = Math.min(1, Math.max(0, audioStore.volume + delta));

  if (newVolume !== audioStore.volume) {
    localVolume.value = newVolume;
    try {
      await invoke('set_volume', { volume: newVolume });
    } catch (e) {
      console.error('Failed to set volume via wheel:', e);
    }
  }
};

const formatTime = (seconds: number): string => {
  if (!seconds || !isFinite(seconds)) return '00:00';
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60).toString().padStart(2, '0');
  const secs = Math.floor(seconds % 60).toString().padStart(2, '0');
  return hours > 0 ? `${hours}:${mins}:${secs}` : `${mins}:${secs}`;
};

const progress = computed(() =>
  audioStore.duration > 0 ? Math.min(100, (audioStore.position / audioStore.duration) * 100) : 0,
);

const repeatTitleKey = computed(() => `player.repeat.${audioStore.repeatMode}`);

const sourceKey = computed(() => {
  const state = audioStore.currentTrack?.state?.type;
  return state === 'cached' ? 'track.source.cached' : 'track.source.local';
});

onMounted(async () => {
  fetchAudioState();
  fetchQueueState();
  eqStore.load();
  document.addEventListener('click', handleDocumentClick);

  try {
    unlistenFn = await listen('audio-event', (event: { payload: AudioEvent }) => {
      audioStore.handleAudioEvent(event.payload);

      if (
        event.payload.type === 'PlaybackStarted' ||
        event.payload.type === 'TrackFinished'
      ) {
        fetchAudioState();
      }
    });
  } catch (e) {
    console.error('Failed to subscribe to audio events:', e);
  }
});

onUnmounted(() => {
  document.removeEventListener('click', handleDocumentClick);

  if (unlistenFn) {
    unlistenFn();
  }

});
</script>

<template>
  <div :class="$style.deck">
    <div :class="[$style.screen, $style.readout]">
      <div :class="$style.art">
        <AppImage
          v-if="audioStore.currentTrack?.artworkUrl"
          :src="audioStore.currentTrack.artworkUrl"
          :alt="audioStore.currentTrack.title"
          variant="large"
          :class="$style.artImage"
        />
        <AppIcon v-else name="music-note" :size="24" />
      </div>
      <div :class="$style.readoutText">
        <div :class="$style.time">
          {{ formatTime(audioStore.position) }}<small> / {{ formatTime(audioStore.duration) }}</small>
        </div>
        <div :class="$style.trackTitle" :title="audioStore.currentTrack?.title">
          {{ audioStore.currentTrack?.title ?? $t('player.noTrack') }}
        </div>
        <div v-if="audioStore.currentTrack" :class="$style.trackMeta">
          {{ audioStore.currentTrack.artist || $t('track.unknownArtist') }} ·
          {{ audioStore.currentTrack.source?.system ? sourceLabel(audioStore.currentTrack.source.system, $t) : $t(sourceKey) }}
        </div>
      </div>
    </div>

    <div :class="$style.middle">
      <div :class="[$style.screen, $style.scope]">
        <div :class="$style.scopeInner">
          <WaveformBar />
          <i :class="$style.playhead" :style="{ left: `${progress}%` }" />
        </div>
      </div>

      <div :class="$style.keys">
        <button
          :class="[$style.key, { [$style.on]: audioStore.shuffleEnabled }]"
          :disabled="audioStore.isRadioActive"
          :title="audioStore.isRadioActive
            ? $t('player.unavailableInRadio')
            : audioStore.shuffleEnabled ? $t('player.shuffleOff') : $t('player.shuffleOn')"
          :aria-label="$t('player.shuffle')"
          @click="toggleShuffleMode"
        >
          <AppIcon name="shuffle" :size="15" />
          <span :class="$style.keyLabel">{{ $t('player.shuffle') }}</span>
        </button>
        <button :class="$style.key" :title="$t('player.previous')" :aria-label="$t('player.previous')" @click="playPrevious">
          <AppIcon name="prev" :size="16" />
        </button>
        <button
          :class="[$style.key, $style.mainKey]"
          :title="audioStore.isPlaying ? $t('player.pause') : $t('player.play')"
          :aria-label="audioStore.isPlaying ? $t('player.pause') : $t('player.play')"
          :disabled="audioStore.isLoading"
          @click="togglePlayPause"
        >
          <AppIcon v-if="audioStore.isLoading" name="spinner" :size="16" :class="$style.loader" />
          <AppIcon v-else :name="audioStore.isPlaying ? 'pause' : 'play'" :size="16" />
        </button>
        <button :class="$style.key" :title="$t('player.next')" :aria-label="$t('player.next')" @click="playNext">
          <AppIcon name="next" :size="16" />
        </button>
        <button
          :class="[$style.key, { [$style.on]: audioStore.repeatMode !== 'off' }]"
          :title="$t(repeatTitleKey)"
          :aria-label="$t(repeatTitleKey)"
          @click="cycleRepeatMode"
        >
          <AppIcon :name="audioStore.repeatIcon" :size="15" />
          <span :class="$style.keyLabel">{{ $t('player.repeatLabel') }}</span>
        </button>

        <span :class="$style.gap" />

        <button :class="$style.key" :title="$t('player.visualizer')" :aria-label="$t('player.visualizer')" @click="emit('toggleVisualizer')">
          <AppIcon name="visualizer" :size="15" />
          <span :class="$style.keyLabel">{{ $t('player.visualizerLabel') }}</span>
        </button>
        <button
          :class="['eq-btn', $style.key, { [$style.on]: eqStore.settings.enabled }]"
          :title="$t('player.equalizer')"
          :aria-label="$t('player.equalizer')"
          @click="eqStore.toggleOpen()"
        >
          <span :class="$style.eqText">EQ</span>
        </button>

        <div v-if="!isAndroid" ref="volumeWrapperRef" :class="$style.volume" @wheel.stop.prevent="handleVolumeWheel">
          <button
            :class="[$style.key, { [$style.on]: volumePopupOpen }]"
            :title="$t('player.volume', { percent: Math.round(audioStore.volume * 100) })"
            :aria-label="$t('player.volume', { percent: Math.round(audioStore.volume * 100) })"
            @click.stop="toggleVolumePopup"
          >
            <AppIcon :name="volumeIcon" :size="15" />
          </button>
          <div v-if="volumePopupOpen" :class="$style.volumePopup" @click.stop>
            <input
              type="range"
              :class="$style.volumeSlider"
              :style="{ '--volume': `${localVolume * 100}%` }"
              :value="localVolume"
              min="0"
              max="1"
              step="0.01"
              :aria-label="$t('player.volume', { percent: Math.round(audioStore.volume * 100) })"
              @input="setVolume"
            />
            <span :class="$style.volumePercent">{{ Math.round(audioStore.volume * 100) }}%</span>
          </div>
        </div>

        <button
          :class="[$style.key, { [$style.on]: queueVisible }]"
          :title="queueVisible ? $t('player.hideQueue') : $t('player.showQueue')"
          :aria-label="queueVisible ? $t('player.hideQueue') : $t('player.showQueue')"
          @click="emit('toggleQueue')"
        >
          <AppIcon name="queue" :size="15" />
          <span :class="$style.keyLabel">{{ $t('player.queue') }}</span>
        </button>
      </div>
    </div>

    <div :class="$style.right">
      <EqDisplay />
      <LevelMeter />
    </div>

    <EqPanel />
  </div>
</template>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.deck {
  display: grid;
  grid-template-columns: 292px minmax(0, 1fr) 232px;
  gap: 10px;
  padding: 10px;
}

.screen {
  background: var(--screen);
  color: var(--screen-text);
  border-radius: var(--radius-md);
  box-shadow: inset 0 0 0 1px var(--key-edge);
}

.readout {
  display: grid;
  grid-template-columns: 76px minmax(0, 1fr);
  gap: 10px;
  padding: 10px;
  min-width: 0;
}

.art {
  @include flex-center;
  width: 76px;
  height: 76px;
  overflow: hidden;
  background: var(--screen-track);
  color: var(--screen-muted);
}

.artImage {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.readoutText {
  min-width: 0;
}

.time {
  font: 600 2.125rem/1 $font-family-mono;
  letter-spacing: -0.02em;
  color: var(--led);
  text-shadow: 0 0 12px var(--led-glow);
  white-space: nowrap;

  small {
    font-size: var(--font-size-base);
    font-weight: 500;
    color: var(--screen-muted);
    text-shadow: none;
  }
}

.trackTitle {
  margin-top: 6px;
  font-weight: 600;
  font-size: var(--font-size-lg);
  @include text-ellipsis;
}

.trackMeta {
  font-size: var(--font-size-sm);
  color: var(--screen-muted);
  @include text-ellipsis;
}

.middle {
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto;
  gap: 8px;
  min-width: 0;
}

.scope {
  position: relative;
  height: 62px;
  padding: 7px 10px 12px;

  &::after {
    content: '';
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 5px;
    height: 4px;
    background: repeating-linear-gradient(90deg, var(--screen-track) 0 1px, transparent 1px 28px);
  }
}

.scopeInner {
  position: relative;
  height: 100%;
}

.playhead {
  position: absolute;
  top: -3px;
  bottom: -8px;
  width: 1px;
  background: var(--led-hot);
  pointer-events: none;
}

.keys {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.key {
  @include flex-center;
  flex: none;
  gap: 6px;
  height: 34px;
  min-width: 40px;
  padding: 0 9px;
  background: var(--key);
  border-radius: var(--radius-sm);
  box-shadow: inset 0 1px 0 var(--key-highlight), 0 2px 0 var(--key-edge);
  font: 600 0.6563rem $font-family-mono;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  white-space: nowrap;

  &:hover:not(:disabled) {
    filter: brightness(1.12);
  }

  &:active:not(:disabled) {
    transform: translateY(1px);
    box-shadow: inset 0 1px 0 var(--key-highlight), 0 1px 0 var(--key-edge);
  }

  &:disabled {
    opacity: 0.45;
    cursor: default;
  }
}

.on {
  color: var(--accent);

  &::before {
    content: '';
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
  }
}

.mainKey {
  min-width: 58px;
  background: var(--accent);
  color: var(--on-accent);
}

.loader {
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.gap {
  flex: 1;
}

.eqText {
  letter-spacing: 0.06em;
}

.volume {
  position: relative;
  flex: none;
}

.volumePopup {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  z-index: 1;
}

.volumeSlider {
  appearance: none;
  -webkit-appearance: none;
  width: 140px;
  height: 4px;
  padding: 0;
  border: 0;
  border-radius: 0;
  background: linear-gradient(to right, var(--accent) var(--volume), var(--line) var(--volume));

  &::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 10px;
    height: 16px;
    background: var(--text);
    border-radius: 1px;
  }

  &::-moz-range-thumb {
    width: 10px;
    height: 16px;
    border: 0;
    background: var(--text);
    border-radius: 1px;
  }
}

.volumePercent {
  min-width: 34px;
  text-align: right;
  font: 500 var(--font-size-xs) $font-family-mono;
}

.right {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 26px;
  gap: 8px;
  min-width: 0;
}

@media (max-width: 1100px) {
  .deck {
    grid-template-columns: 262px minmax(0, 1fr);
  }

  .right {
    display: none;
  }
}

@media (max-width: 1000px) {
  .keyLabel {
    display: none;
  }
}

// All keys must stay on screen: the queue key is the last one in the row.
@media (max-width: 760px) {
  .deck {
    grid-template-columns: 190px minmax(0, 1fr);
  }

  .key {
    min-width: 34px;
    padding: 0 6px;
  }
}

@media (max-width: $mobile-breakpoint) {
  .deck {
    grid-template-columns: minmax(0, 1fr);
    gap: 7px;
    padding: 8px;
  }

  .readout {
    grid-template-columns: 52px minmax(0, 1fr);
    padding: 7px 8px;
    gap: 9px;
  }

  .art {
    width: 52px;
    height: 52px;
  }

  .time {
    font-size: 1.375rem;

    small {
      font-size: var(--font-size-sm);
    }
  }

  .trackTitle {
    margin-top: 1px;
    font-size: var(--font-size-base);
  }

  .scope {
    height: 42px;
    padding: 5px 8px 9px;
  }

  .keys {
    gap: 3px;
  }

  .key {
    flex: 1;
    min-width: 0;
    height: 44px;
    padding: 0;
    gap: 0;
  }

  .on::before,
  .gap,
  .volume {
    display: none;
  }
}
</style>
