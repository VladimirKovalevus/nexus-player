<template>
  <div :class="$style.waveformContainer" ref="containerRef" @click="handleSeek">
    <canvas ref="canvasRef" :class="$style.canvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch, onUnmounted } from 'vue';
import { useAudioStore } from '@/stores/audio';
import { invoke } from '@tauri-apps/api/core';

const audioStore = useAudioStore();
const containerRef = ref<HTMLElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);

const BAR_WIDTH = 2;
const BAR_GAP = 1;
const BAR_RADIUS = 1;

let animationFrame: number | null = null;

const handleSeek = (event: MouseEvent) => {
  if (!containerRef.value || audioStore.duration === 0) return;
  
  const rect = containerRef.value.getBoundingClientRect();
  const x = event.clientX - rect.left;
  const percentage = x / rect.width;
  const seekTime = percentage * audioStore.duration;
  
  invoke('seek', { positionSecs: seekTime });
};

const draw = () => {
  const canvas = canvasRef.value;
  const container = containerRef.value;
  if (!canvas || !container) return;

  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const dpr = window.devicePixelRatio || 1;
  const width = container.clientWidth;
  const height = container.clientHeight;

  if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
  }

  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, width, height);

  const duration = audioStore.duration;
  const position = audioStore.position;
  const bufferedRanges = audioStore.bufferedRanges;
  const waveforms = audioStore.waveforms;

  if (duration === 0) {
    animationFrame = requestAnimationFrame(draw);
    return;
  }

  const totalBars = Math.floor(width / (BAR_WIDTH + BAR_GAP));
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
      const dataIdx = Math.floor(((startTime - relevantWaveform.startTime) / (relevantWaveform.endTime - relevantWaveform.startTime)) * relevantWaveform.data.length);
      const val = relevantWaveform.data[Math.max(0, Math.min(dataIdx, relevantWaveform.data.length - 1))];
      amplitude = Math.max(0.1, val * 0.8); 
    }

    const barHeight = Math.max(4, amplitude * height);
    const x = i * (BAR_WIDTH + BAR_GAP);
    const y = (height - barHeight) / 2;

    let color = '#333333';
    
    const isBuffered = bufferedRanges.some(r => 
      (startTime >= r.start && startTime <= r.end) || 
      (endTime >= r.start && endTime <= r.end) ||
      (startTime <= r.start && endTime >= r.end)
    );

    if (startTime < position) {
      color = '#f50';
    } else if (isBuffered) {
      color = '#777777';
    }

    ctx.fillStyle = color;
    
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
  position: relative;
  display: flex;
  align-items: center;
}

.canvas {
  width: 100%;
  height: 100%;
}
</style>
