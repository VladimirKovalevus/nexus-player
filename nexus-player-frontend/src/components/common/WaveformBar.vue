<template>
  <div :class="$style.waveformContainer" ref="containerRef" @click="handleSeek">
    <canvas ref="canvasRef" :class="$style.canvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { useAudioStore } from '@/stores/audio';
import { invoke } from '@tauri-apps/api/core';

const audioStore = useAudioStore();
const containerRef = ref<HTMLElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);

const BAR_WIDTH = 2;
const BAR_GAP = 1;
const BAR_RADIUS = 1;

let animationFrame: number | null = null;

const getThemeColors = () => {
  const styles = getComputedStyle(document.documentElement);
  return {
    background: styles.getPropertyValue('--waveform-bg').trim() || '#333333',
    played: styles.getPropertyValue('--waveform-played').trim() || '#ffffff',
    buffered: styles.getPropertyValue('--waveform-buffered').trim() || '#888888',
  };
};

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

  const dpr = window.devicePixelRatio || 1;
  // getBoundingClientRect is always the container's actual layout rect,
  // unaffected by the canvas child's explicit style width.
  const rect = container.getBoundingClientRect();
  const width = Math.floor(rect.width);
  const height = Math.floor(rect.height);

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

  const duration = audioStore.duration;
  const position = audioStore.position;
  const bufferedRanges = audioStore.bufferedRanges;
  const waveforms = audioStore.waveforms;

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

onMounted(() => {
  draw();
});

onUnmounted(() => {
  if (animationFrame) cancelAnimationFrame(animationFrame);
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
