<template>
  <div :class="$style.waveformContainer" ref="containerRef" @click="handleSeek">
    <canvas ref="canvasRef" :class="$style.canvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { useAudioStore } from '@/stores/audio';
import { invoke } from '@tauri-apps/api/core';

const audioStore = useAudioStore();
const containerRef = ref<HTMLElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);

const BAR_WIDTH = 2;
const BAR_GAP = 1;
const BAR_RADIUS = 1;

let animationFrame: number | null = null;

// Cached values to skip redundant redraws
let cachedColors = { background: '#333333', played: '#ffffff', buffered: '#888888' };
let cachedWidth = 0;
let cachedHeight = 0;
let prevPosition = -1;
let prevDuration = -1;
let prevWaveformRef: unknown = null;
let prevBufferedRef: unknown = null;
let needsRedraw = true; // force first draw

const refreshColors = () => {
  const styles = getComputedStyle(document.documentElement);
  cachedColors = {
    background: styles.getPropertyValue('--waveform-bg').trim() || '#333333',
    played: styles.getPropertyValue('--waveform-played').trim() || '#ffffff',
    buffered: styles.getPropertyValue('--waveform-buffered').trim() || '#888888',
  };
  needsRedraw = true;
};

const getThemeColors = () => cachedColors;

const handleSeek = (event: MouseEvent) => {
  if (!containerRef.value || audioStore.duration === 0) return;
  const rect = containerRef.value.getBoundingClientRect();
  const x = event.clientX - rect.left;
  const percentage = x / rect.width;
  invoke('seek', { positionSecs: Math.max(percentage * audioStore.duration,0) });
};

const draw = () => {
  const canvas = canvasRef.value;
  const container = containerRef.value;
  if (!canvas || !container) return;

  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const duration = audioStore.duration;
  const position = audioStore.position;
  const bufferedRanges = audioStore.bufferedRanges;
  const waveforms = audioStore.waveforms;

  // Skip redraw if nothing changed
  const sizeChanged = cachedWidth !== container.clientWidth || cachedHeight !== container.clientHeight;
  const stateChanged = position !== prevPosition || duration !== prevDuration
    || waveforms !== prevWaveformRef || bufferedRanges !== prevBufferedRef;

  if (!needsRedraw && !sizeChanged && !stateChanged) {
    animationFrame = requestAnimationFrame(draw);
    return;
  }

  needsRedraw = false;
  prevPosition = position;
  prevDuration = duration;
  prevWaveformRef = waveforms;
  prevBufferedRef = bufferedRanges;

  const dpr = window.devicePixelRatio || 1;
  // Use clientWidth/clientHeight (no reflow) — updated by ResizeObserver
  const width = Math.floor(container.clientWidth);
  const height = Math.floor(container.clientHeight);

  cachedWidth = width;
  cachedHeight = height;

  if (width === 0 || height === 0) {
    animationFrame = requestAnimationFrame(draw);
    return;
  }

  // Always keep canvas display size in sync with container (handles both grow and shrink).
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;

  // Reset canvas resolution only when it actually changes (clears content).
  const targetW = Math.round(width * dpr);
  const targetH = Math.round(height * dpr);
  if (canvas.width !== targetW || canvas.height !== targetH) {
    canvas.width = targetW;
    canvas.height = targetH;
  }

  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, width, height);

  const totalBars = Math.floor(width / (BAR_WIDTH + BAR_GAP));
  const colors = getThemeColors();

  if (duration === 0) {
    // Draw placeholder bars using background color
    for (let i = 0; i < totalBars; i++) {
      const barHeight = 4;
      const x = i * (BAR_WIDTH + BAR_GAP);
      const y = (height - barHeight) / 2;
      ctx.fillStyle = colors.background;
      const r = BAR_RADIUS;
      ctx.beginPath();
      ctx.moveTo(x + r, y);
      ctx.lineTo(x + BAR_WIDTH - r, y);
      ctx.quadraticCurveTo(x + BAR_WIDTH, y, x + BAR_WIDTH, y + r);
      ctx.lineTo(x + BAR_WIDTH, y + barHeight - r);
      ctx.quadraticCurveTo(x + BAR_WIDTH, y + barHeight, x + BAR_WIDTH - r, y + barHeight);
      ctx.lineTo(x + r, y + barHeight);
      ctx.quadraticCurveTo(x, y + barHeight, x, y + barHeight - r);
      ctx.lineTo(x, y + r);
      ctx.quadraticCurveTo(x, y, x + r, y);
      ctx.closePath();
      ctx.fill();
    }
    animationFrame = requestAnimationFrame(draw);
    return;
  }

  const timePerBar = duration / totalBars;

  for (let i = 0; i < totalBars; i++) {
    const startTime = i * timePerBar;
    const endTime = (i + 1) * timePerBar;

    let amplitude = 0.1;

    const relevantWaveform = waveforms.find(w =>
      (startTime >= w.startTime && startTime <= w.endTime) ||
      (endTime >= w.startTime && endTime <= w.endTime) ||
      (startTime <= w.startTime && endTime >= w.endTime)
    );

    if (relevantWaveform && relevantWaveform.data.length > 0) {
      const dataIdx = Math.floor(
        ((startTime - relevantWaveform.startTime) /
          (relevantWaveform.endTime - relevantWaveform.startTime)) *
          relevantWaveform.data.length
      );
      const val = relevantWaveform.data[Math.max(0, Math.min(dataIdx, relevantWaveform.data.length - 1))];
      amplitude = Math.max(0.1, val * 0.8);
    }

    const barHeight = Math.max(4, amplitude * height);
    const x = i * (BAR_WIDTH + BAR_GAP);
    const y = (height - barHeight) / 2;

    const isBuffered = bufferedRanges.some(r =>
      (startTime >= r.start && startTime <= r.end) ||
      (endTime >= r.start && endTime <= r.end) ||
      (startTime <= r.start && endTime >= r.end)
    );

    ctx.fillStyle = startTime < position ? colors.played : isBuffered ? colors.buffered : colors.background;

    const r = BAR_RADIUS;
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + BAR_WIDTH - r, y);
    ctx.quadraticCurveTo(x + BAR_WIDTH, y, x + BAR_WIDTH, y + r);
    ctx.lineTo(x + BAR_WIDTH, y + barHeight - r);
    ctx.quadraticCurveTo(x + BAR_WIDTH, y + barHeight, x + BAR_WIDTH - r, y + barHeight);
    ctx.lineTo(x + r, y + barHeight);
    ctx.quadraticCurveTo(x, y + barHeight, x, y + barHeight - r);
    ctx.lineTo(x, y + r);
    ctx.quadraticCurveTo(x, y, x + r, y);
    ctx.closePath();
    ctx.fill();
  }

  animationFrame = requestAnimationFrame(draw);
};

