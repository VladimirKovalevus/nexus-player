<template>
  <section :class="$style.section">
    <div class="section-head">
      <span class="section-label">{{ title }}</span>
      <span class="view-spacer" />
      <slot name="header-extra" />
    </div>

    <label class="filter-box">
      <AppIcon name="search" :size="14" />
      <input v-model="searchQuery" type="text" :placeholder="$t('common.filterTracks')" autocomplete="off" spellcheck="false" />
      <button v-if="searchQuery" type="button" :aria-label="$t('common.clear')" @click="searchQuery = ''">
        <AppIcon name="close" :size="13" />
      </button>
    </label>

    <div v-if="filteredTracks.length === 0" :class="$style.emptyState">
      <AppIcon :name="emptyIcon" :size="36" />
      <div :class="$style.emptyTitle">{{ searchQuery ? $t('common.nothingFound') : emptyTitle }}</div>
      <slot v-if="!searchQuery" name="empty-extra" />
    </div>

    <div v-else class="track-table">
      <TrackTableHeader />
      <VirtualList
        :items="filteredTracks"
        :item-height="rowHeights.table.value"
        :overscan="8"
        :container-selector="scrollSelector"
      >
        <template #default="{ item: track, index }">
          <MediaRow
            v-memo="[track.id, track.liked, track.state?.type, track.unavailable, activeTrackId === track.id, index, locale]"
            type="track"
            :show-action="showAction"
            :track="track"
            :index="index"
            :active="activeTrackId === track.id"
            @click="$emit('track-click', track, filteredTracks, !!searchQuery)"
            @play="$emit('track-play', track, filteredTracks, !!searchQuery)"
            @refresh="$emit('track-refresh')"
          />
        </template>
      </VirtualList>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import AppIcon from './AppIcon.vue';
import MediaRow from './MediaRow.vue';
import TrackTableHeader from './TrackTableHeader.vue';
import VirtualList from './VirtualList.vue';
import { useRowHeights } from '@/composables/useRowHeights';
import { fuzzyFilterItems } from '@/utils/fuzzy';
import type { TrackDto } from '@/types/types';

const props = withDefaults(
  defineProps<{
    title: string;
    tracks: TrackDto[];
    scrollSelector: string;
    emptyTitle: string;
    activeTrackId?: number | null;
    showAction?: boolean;
    emptyIcon?: string;
  }>(),
  {
    activeTrackId: null,
    showAction: true,
    emptyIcon: 'music-note',
  },
);

defineEmits<{
  'track-click': [track: TrackDto, visibleTracks: TrackDto[], isFiltered: boolean];
  'track-play': [track: TrackDto, visibleTracks: TrackDto[], isFiltered: boolean];
  'track-refresh': [];
}>();

const { locale } = useI18n();
const rowHeights = useRowHeights();
const searchQuery = ref('');

const filteredTracks = computed(() =>
  fuzzyFilterItems(searchQuery.value.trim(), props.tracks, (track) => [track.title, track.artist]),
);
</script>

<style module lang="scss">
.section {
  min-width: 0;
}

.emptyState {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 36px 12px;
  color: var(--muted);
  text-align: center;
}

.emptyTitle {
  font-weight: 600;
  color: var(--text);
}
</style>
