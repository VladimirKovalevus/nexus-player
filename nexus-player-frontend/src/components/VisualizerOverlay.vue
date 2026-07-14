<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { useI18n } from 'vue-i18n';
import { useAudioStore } from '../stores/audio';
import { useRadioStore } from '../stores/radio';
import type { RadioMode } from '../types/types';
import { invoke } from '@/utils/providerAuth';
import { isAndroid } from '@/utils/platform';
import WaveformBar from './common/WaveformBar.vue';
import AppIcon from './common/AppIcon.vue';
import AppImage from './common/AppImage.vue';

// @ts-ignore
import butterchurn from 'butterchurn/dist/butterchurn.min.js';
// @ts-ignore
import butterchurnPresets from 'butterchurn-presets';

const props = defineProps<{
  isOpen: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();

const { t } = useI18n();
const audioStore = useAudioStore();
const radioStore = useRadioStore();
const { playNext, playPrevious } = audioStore;

const RADIO_MODES: Array<{ mode: RadioMode; labelKey: string }> = [
  { mode: 'tracks', labelKey: 'visualizer.modeTracks' },
  { mode: 'playlists', labelKey: 'visualizer.modePlaylists' },
];
const isCanvasActive = ref(false);
const canvasRef = ref<HTMLCanvasElement | null>(null);
const localVolume = ref(audioStore.volume);
const butterchurnError = ref<string | null>(null);

// Preset selector state
const showPresetList = ref(false);
const isRandomMode = ref(true);
const selectedPresetName = ref('Random');
const presetList = ref<string[]>([]);

// Butterchurn state
let visualizer: any = null;
let animationFrameId: number | null = null;
let presetInterval: number | null = null;
let resizeObserver: ResizeObserver | null = null;
let onWindowResize: (() => void) | null = null;
let lastRenderTime: number = 0;

// Butterchurn expects raw PCM time-domain data (Uint8Array, 0-255).
// It computes FFT internally. We pass mono data for all three channels.
const BCH_FFT_SIZE = 1024;
const audioLevels = {
  timeByteArray: new Uint8Array(BCH_FFT_SIZE).fill(128),
  timeByteArrayL: new Uint8Array(BCH_FFT_SIZE).fill(128),
  timeByteArrayR: new Uint8Array(BCH_FFT_SIZE).fill(128),
  freqArray: new Float32Array(BCH_FFT_SIZE / 2).fill(0),
  freqArrayL: new Float32Array(BCH_FFT_SIZE / 2).fill(0),
  freqArrayR: new Float32Array(BCH_FFT_SIZE / 2).fill(0),
};

const formatTime = (seconds: number): string => {
  if (!seconds || !isFinite(seconds)) return '0:00';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

const setVolume = async (event: Event) => {
  const target = event.target as HTMLInputElement;
  const volume = parseFloat(target.value);
  localVolume.value = volume;
  try {
    await invoke('set_volume', { volume });
  } catch (e) {
    console.error('Failed to set volume:', e);
  }
};

const togglePlayPause = async () => {
  // Nothing loaded: no command would run, and the loading state would never clear.
  if (!audioStore.isPlaying && !audioStore.isPaused) return;
  audioStore.startLoading();
  try {
    if (audioStore.isPlaying) await invoke('pause');
    else if (audioStore.isPaused) await invoke('resume');
  } catch (e) {
    console.error('Playback toggle failed:', e);
    audioStore.stopLoading();
  }
};

const loadRandomPreset = () => {
  if (!visualizer) return;
  const presets = butterchurnPresets.getPresets();
  const presetNames = Object.keys(presets);
  const name = presetNames[Math.floor(Math.random() * presetNames.length)];
  const randomPreset = presets[name];
  visualizer.loadPreset(randomPreset, 2.7);
  
  // Update the name even if in random mode so user sees what's playing
  selectedPresetName.value = name;
};

const selectPreset = (name: string) => {
  if (!visualizer) return;
  
  if (name === 'Random') {
    isRandomMode.value = true;
    selectedPresetName.value = 'Random';
    loadRandomPreset();
    if (!presetInterval) {
      presetInterval = window.setInterval(loadRandomPreset, 30000);
    }
  } else {
    isRandomMode.value = false;
    selectedPresetName.value = name;
    const presets = butterchurnPresets.getPresets();
    visualizer.loadPreset(presets[name], 2.7);
    if (presetInterval) {
      clearInterval(presetInterval);
      presetInterval = null;
    }
  }
  showPresetList.value = false;
};

const togglePresetList = () => {
  showPresetList.value = !showPresetList.value;
};

const destroyVisualizer = () => {
  if (animationFrameId) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }
  if (presetInterval) {
    clearInterval(presetInterval);
    presetInterval = null;
  }
  if (visualizer) {
    try {
      visualizer.loseGLContext();
    } catch (e) {
      console.warn('[Butterchurn] Error losing GL context:', e);
    }
    visualizer = null;
  }
  lastRenderTime = 0;
};

const initButterchurn = () => {
  if (!canvasRef.value) return;
  butterchurnError.value = null;

  try {
    const canvas = canvasRef.value;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;

    if (width <= 0 || height <= 0) {
      console.warn('[Butterchurn] Canvas dimensions are zero, delaying init:', { width, height });
      return;
    }

    // We use the new Raw interface from the fork:
    // Passing null as the first argument disables internal AudioContext/Analyser creation.
    visualizer = butterchurn.createVisualizer(null, canvas, {
      width,
      height,
      pixelRatio: window.devicePixelRatio || 1,
      textureRatio: 1,
    });

    if (!visualizer || !visualizer.gl) {
      console.error('[Butterchurn] Initialization failed: No GL context returned', { visualizer });
      throw new Error('WebGL context could not be created. Check browser support.');
    }

    // Get preset list
    const presets = butterchurnPresets.getPresets();
    presetList.value = ['Random', ...Object.keys(presets)];

    if (isRandomMode.value) {
      loadRandomPreset();
      presetInterval = window.setInterval(loadRandomPreset, 30000);
    } else {
      selectPreset(selectedPresetName.value);
    }

    // Set initial canvas size and observe container for future resizes
    resizeCanvas(width, height);
    resizeObserver?.disconnect();
    resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const { width, height } = entry.contentRect;
      resizeCanvas(width, height);
    });
    resizeObserver.observe(canvas.parentElement ?? canvas);

    if (onWindowResize) window.removeEventListener('resize', onWindowResize);
    onWindowResize = () => {
      const c = canvasRef.value;
      if (c) resizeCanvas(c.clientWidth, c.clientHeight);
    };
    window.addEventListener('resize', onWindowResize);

    butterchurnError.value = null;
  } catch (e: any) {
    console.error('[Butterchurn] Init Error:', e);
    butterchurnError.value = e.message;
  }
};

