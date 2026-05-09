<template>
  <section :class="$style.section">
    <div :class="$style.sectionHeader">
      <h2 :class="$style.sectionTitle">{{ title }}</h2>
      <div v-if="$slots['header-extra']" :class="$style.headerControls">
        <slot name="header-extra" />
      </div>
    </div>

    <label :class="$style.search">
      <AppIcon name="search" :size="16" />
      <input
        v-model="searchQuery"
        type="text"
        placeholder="Поиск по названию или исполнителю"
        autocomplete="off"
        autocorrect="off"
        autocapitalize="off"
        spellcheck="false"
      />
      <button
        v-if="searchQuery"
        type="button"
        :class="$style.clearButton"
        @click="searchQuery = ''"
      >
        <AppIcon name="close" :size="14" />
      </button>
    </label>

    <slot name="above-list" />

    <div v-if="filteredTracks.length === 0" :class="$style.emptyState">
      <div :class="$style.emptyIcon"><AppIcon :name="emptyIcon" :size="64" /></div>
      <div :class="$style.emptyTitle">{{ searchQuery ? 'Ничего не найдено' : emptyTitle }}</div>
      <slot v-if="!searchQuery" name="empty-extra" />
    </div>

    <VirtualList
      v-else
      :class="$style.scroller"
      :items="filteredTracks"
      :item-height="96"
      :overscan="8"
    >
      <template #default="{ item: track }">
        <MediaCard
          v-memo="[track.id, track.liked, activeTrackId === track.id]"
          type="track"
          :show-action="showAction"
          :track="track"
          :active="activeTrackId === track.id"
          @click="$emit('track-click', track, filteredTracks, !!searchQuery)"
          @play="$emit('track-play', track, filteredTracks, !!searchQuery)"
          @refresh="$emit('track-refresh')"
        />
      </template>
    </VirtualList>
  </section>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import AppIcon from './AppIcon.vue';
import MediaCard from './MediaCard.vue';
import VirtualList from './VirtualList.vue';
import type { TrackDto } from '@/types/types';
import { fuzzyFilterItems } from '@/utils/fuzzy';

const props = withDefaults(
  defineProps<{
    title: string;
    tracks: TrackDto[];
    activeTrackId?: number | null;
    showAction?: boolean;
    emptyIcon?: string;
    emptyTitle?: string;
  }>(),
  {
    activeTrackId: null,
    showAction: true,
    emptyIcon: 'music-note',
    emptyTitle: 'Данных нет',
  },
);

defineEmits<{
  'track-click': [track: TrackDto, visibleTracks: TrackDto[], isFiltered: boolean];
  'track-play': [track: TrackDto, visibleTracks: TrackDto[], isFiltered: boolean];
  'track-refresh': [];
}>();

const searchQuery = ref('');

const filteredTracks = computed(() =>
  fuzzyFilterItems(searchQuery.value.trim(), props.tracks, (track) => [track.title, track.artist]),
);
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.section {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0 $spacing-xl;
}

.scroller {
  flex: 1;
  min-height: 0;
}

.sectionHeader {
  @include flex-between;
  margin-bottom: $spacing-lg;
}

.sectionTitle {
  font-size: $font-size-2xl;
  font-weight: $font-weight-bold;
  color: var(--color-text-primary);
  margin: 0;
}

.headerControls {
  @include flex-center;
  gap: $spacing-sm;
}

.search {
  flex-shrink: 0;
  @include flex-start;
  gap: $spacing-sm;
  padding: $spacing-sm $spacing-md;
  margin-bottom: $spacing-lg;
  background-color: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  color: var(--color-text-secondary);

  input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--color-text-primary);
    font: inherit;

    &::placeholder {
      color: var(--color-text-secondary);
    }
  }
}

.clearButton {
  flex-shrink: 0;
  border: 0;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: 0;
  @include flex-center;

  &:hover {
    color: var(--color-text-primary);
  }
}

.emptyState {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: $spacing-2xl * 2;
  background-color: var(--color-bg-secondary);
  border-radius: $radius-lg;
  border: 2px dashed var(--color-border);
  gap: $spacing-lg;
}

.emptyIcon {
  font-size: 64px;
  opacity: 0.4;
  filter: grayscale(1);
}

.emptyTitle {
  font-size: $font-size-xl;
  font-weight: $font-weight-semibold;
  color: var(--color-text-primary);
}
</style>
