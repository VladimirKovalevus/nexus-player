<template>
  <button
    :class="['eq-btn', $style.display, { [$style.off]: !eqStore.settings.enabled }]"
    :title="$t('player.equalizer')"
    :aria-label="$t('player.equalizer')"
    @click="eqStore.toggleOpen()"
  >
    <span v-for="(label, i) in FREQ_LABELS" :key="label" :class="$style.band">
      <span :class="$style.fader">
        <i :style="{ top: `${faderTop(eqStore.settings.gains[i] ?? 0)}%` }" />
      </span>
      {{ label }}
    </span>
  </button>
</template>

<script setup lang="ts">
import { useEqStore } from '@/stores/eq';

const eqStore = useEqStore();

const FREQ_LABELS = ['32', '64', '125', '250', '500', '1K', '2K', '4K', '8K', '16K'];
const MAX_GAIN_DB = 12;

const faderTop = (gain: number) => ((MAX_GAIN_DB - gain) / (MAX_GAIN_DB * 2)) * 100;
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.display {
  display: flex;
  align-items: stretch;
  justify-content: space-between;
  gap: 2px;
  width: 100%;
  height: 100%;
  padding: 7px 6px 5px;
  background: var(--screen);
  border-radius: var(--radius-md);
  box-shadow: inset 0 0 0 1px var(--key-edge);

  &:hover {
    box-shadow: inset 0 0 0 1px var(--led-dim);
  }
}

.off {
  opacity: 0.5;
}

.band {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  font: 500 0.5rem $font-family-mono;
  color: var(--screen-muted);
}

.fader {
  flex: 1;
  width: 2px;
  background: var(--screen-track);
  position: relative;

  i {
    position: absolute;
    left: -4px;
    width: 10px;
    height: 5px;
    margin-top: -2px;
    background: var(--screen-text);
    border-radius: 1px;
  }
}
</style>