let resizeObserver: ResizeObserver | null = null;

const startLoop = () => {
  if (animationFrame !== null) return;
  draw();
};

const stopLoop = () => {
  if (animationFrame !== null) {
    cancelAnimationFrame(animationFrame);
    animationFrame = null;
  }
};

onMounted(() => {
  refreshColors();

  // Track container size changes without getBoundingClientRect in hot loop
  resizeObserver = new ResizeObserver(() => { needsRedraw = true; });
  if (containerRef.value) resizeObserver.observe(containerRef.value);

  window.addEventListener('theme-changed', refreshColors);

  startLoop();
});

onUnmounted(() => {
  stopLoop();
  resizeObserver?.disconnect();
  window.removeEventListener('theme-changed', refreshColors);
});

// Stop RAF when paused — no CPU burn at idle
watch(() => audioStore.isPlaying, (playing) => {
  if (playing) {
    startLoop();
  } else {
    // One final draw to sync position indicator, then stop
    needsRedraw = true;
    draw();
    stopLoop();
  }
});

// Redraw when data arrives while paused (waveform load, seek, buffer update)
watch(() => [audioStore.waveforms, audioStore.bufferedRanges, audioStore.position], () => {
  if (!audioStore.isPlaying) {
    needsRedraw = true;
    if (animationFrame === null) draw();
  }
});
</script>

<style module lang="scss">
.waveformContainer {
  width: 100%;
  height: 40px;
  cursor: pointer;
  /* position: relative so the absolutely-positioned canvas is contained here */
  position: relative;
  min-width: 0;
}

.canvas {
  /* Out of flow: doesn't affect container's min-content size,
     so the container can freely shrink with its flex parent. */
  position: absolute;
  top: 0;
  left: 0;
}
</style>
