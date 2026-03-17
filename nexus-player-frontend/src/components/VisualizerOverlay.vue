<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { useAudioStore } from '../stores/audio';
import { invoke } from '@tauri-apps/api/core';
import WaveformBar from './common/WaveformBar.vue';

// @ts-ignore
import butterchurn from 'butterchurn';
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

// Butterchurn state
let audioContext: AudioContext | null = null;
let visualizer: any = null;
let animationFrameId: number | null = null;
let presetInterval: number | null = null;
let resizeObserver: ResizeObserver | null = null;
let onWindowResize: (() => void) | null = null;

// Butterchurn expects raw PCM time-domain data (Uint8Array, 0-255).
// It computes FFT internally. We pass mono data for all three channels.
const BCH_FFT_SIZE = 1024;
const audioLevels = {
  timeByteArray: new Uint8Array(BCH_FFT_SIZE).fill(128),
  timeByteArrayL: new Uint8Array(BCH_FFT_SIZE).fill(128),
  timeByteArrayR: new Uint8Array(BCH_FFT_SIZE).fill(128),
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
  try {
    if (audioStore.isPlaying) await invoke('pause');
    else if (audioStore.isPaused) await invoke('resume');
  } catch (e) {
    console.error('Playback toggle failed:', e);
  }
};

const loadRandomPreset = () => {
  if (!visualizer) return;
  const presets = butterchurnPresets.getPresets();
  const presetNames = Object.keys(presets);
  const randomPreset = presets[presetNames[Math.floor(Math.random() * presetNames.length)]];
  visualizer.loadPreset(randomPreset, 2.7);
  console.log('[Butterchurn] New preset loaded');
};

const initButterchurn = () => {
  if (!canvasRef.value) return;

  try {
    const canvas = canvasRef.value;
    // We still need an AudioContext instance for Butterchurn's internal nodes, 
    // but we WON'T connect it to destination (no hardware access needed)
    audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
    
    visualizer = butterchurn.createVisualizer(audioContext, canvas, {
      width: canvas.clientWidth,
      height: canvas.clientHeight,
      pixelRatio: window.devicePixelRatio || 1,
      textureRatio: 1,
    });

    loadRandomPreset();
    presetInterval = window.setInterval(loadRandomPreset, 30000);

    // Set initial canvas size and observe container for future resizes
    resizeCanvas(canvas.clientWidth, canvas.clientHeight);

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
    console.log('[Butterchurn] Initialized with Mock Analyser');
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

  // 2. Fill PCM time-domain data for butterchurn
  const pcm = audioStore.pcm;
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

  // 3. Render — butterchurn computes FFT internally from PCM
  visualizer.render({ audioLevels });
  
  animationFrameId = requestAnimationFrame(renderLoop);
};

const handleCanvasClick = () => {
  loadRandomPreset();
};

const startVisualizer = async () => {
  if (animationFrameId) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }
  // Always reinit: canvas element may have been replaced by v-if
  visualizer = null;
  let attempts = 0;
  const checkAndInit = async () => {
    if (canvasRef.value) {
      initButterchurn();
      renderLoop();
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
    if (animationFrameId) {
      cancelAnimationFrame(animationFrameId);
      animationFrameId = null;
    }
  }
});

watch(() => props.isOpen, async (val) => {
  if (!val) {
    // Overlay closing: canvas DOM will be destroyed, invalidate visualizer
    if (animationFrameId) {
      cancelAnimationFrame(animationFrameId);
      animationFrameId = null;
    }
    visualizer = null;
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

onUnmounted(() => {
  if (animationFrameId) cancelAnimationFrame(animationFrameId);
  if (presetInterval) clearInterval(presetInterval);
  if (audioContext) audioContext.close();
  resizeObserver?.disconnect();
  if (onWindowResize) window.removeEventListener('resize', onWindowResize);
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
      <button class="fullscreen-btn" @click="toggleFullscreen" title="Полноэкранный режим">
        <span v-if="isFullscreen">⛶</span>
        <span v-else>❐</span>
      </button>
      <button class="close-btn" @click="handleClose">↓</button>
      
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
      </div>

      <div class="volume-section">
        <input
          type="range"
          class="volume-slider"
          :value="localVolume"
          min="0"
          max="1"
          step="0.01"
          @input="setVolume"
          :style="{
            background: `linear-gradient(to right, var(--color-text-primary) ${localVolume * 100}%, var(--color-bg-secondary) ${localVolume * 100}%)`
          }"
        />
        <img src="/icon/volume.svg" class="volume-icon">
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
    background: $color-bg-secondary; border: 1px solid $color-border;
    cursor: pointer; width: 48px; height: 48px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    transition: $transition-fast;
    img { width: 24px; height: 24px; }
    &:hover { transform: scale(1.1); background: $color-bg-hover; }
  }
  .play-btn {
    background: $color-accent-primary; border-color: $color-accent-primary;
    img { width: 32px; height: 32px; margin-left: 2px; filter: brightness(0) invert(1); }
    &:hover { background: $color-accent-hover; border-color: $color-accent-hover; }
  }
}

.waveform-section {
  width: 100%; display: flex; align-items: center; 
  gap: $spacing-md;
  .time { font-size: $font-size-sm; color: $color-text-secondary; min-width: 40px; }
  .waveform-wrapper {
    flex: 1; height: 48px; 
    border-radius: $radius-md; overflow: hidden;
  }
}

.volume-section {
  position: absolute; right: $spacing-lg; bottom: $spacing-xl;
  display: flex; flex-direction: column; align-items: center; width: 32px;
  .volume-icon { width: 20px; height: 20px; opacity: 0.8; }
  .volume-slider {
    -webkit-appearance: none; width: 150px; height: 8px;
    border-radius: $radius-sm; outline: none; cursor: pointer;
    transform: rotate(-90deg); margin-bottom: 70px; padding: 0;
    display: block; background: $color-bg-secondary;
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
