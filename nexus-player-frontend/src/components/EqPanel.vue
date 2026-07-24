<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useEqStore, EQ_FREQUENCIES, presetLabelKey } from '@/stores/eq';
import { onClickOutside } from '@vueuse/core';

const { t } = useI18n();
const eqStore = useEqStore();
const panelRef = ref<HTMLElement | null>(null);

const FREQ_LABELS = ['32', '64', '125', '250', '500', '1K', '2K', '4K', '8K', '16K'];

onClickOutside(panelRef, () => {
  if (eqStore.isOpen) {
    eqStore.close();
  }
}, {
  ignore: ['.eq-btn']
});

onMounted(() => {
  if (eqStore.presets.length === 0) {
    eqStore.load();
  }
});

// Debounce per band to avoid flooding Tauri with calls while dragging
const timers = new Array(10).fill(0);
function onBandInput(index: number, event: Event) {
  const value = parseFloat((event.target as HTMLInputElement).value);
  eqStore.settings.gains[index] = value;
  eqStore.settings.presetName = null;
  clearTimeout(timers[index]);
  timers[index] = window.setTimeout(() => {
    eqStore.applySettings({ ...eqStore.settings });
  }, 60);
}

function formatGain(db: number): string {
  if (db === 0) return '0';
  return (db > 0 ? '+' : '') + db.toFixed(1);
}
</script>

<template>
  <Teleport to="body">
    <Transition name="eq-panel">
      <div v-if="eqStore.isOpen" class="eq-overlay">
        <div class="eq-panel" ref="panelRef">
          <!-- Header -->
          <div class="eq-header">
            <h2 class="view-title">{{ $t('player.equalizer') }}</h2>
            <label class="eq-toggle">
              <input
                type="checkbox"
                :checked="eqStore.settings.enabled"
                @change="eqStore.setEnabled(($event.target as HTMLInputElement).checked)"
              />
              <span>{{ $t(eqStore.settings.enabled ? 'common.on' : 'common.off') }}</span>
            </label>
          </div>

          <!-- Bands -->
          <div class="eq-bands" :class="{ disabled: !eqStore.settings.enabled }">
            <div
              v-for="(freq, i) in EQ_FREQUENCIES"
              :key="freq"
              class="eq-band"
            >
              <span class="band-value">{{ formatGain(eqStore.settings.gains[i] ?? 0) }}</span>
              <input
                type="range"
                class="band-slider"
                min="-12"
                max="12"
                step="0.5"
                :value="eqStore.settings.gains[i] ?? 0"
                :disabled="!eqStore.settings.enabled"
                @input="onBandInput(i, $event)"
              />
              <span class="band-freq">{{ FREQ_LABELS[i] }}</span>
            </div>
          </div>

          <!-- Presets -->
          <div class="seg eq-presets">
            <button
              v-for="preset in eqStore.presets"
              :key="preset.name"
              :aria-pressed="eqStore.settings.presetName === preset.name"
              @click="eqStore.applyPreset(preset)"
            >
              {{ t(presetLabelKey(preset.name)) }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped lang="scss">
@import '@/styles/variables.scss';

// Opens under the deck, right below the EQ key.
.eq-overlay {
  position: fixed;
  top: 124px;
  right: 10px;
  z-index: $z-modal;
  max-width: calc(100vw - 20px);
}

.eq-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  width: 520px;
  max-width: 100%;
  padding: 12px;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
}

.eq-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.eq-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  font: 600 var(--font-size-xs) $font-family-mono;
  text-transform: uppercase;

  input {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  // Fits the longest label, so the checkbox stays put when the text changes.
  span {
    width: 4ch;
  }
}

.eq-bands {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 4px;
  padding: 8px 4px;
  background: var(--screen);
  border-radius: var(--radius-sm);

  &.disabled {
    opacity: 0.4;
    pointer-events: none;
  }
}

.eq-band {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.band-value,
.band-freq {
  font: 500 0.625rem $font-family-mono;
  color: var(--screen-muted);
  text-align: center;
  white-space: nowrap;
}

.band-value {
  min-height: 14px;
  color: var(--led);
}

.band-slider {
  writing-mode: vertical-lr;
  direction: rtl;
  appearance: slider-vertical;
  -webkit-appearance: slider-vertical;
  width: 20px;
  height: 110px;
  cursor: pointer;
  accent-color: var(--led);
}

.eq-presets {
  flex-wrap: wrap;
}

.eq-panel-enter-active,
.eq-panel-leave-active {
  transition: opacity var(--transition-base), transform var(--transition-base);
}

.eq-panel-enter-from,
.eq-panel-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

// The deck is taller here, so the panel sits above the tab bar instead.
@media (max-width: $mobile-breakpoint) {
  .eq-overlay {
    top: auto;
    right: 8px;
    bottom: calc(64px + env(safe-area-inset-bottom));
    left: 8px;
    max-width: none;
  }

  .eq-panel {
    width: 100%;
  }
}
</style>
