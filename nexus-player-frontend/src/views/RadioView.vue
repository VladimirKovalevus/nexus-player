<template>
  <div :class="$style.radioView">
    <header :class="$style.header">
      <h1 :class="$style.title">
        <AppIcon name="radio" :size="28" />
        Радио
      </h1>
      <p :class="$style.subtitle">
        Бесконечный поток из вашей медиатеки. Порядок составляется один раз при
        включении и больше не меняется.
      </p>
    </header>

    <section :class="$style.panel">
      <div :class="$style.field">
        <label :class="$style.label">Источник</label>
        <div :class="$style.modeGrid">
          <button
            v-for="option in MODE_OPTIONS"
            :key="option.mode"
            :class="[$style.modeButton, radioStore.selectedMode === option.mode && $style.active]"
            @click="radioStore.selectMode(option.mode)"
          >
            <span :class="$style.modeName">{{ option.name }}</span>
            <span :class="$style.modeHint">{{ option.hint }}</span>
          </button>
        </div>
      </div>

      <button
        :class="[$style.powerButton, radioStore.isActive && $style.on]"
        :disabled="radioStore.isStarting"
        @click="handleToggle"
      >
        <AppIcon :name="radioStore.isActive ? 'pause' : 'play'" :size="20" />
        {{ powerLabel }}
      </button>

      <p v-if="radioStore.isActive" :class="$style.status">
        Сейчас играет радио: <strong>{{ activeModeName }}</strong>
      </p>
      <p v-else :class="$style.status">Радио выключено.</p>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import AppIcon from '@/components/common/AppIcon.vue';
import { useRadioStore } from '@/stores/radio';
import type { RadioMode } from '@/types/types';

const radioStore = useRadioStore();

const MODE_OPTIONS: Array<{ mode: RadioMode; name: string; hint: string }> = [
  {
    mode: 'tracks',
    name: 'На основе треков',
    hint: 'Все треки медиатеки в случайном порядке',
  },
  {
    mode: 'playlists',
    name: 'На основе плейлистов',
    hint: 'Плейлисты целиком, между ними — треки вне плейлистов',
  },
];

const powerLabel = computed(() => {
  if (radioStore.isStarting) return 'Составляем очередь...';
  return radioStore.isActive ? 'Выключить радио' : 'Включить радио';
});

const activeModeName = computed(
  () => MODE_OPTIONS.find((o) => o.mode === radioStore.activeMode)?.name ?? '',
);

const handleToggle = () => radioStore.toggle();
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.radioView {
  padding: $spacing-xl;
  height: 100%;
  overflow-y: auto;
}

.header {
  margin-bottom: $spacing-2xl;
}

.title {
  font-size: $font-size-3xl;
  font-weight: $font-weight-bold;
  color: var(--color-text-primary);
  display: flex;
  align-items: center;
  gap: $spacing-md;
}

.subtitle {
  margin-top: $spacing-sm;
  max-width: 60ch;
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
}

.panel {
  max-width: 640px;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-lg;
  padding: $spacing-xl;
  display: flex;
  flex-direction: column;
  gap: $spacing-xl;
}

.field {
  display: flex;
  flex-direction: column;
  gap: $spacing-md;
}

.label {
  font-size: $font-size-sm;
  font-weight: $font-weight-semibold;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.modeGrid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: $spacing-md;
}

.modeButton {
  display: flex;
  flex-direction: column;
  gap: $spacing-xs;
  text-align: left;
  padding: $spacing-md;
  background: transparent;
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  color: var(--color-text-primary);
  cursor: pointer;
  transition: all $transition-fast;

  &:hover {
    background: var(--color-bg-hover);
  }

  &.active {
    border-color: var(--color-accent-primary);
    box-shadow: 0 0 0 1px var(--color-accent-primary);
  }
}

.modeName {
  font-size: $font-size-base;
  font-weight: $font-weight-semibold;
}

.modeHint {
  font-size: $font-size-xs;
  color: var(--color-text-secondary);
}

.powerButton {
  @include flex-center;
  gap: $spacing-md;
  padding: $spacing-md $spacing-lg;
  font-size: $font-size-base;
  font-weight: $font-weight-semibold;
  border: 1px solid var(--color-accent-primary);
  border-radius: $radius-md;
  background: transparent;
  color: var(--color-accent-primary);
  cursor: pointer;
  transition: all $transition-fast;

  &:hover:not(:disabled) {
    background: var(--color-bg-hover);
  }

  &:disabled {
    opacity: 0.6;
    cursor: default;
  }

  &.on {
    background: var(--color-accent-primary);
    color: var(--color-bg-primary);
  }
}

.status {
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
}
</style>
