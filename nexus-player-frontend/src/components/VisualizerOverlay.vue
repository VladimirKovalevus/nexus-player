<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { useAudioStore } from '../stores/audio';
import { invoke } from '@tauri-apps/api/core';
import WaveformBar from './common/WaveformBar.vue';

// @ts-ignore
import butterchurn from '../../../../butterchurn/dist/butterchurn.min.js';
// @ts-ignore
import butterchurnPresets from 'butterchurn-presets';

const props = defineProps<{
  isOpen: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();

const audioStore = useAudioStore();
const { playNext, playPrevious } = audioStore;
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
  
  console.log('[Butterchurn] New random preset loaded:', name);
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

    console.log('[Butterchurn] Attempting init with Raw Interface...', { width, height, pixelRatio: window.devicePixelRatio });
    
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

    console.log('[Butterchurn] Initialization successful. GL Version:', visualizer.gl.getParameter(visualizer.gl.VERSION));

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
    console.log('[Butterchurn] Initialized with Raw Interface');
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
const handleClose=()=>{
emit("close");
}

</script>

<template>
  <Transition name="overlay-slide">
    <div v-if="isOpen" class="player-overlay" :class="{ 'is-fullscreen': isFullscreen }">
      <div class="top-controls">
        <button class="fullscreen-btn" @click="toggleFullscreen" title="Полноэкранный режим">
          <span v-if="isFullscreen">⛶</span>
          <span v-else>❐</span>
        </button>

        <div v-if="isCanvasActive" class="visualizer-tools">
          <button class="tool-btn" @click="isCanvasActive = false" title="Вернуться к обложке">
            🖼️
          </button>
          
          <div class="preset-selector">
            <button class="tool-btn preset-btn" @click="togglePresetList" :title="selectedPresetName">
              🎨 {{ isRandomMode ? `Random (${selectedPresetName})` : selectedPresetName }}
            </button>
            
            <div v-if="showPresetList" class="preset-dropdown">
              <div 
                v-for="name in presetList" 
                :key="name"
                class="preset-item"
                :class="{ active: selectedPresetName === name }"
                @click="selectPreset(name)"
              >
                {{ name }}
              </div>
            </div>
          </div>
        </div>

        <button class="close-btn" @click="handleClose">↓</button>
      </div>
      
      <div class="main-content">
        <div class="visualizer-container" @click="handleCanvasClick">
          <Transition name="fade" mode="out-in">
            <div v-if="!isCanvasActive" key="cover" class="cover-wrapper" @click.stop="isCanvasActive = true">
              <img 
                v-if="audioStore.currentTrack?.artworkUrl" 
                :src="audioStore.currentTrack.artworkUrl" 
                class="big-cover"
              />
              <div v-else class="big-cover-placeholder">🎵</div>
            </div>
            <div v-else key="canvas" class="canvas-wrapper">
              <div v-if="butterchurnError" class="error-msg">
                Error: {{ butterchurnError }}
              </div>
              <canvas ref="canvasRef"></canvas>
              <div class="canvas-hint">Milkdrop Active • Click to change style</div>
            </div>
          </Transition>
        </div>

        <div class="track-details">
          <h1 class="title">{{ audioStore.currentTrack?.title || 'No track' }}</h1>
          <h2 class="artist">{{ audioStore.currentTrack?.artist || 'Unknown Artist' }}</h2>
        </div>

        <div class="playback-controls">
          <button class="control-btn" @click="playPrevious">
            <img src="/icon/prev.svg">
          </button>
          <button class="control-btn play-btn" @click="togglePlayPause">
            <img v-if="!audioStore.isPlaying" src="/icon/play.svg">
            <img v-else src="/icon/pause.svg">
          </button>
          <button class="control-btn" @click="playNext">
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

        <div class="volume-section">
          <img src="/icon/volume.svg" class="volume-icon">
          <input
            type="range"
            class="volume-slider"
            :value="localVolume"
            min="0"
            max="1"
            step="0.01"
            @input="setVolume"
            :style="{
              '--volume-fill-color': 'var(--color-accent-hover, #00ffd2)',
              '--volume-bg-color': 'var(--color-bg-secondary, #333)',
              background: `linear-gradient(to right, var(--volume-fill-color) ${localVolume * 100}%, var(--volume-bg-color) ${localVolume * 100}%)`
            }"
          />
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
  top: 0; left: 0;
  width: 100%; height: 100%;
  background-color: $color-bg-player;
  z-index: $z-modal;
  display: flex;
  flex-direction: column;
  padding: $spacing-xl;
  color: $color-text-primary;
}

.close-btn, .fullscreen-btn {
  position: absolute;
  top: $spacing-md;
  background: rgba(0, 0, 0, 0.45);
  border: none;
  color: #fff;
  cursor: pointer;
  z-index: 10;
  border-radius: $radius-md;
  width: 36px; height: 36px;
  display: flex; align-items: center; justify-content: center;
  backdrop-filter: blur(6px);
  opacity: 0.75;
  transition: opacity 0.2s;

  &:hover { opacity: 1; }
}

.close-btn {
  right: $spacing-lg;
  font-size: 1.5rem;
}

.fullscreen-btn {
  left: $spacing-lg;
  font-size: 1.2rem;
}

.top-controls {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 60px;
  z-index: 100;
  pointer-events: none;

  & > * {
    pointer-events: auto;
  }
}