const resizeCanvas = (width: number, height: number) => {
  const canvas = canvasRef.value;
  if (!canvas || !visualizer || width <= 0 || height <= 0) return;
  canvas.width = Math.floor(width);
  canvas.height = Math.floor(height);
  visualizer.setRendererSize(width, height);
};

const renderLoop = () => {
  if (!isCanvasActive.value || !visualizer) return;

  const now = performance.now();
  const deltaTime = lastRenderTime ? (now - lastRenderTime) / 1000 : 1/60;
  lastRenderTime = now;

  // 2. Fill PCM time-domain and FFT frequency data for butterchurn
  const pcm = audioStore.pcm;
  const fft = audioStore.fft;

  if (audioStore.isPlaying && pcm && pcm.length > 0) {
    const src = pcm;
    for (let i = 0; i < BCH_FFT_SIZE; i++) {
      const idx = Math.floor((i / BCH_FFT_SIZE) * src.length);
      const v = src[idx];
      audioLevels.timeByteArray[i] = v;
      audioLevels.timeByteArrayL[i] = v;
      audioLevels.timeByteArrayR[i] = v;
    }
  } else {
    audioLevels.timeByteArray.fill(128);
    audioLevels.timeByteArrayL.fill(128);
    audioLevels.timeByteArrayR.fill(128);
  }

  if (audioStore.isPlaying && fft && fft.length > 0) {
    const halfSize = BCH_FFT_SIZE / 2;
    for (let i = 0; i < halfSize; i++) {
      const idx = Math.floor((i / halfSize) * fft.length);
      const v = fft[idx];
      audioLevels.freqArray[i] = v;
      audioLevels.freqArrayL[i] = v;
      audioLevels.freqArrayR[i] = v;
    }
  } else {
    audioLevels.freqArray.fill(0);
    audioLevels.freqArrayL.fill(0);
    audioLevels.freqArrayR.fill(0);
  }

  // 3. Render using the new renderRaw method which accepts deltaTime
  if (typeof visualizer.renderRaw === 'function') {
    visualizer.renderRaw(audioLevels, deltaTime);
  } else {
    visualizer.render({ audioLevels });
  }
  
  animationFrameId = requestAnimationFrame(renderLoop);
};

