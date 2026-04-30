<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useEqStore, EQ_FREQUENCIES } from '@/stores/eq';
import { onClickOutside } from '@vueuse/core';

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
            <span class="eq-title">Эквалайзер</span>
            <label class="eq-toggle">
              <input
                type="checkbox"
                :checked="eqStore.settings.enabled"
                @change="eqStore.setEnabled(($event.target as HTMLInputElement).checked)"
              />
              <span class="eq-toggle-label">{{ eqStore.settings.enabled ? 'Вкл' : 'Выкл' }}</span>
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
          <div class="eq-presets">
            <button
              v-for="preset in eqStore.presets"
              :key="preset.name"
              class="preset-btn"
              :class="{ active: eqStore.settings.presetName === preset.name }"
              @click="eqStore.applyPreset(preset)"
            >
              {{ preset.name }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped lang="scss">
@import '@/styles/variables.scss';

.eq-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: flex-end;
  justify-content: flex-end;
  padding-bottom: 108px;
  padding-right: 16px;
  pointer-events: none;
}

.eq-panel {
  pointer-events: all;
  background: var(--color-bg-player);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  padding: 16px;
  width: 520px;
  box-shadow: 0 -8px 32px rgba(0, 0, 0, 0.4);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.eq-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.eq-title {
  font-size: 0.95rem;
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  letter-spacing: 0.05em;
}

.eq-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  font-size: 0.85rem;
  color: var(--color-text-secondary);

  input[type='checkbox'] {
    accent-color: var(--color-accent-primary);
    width: 16px;
    height: 16px;
    cursor: pointer;
  }
}

.eq-toggle-label {
  color: var(--color-text-primary);
}

.eq-bands {
  display: flex;
  gap: 4px;
  align-items: flex-end;
  justify-content: space-between;

  &.disabled {
    opacity: 0.4;
    pointer-events: none;
  }
}

.eq-band {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  flex: 1;
}

.band-value {
  font-size: 0.65rem;
  color: var(--color-text-muted);
  min-height: 14px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

.band-slider {
  writing-mode: vertical-lr;
  direction: rtl;
  appearance: slider-vertical;
  -webkit-appearance: slider-vertical;
  width: 20px;
  height: 100px;
  cursor: pointer;
  accent-color: var(--color-accent-primary);

  &::-webkit-slider-thumb {
    width: 14px;
    height: 14px;
  }
}

.band-freq {
  font-size: 0.6rem;
  color: var(--color-text-muted);
  text-align: center;
}

.eq-presets {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.preset-btn {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  color: var(--color-text-primary);
  font-size: 0.75rem;
  padding: 4px 10px;
  cursor: pointer;
  transition: all 0.15s;

  &:hover {
    background: var(--color-bg-hover);
    color: white;
  }

  &.active {
    background: var(--color-accent-primary);
    border-color: var(--color-accent-primary);
    color: var(--color-bg-player);
  }
}

// Transition
.eq-panel-enter-active,
.eq-panel-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}
.eq-panel-enter-from,
.eq-panel-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