.visualizer-tools {
  position: absolute;
  top: $spacing-md;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  gap: $spacing-sm;
}

.tool-btn {
  background: rgba(0, 0, 0, 0.45);
  border: none;
  color: #fff;
  cursor: pointer;
  border-radius: $radius-md;
  height: 36px;
  padding: 0 $spacing-md;
  display: flex; align-items: center; justify-content: center;
  backdrop-filter: blur(6px);
  opacity: 0.75;
  transition: all 0.2s;
  font-size: $font-size-sm;

  &:hover { opacity: 1; background: rgba(0, 0, 0, 0.6); }
}

.preset-selector {
  position: relative;
}

.preset-btn {
  max-width: 200px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.preset-dropdown {
  position: absolute;
  top: 100%;
  left: 50%;
  transform: translateX(-50%);
  margin-top: $spacing-xs;
  background: rgba(20, 20, 20, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: $radius-md;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
  max-height: 300px;
  overflow-y: auto;
  width: 250px;
  z-index: 100;
  backdrop-filter: blur(10px);

  &::-webkit-scrollbar { width: 6px; }
  &::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.2); border-radius: 3px; }
}

.preset-item {
  padding: $spacing-sm $spacing-md;
  cursor: pointer;
  transition: all 0.2s;
  font-size: $font-size-xs;
  color: rgba(255, 255, 255, 0.7);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;

  &:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  &.active {
    background: var(--color-accent-primary);
    color: #fff;
  }
}

.main-content {
  flex: 1;
  display: flex; flex-direction: column;
  align-items: center; justify-content: center;
  gap: $spacing-lg;
  max-width: 600px; margin: 0 auto; width: 100%;
}

.visualizer-container {
  width: 100%; aspect-ratio: 1;
  position: relative; cursor: pointer;
  box-shadow: $shadow-xl;
  border-radius: $radius-lg;
  overflow: hidden;
  background: #000;
  transition: all 0.3s ease;
}

.player-overlay.is-fullscreen {
  padding: 0;

  .main-content {
    max-width: none;
    gap: 0;
  }

  .visualizer-container {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    border-radius: 0;
    z-index: 1;
    aspect-ratio: auto;
  }

  .track-details,
  .playback-controls,
  .waveform-section,
  .volume-section {
    display: none;
  }

  .close-btn, .fullscreen-btn {
    z-index: 100;
  }
}

.cover-wrapper, .canvas-wrapper {
  width: 100%; height: 100%;
  display: flex; align-items: center; justify-content: center;
}

.big-cover { width: 100%; height: 100%; object-fit: cover; }
.big-cover-placeholder { font-size: 8rem; opacity: 0.3; }

canvas { width: 100%; height: 100%; display: block; }

.error-msg {
  position: absolute; color: #ff4444; background: rgba(0,0,0,0.8);
  padding: 1rem; z-index: 5;
}

.canvas-hint {
  position: absolute; bottom: $spacing-md;
  width: 100%; text-align: center; font-size: $font-size-sm;
  color: #fff; text-shadow: 0 0 10px #000;
  opacity: 0.6; pointer-events: none;
}

.track-details {
  text-align: center; width: 100%;
  .title {
    font-size: $font-size-lg; color: $color-bg-primary;
    margin: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .artist {
    font-size: $font-size-sm; color: $color-bg-primary;
    margin: $spacing-sm 0 0 0;
  }
}

.playback-controls {
  display: flex; align-items: center; gap: $spacing-xl;
  .control-btn {
    background: none; border: 1px solid transparent;
    cursor: pointer; width: 48px; height: 48px; border-radius: 50%;
    padding: 0 !important;
    display: flex; align-items: center; justify-content: center;
    transition: $transition-fast;
    img { width: 24px; height: 24px; }
    &:hover { transform: scale(1.1); background: var(--color-bg-hover); }
  }
  .play-btn {
    background: none; border: 2px solid var(--color-accent-primary);
    img { width: 32px; height: 32px; margin-left: 2px; }
    &:hover { background: var(--color-bg-hover); border-color: var(--color-accent-hover); }
  }
}

.waveform-section {
  width: 100%; display: flex; align-items: center; 
  gap: $spacing-md;
  .time { font-size: var(--font-size-lg); color: var(--player-time-color, white); min-width: 40px; }
  .waveform-wrapper {
    flex: 1; height: 48px; 
    border-radius: $radius-md; overflow: hidden;
  }
}

.volume-section {
  width: 100%;
  display: flex; align-items: center; gap: $spacing-md;
  margin-top: $spacing-lg;
  .volume-icon { width: 20px; height: 20px; opacity: 0.8; flex-shrink: 0; }
  .volume-slider {
    -webkit-appearance: none; flex: 1; height: 8px;
    border-radius: $radius-sm; outline: none; cursor: pointer;
    background: $color-bg-secondary;
    padding: 0; margin: 0;
    &::-webkit-slider-thumb {
      -webkit-appearance: none; width: 20px; height: 20px;
      background: $color-text-primary; border-radius: 50%; box-shadow: $shadow-md;
    }
  }
}

.overlay-slide-enter-active, .overlay-slide-leave-active { transition: transform $transition-slow; }
.overlay-slide-enter-from, .overlay-slide-leave-to { transform: translateY(100%); }
.fade-enter-active, .fade-leave-active { transition: opacity $transition-base; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>
