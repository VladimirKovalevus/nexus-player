<template>
  <div :class="$style.radio">
    <div class="view-header">
      <h1 class="view-title">{{ $t('nav.radio') }}</h1>
    </div>

    <div :class="$style.body">
      <p :class="$style.subtitle">{{ $t('radio.subtitle') }}</p>

      <div :class="$style.display" role="status">
        <span :class="[$style.lamp, { [$style.lampOn]: radioStore.isActive }]" />
        <span>{{ statusText }}</span>
      </div>

      <span class="section-label">{{ $t('radio.source') }}</span>
      <div :class="$style.modes">
        <button
          v-for="mode in MODES"
          :key="mode"
          :class="$style.mode"
          :aria-pressed="radioStore.selectedMode === mode"
          @click="radioStore.selectMode(mode)"
        >
          <span :class="$style.modeName">{{ $t(`radio.modes.${mode}.name`) }}</span>
          <span :class="$style.modeHint">{{ $t(`radio.modes.${mode}.hint`) }}</span>
        </button>
      </div>

      <button
        class="deck-btn"
        :class="[$style.power, { 'deck-btn--primary': !radioStore.isActive }]"
        :disabled="radioStore.isStarting"
        @click="radioStore.toggle"
      >
        <AppIcon :name="radioStore.isActive ? 'pause' : 'play'" :size="14" />
        {{ powerLabel }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import AppIcon from '@/components/common/AppIcon.vue';
import { useRadioStore } from '@/stores/radio';
import type { RadioMode } from '@/types/types';

const { t } = useI18n();
const radioStore = useRadioStore();

const MODES: RadioMode[] = ['tracks', 'playlists'];

const powerLabel = computed(() => {
  if (radioStore.isStarting) return t('radio.starting');
  return t(radioStore.isActive ? 'radio.turnOff' : 'radio.turnOn');
});

const statusText = computed(() =>
  radioStore.activeMode
    ? t('radio.playing', { mode: t(`radio.modes.${radioStore.activeMode}.name`) })
    : t('radio.off'),
);
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.radio {
  height: 100%;
  overflow-y: auto;
}

.body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
}

.subtitle {
  font-size: var(--font-size-sm);
  color: var(--muted);
}

.display {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 42px;
  margin-bottom: 6px;
  padding: 8px 12px;
  background: var(--screen);
  border-radius: var(--radius-sm);
  color: var(--screen-text);
  font: 500 var(--font-size-base) $font-family-mono;
}

.lamp {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--led-dim);
}

.lampOn {
  background: var(--led);
  box-shadow: 0 0 6px var(--led-glow);
}

.modes {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 6px;
}

.mode {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 10px 12px;
  background: var(--key);
  border-radius: var(--radius-sm);
  box-shadow: inset 0 1px 0 var(--key-highlight), 0 1px 0 var(--key-edge);
  text-align: left;

  &:hover {
    filter: brightness(1.12);
  }

  &[aria-pressed='true'] {
    background: var(--bg-selected);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
}

.modeName {
  font-weight: 600;
}

[aria-pressed='true'] > .modeName {
  color: var(--accent);
}

.modeHint {
  font-size: var(--font-size-xs);
  color: var(--muted);
}

.power {
  height: 40px;
  margin-top: 6px;
  font-size: var(--font-size-xs);
}

@media (max-width: $mobile-breakpoint) {
  .power {
    height: 48px;
  }
}
</style>