const handleCanvasClick = () => {
  loadRandomPreset();
};

const startVisualizer = async () => {
  destroyVisualizer();
  
  // Always reinit: canvas element may have been replaced by v-if
  let attempts = 0;
  const checkAndInit = async () => {
    await nextTick();
    if (canvasRef.value) {
      initButterchurn();
      if (visualizer && visualizer.gl) {
        renderLoop();
      }
    } else if (attempts < 10) {
      attempts++;
      setTimeout(checkAndInit, 100);
    }
  };
  await nextTick();
  checkAndInit();
};

watch(isCanvasActive, async (val) => {
  if (val) {
    await startVisualizer();
  } else {
    destroyVisualizer();
  }
});

watch(() => props.isOpen, async (val) => {
  if (!val) {
    // Overlay closing: canvas DOM will be destroyed, invalidate visualizer
    destroyVisualizer();
    resizeObserver?.disconnect();
    resizeObserver = null;
    if (onWindowResize) {
      window.removeEventListener('resize', onWindowResize);
      onWindowResize = null;
    }
  } else if (isCanvasActive.value) {
    // Overlay reopening while canvas mode was already active
    await startVisualizer();
  }
});

const handleDocumentClick = (e: MouseEvent) => {
  const target = e.target as HTMLElement;
  if (!target.closest('.preset-selector')) {
    showPresetList.value = false;
  }
};

onMounted(() => {
  document.addEventListener('click', handleDocumentClick);
});

onUnmounted(() => {
  destroyVisualizer();
  resizeObserver?.disconnect();
  if (onWindowResize) window.removeEventListener('resize', onWindowResize);
  document.removeEventListener('click', handleDocumentClick);
});

watch(() => audioStore.volume, (newVol) => {
  localVolume.value = newVol;
});
const isFullscreen = ref(false);

const toggleFullscreen = async () => {
  isFullscreen.value = !isFullscreen.value;
  await nextTick();
  const canvas = canvasRef.value;
  if (canvas) resizeCanvas(canvas.clientWidth, canvas.clientHeight);
};

const presetLabel = (name: string) => (name === 'Random' ? t('visualizer.random') : name);
</script>

