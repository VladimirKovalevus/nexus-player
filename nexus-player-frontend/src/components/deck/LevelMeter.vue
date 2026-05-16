<template>
  <div :class="$style.meter" aria-hidden="true">
    <i
      v-for="n in SEGMENTS"
      :key="n"
      :class="{
        [$style.ok]: n <= lit && n <= OK_SEGMENTS,
        [$style.warn]: n <= lit && n > OK_SEGMENTS && n <= WARN_SEGMENTS,
        [$style.hot]: n <= lit && n > WARN_SEGMENTS,
      }"
    />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useAudioStore } from '@/stores/audio';

const SEGMENTS = 14;
const OK_SEGMENTS = 9;
const WARN_SEGMENTS = 12;
const SAMPLE_STRIDE = 8; // Level does not need every sample of the 1024-sample window.
const GAIN = 2.4; // Typical music RMS sits well below full scale.

const audioStore = useAudioStore();

// The store receives mono u8 PCM centered at 128 with each position update while playing.
const lit = computed(() => {
  const pcm = audioStore.pcm;
  if (!audioStore.isPlaying || !pcm || pcm.length === 0) return 0;

  let sum = 0;
  let count = 0;
  for (let i = 0; i < pcm.length; i += SAMPLE_STRIDE) {
    const sample = (pcm[i] - 128) / 127;
    sum += sample * sample;
    count++;
  }
  const rms = Math.sqrt(sum / count);
  return Math.min(SEGMENTS, Math.round(rms * GAIN * SEGMENTS));
});
</script>

<style module lang="scss">
.meter {
  height: 100%;
  display: flex;
  flex-direction: column-reverse;
  gap: 2px;
  padding: 7px 6px;
  background: var(--screen);
  border-radius: var(--radius-md);
  box-shadow: inset 0 0 0 1px var(--key-edge);

  i {
    flex: 1;
    min-height: 0;
    background: var(--screen-track);
    border-radius: 1px;
    opacity: 0.45;
  }

  .ok,
  .warn,
  .hot {
    opacity: 1;
  }

  .ok {
    background: var(--led-ok);
  }

  .warn {
    background: var(--led);
  }

  .hot {
    background: var(--led-hot);
  }
}
</style>