<template>
  <Transition name="overlay-slide">
    <div v-if="isOpen" class="player-overlay" :class="{ 'is-fullscreen': isFullscreen }">
      <div class="top-bar">
        <button
          class="deck-btn"
          :title="$t('visualizer.fullscreen')"
          :aria-label="$t('visualizer.fullscreen')"
          @click="toggleFullscreen"
        >
          <AppIcon :name="isFullscreen ? 'fullscreen-exit' : 'fullscreen-enter'" :size="14" />
        </button>

        <button
          class="deck-btn"
          :class="{ 'is-on': radioStore.isActive }"
          :disabled="radioStore.isStarting"
          :title="$t(radioStore.isActive ? 'radio.turnOff' : 'radio.turnOn')"
          @click="radioStore.toggle()"
        >
          <AppIcon name="radio" :size="13" /> {{ $t('nav.radio') }}
        </button>
        <div class="seg">
          <button
            v-for="option in RADIO_MODES"
            :key="option.mode"
            :aria-pressed="radioStore.selectedMode === option.mode"
            @click="radioStore.selectMode(option.mode)"
          >
            {{ $t(option.labelKey) }}
          </button>
        </div>

        <template v-if="isCanvasActive">
          <button
            class="deck-btn"
            :title="$t('visualizer.backToCover')"
            :aria-label="$t('visualizer.backToCover')"
            @click="isCanvasActive = false"
          >
            <AppIcon name="image" :size="14" />
          </button>

          <div class="preset-selector">
            <button class="deck-btn preset-btn" :title="selectedPresetName" @click="togglePresetList">
              <AppIcon name="palette" :size="13" />
              <span class="preset-name">
                {{ isRandomMode ? $t('visualizer.randomWith', { name: selectedPresetName }) : selectedPresetName }}
              </span>
            </button>

            <div v-if="showPresetList" class="preset-dropdown">
              <button
                v-for="name in presetList"
                :key="name"
                class="preset-item"
                :class="{ active: selectedPresetName === name }"
                @click="selectPreset(name)"
              >
                {{ presetLabel(name) }}
              </button>
            </div>
          </div>
        </template>

        <span class="view-spacer" />
        <button class="deck-btn" :title="$t('common.close')" :aria-label="$t('common.close')" @click="emit('close')">
          <AppIcon name="close" :size="14" />
        </button>
      </div>

      <div class="stage" @click="handleCanvasClick">
        <Transition name="fade" mode="out-in">
          <div v-if="!isCanvasActive" key="cover" class="cover-wrapper" @click.stop="isCanvasActive = true">
            <AppImage :src="audioStore.currentTrack?.artworkUrl" variant="large" class="big-cover">
              <template #placeholder>
                <div class="big-cover-placeholder"><AppIcon name="music-note" :size="96" /></div>
              </template>
            </AppImage>
            <div class="stage-hint">{{ $t('visualizer.startHint') }}</div>
          </div>
          <div v-else key="canvas" class="canvas-wrapper">
            <div v-if="butterchurnError" class="error-msg">{{ $t('common.error', { error: butterchurnError }) }}</div>
            <canvas ref="canvasRef"></canvas>
            <div class="stage-hint">{{ $t('visualizer.changeHint') }}</div>
          </div>
        </Transition>
      </div>

      <div class="console">
        <div class="track-details">
          <div class="title" :title="audioStore.currentTrack?.title">{{ audioStore.currentTrack?.title || $t('player.noTrack') }}</div>
          <div class="artist">{{ audioStore.currentTrack?.artist || $t('track.unknownArtist') }}</div>
        </div>

        <div class="playback-controls">
          <button class="deck-btn control-btn" :title="$t('player.previous')" :aria-label="$t('player.previous')" @click="playPrevious">
            <AppIcon name="prev" :size="16" />
          </button>
          <button
            class="deck-btn deck-btn--primary control-btn play-btn"
            :title="$t(audioStore.isPlaying ? 'player.pause' : 'player.play')"
            :aria-label="$t(audioStore.isPlaying ? 'player.pause' : 'player.play')"
            @click="togglePlayPause"
          >
            <AppIcon :name="audioStore.isPlaying ? 'pause' : 'play'" :size="18" />
          </button>
          <button class="deck-btn control-btn" :title="$t('player.next')" :aria-label="$t('player.next')" @click="playNext">
            <AppIcon name="next" :size="16" />
          </button>
        </div>

        <div v-if="!isAndroid" class="volume-section">
          <AppIcon name="volume-high" :size="16" />
          <input
            type="range"
            :value="localVolume"
            min="0"
            max="1"
            step="0.01"
            :aria-label="$t('player.volume', { percent: Math.round(localVolume * 100) })"
            @input="setVolume"
          />
        </div>

        <div class="waveform-section">
          <span class="time">{{ formatTime(audioStore.position) }}</span>
          <div class="waveform-wrapper">
            <WaveformBar />
          </div>
          <span class="time">{{ formatTime(audioStore.duration) }}</span>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.player-overlay {
  position: fixed;
  inset: 0;
  z-index: $z-modal;
  display: flex;
  flex-direction: column;
  padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
  background: var(--bg-body);
  color: var(--text);
}

.top-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  flex: none;
  min-height: 45px;
  padding: 8px 10px;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--line);
}

.is-on {
  background: var(--accent);
  color: var(--on-accent);
}

.preset-selector {
  position: relative;
  min-width: 0;
}

.preset-btn {
  max-width: 240px;
}

.preset-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  text-transform: none;
}

.preset-dropdown {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 10;
  width: min(280px, 80vw);
  max-height: 300px;
  overflow-y: auto;
  padding: 4px;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
}

.preset-item {
  display: block;
  width: 100%;
  padding: 6px 8px;
  overflow: hidden;
  font-size: var(--font-size-sm);
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;

  &:hover {
    background: var(--bg-selected);
  }

  &.active {
    color: var(--accent);
    font-weight: 600;
  }
}

.stage {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background: var(--screen);
  cursor: pointer;
}

.cover-wrapper,
.canvas-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
}

// The bottom padding leaves room for the hint under the cover.
.cover-wrapper {
  padding: 16px 16px 30px;
}

.canvas-wrapper {
  background: var(--video-bg);

  .stage-hint {
    color: var(--on-overlay);
  }
}

.big-cover {
  display: block;
  max-width: 100%;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.big-cover-placeholder {
  color: var(--screen-muted);
}

canvas {
  display: block;
  width: 100%;
  height: 100%;
}

.error-msg {
  position: absolute;
  z-index: 5;
  padding: 10px 12px;
  background: var(--screen);
  color: var(--led-hot);
}

.stage-hint {
  position: absolute;
  bottom: 8px;
  width: 100%;
  font: 500 var(--font-size-xs) $font-family-mono;
  text-align: center;
  color: var(--screen-text);
  opacity: 0.7;
  pointer-events: none;
}

.console {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  gap: 8px 16px;
  flex: none;
  padding: 10px;
  background: var(--bg-panel);
  border-top: 1px solid var(--line);
}

.track-details {
  min-width: 0;
  line-height: 1.25;
}

.title {
  font-weight: 600;
  font-size: var(--font-size-lg);
  @include text-ellipsis;
}

.artist {
  font-size: var(--font-size-sm);
  color: var(--muted);
  @include text-ellipsis;
}

.playback-controls {
  display: flex;
  align-items: center;
  gap: 4px;
}

.control-btn {
  width: 44px;
  height: 36px;
  padding: 0;
}

.play-btn {
  width: 62px;
}

.volume-section {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  color: var(--muted);

  input {
    width: min(180px, 100%);
    min-width: 0;
    accent-color: var(--accent);
  }
}

.waveform-section {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  background: var(--screen);
  border-radius: var(--radius-sm);
}

.time {
  flex: none;
  min-width: 40px;
  font: 500 var(--font-size-sm) $font-family-mono;
  color: var(--led);
  text-align: center;
}

.waveform-wrapper {
  flex: 1;
  min-width: 0;
  height: 48px;
  overflow: hidden;
}

// The stage takes the whole window; the bar appears on hover and stays where there is no hover.
.is-fullscreen {
  .console {
    display: none;
  }

  .top-bar {
    position: absolute;
    top: env(safe-area-inset-top);
    right: 0;
    left: 0;
    z-index: 10;
    transition: opacity var(--transition-base);

    @media (hover: hover) {
      opacity: 0;

      &:hover,
      &:focus-within {
        opacity: 1;
      }
    }
  }
}

@media (max-width: $mobile-breakpoint) {
  .console {
    grid-template-columns: minmax(0, 1fr);
    justify-items: center;
    padding: 10px 12px;
  }

  .track-details {
    width: 100%;
    text-align: center;
  }

  .control-btn {
    width: 64px;
    height: 48px;
  }

  .play-btn {
    width: 84px;
  }

  .volume-section {
    width: 100%;

    input {
      width: 100%;
    }
  }

  .waveform-section {
    width: 100%;
  }
}

.overlay-slide-enter-active,
.overlay-slide-leave-active {
  transition: transform var(--transition-slow);
}

.overlay-slide-enter-from,
.overlay-slide-leave-to {
  transform: translateY(100%);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--transition-base);
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
