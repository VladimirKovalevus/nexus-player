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

    <div v-if="filteredItems.length === 0" :class="$style.emptyState">
      <div :class="$style.emptyIcon"><AppIcon :name="emptyIcon" :size="64" /></div>
      <div :class="$style.emptyTitle">{{ searchQuery ? 'Ничего не найдено' : emptyTitle }}</div>
      <slot v-if="!searchQuery" name="empty-extra" />
    </div>

    <div v-else :class="$style.cardList">
      <VirtualHorizontalList :items="filteredItems" :item-width="180" :height="220">
        <template #default="{ item }">
          <MediaCardCompact
            :type="cardType"
            :title="item.title"
            :artist="item.subtitle"
            :thumbnail="item.thumbnail || undefined"
            :track-count="item.trackCount"
            :playlist-id="cardType === 'playlist' ? item.id : undefined"
            :show-action="showAction"
            @click="$emit('item-click', item)"
            @play="$emit('item-play', item)"
          >
            <template v-if="$slots.menu" #menu="menuProps">
              <slot name="menu" v-bind="{ ...menuProps, item }" />
            </template>
          </MediaCardCompact>
        </template>
      </VirtualHorizontalList>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import AppIcon from './AppIcon.vue';
import MediaCardCompact from './MediaCardCompact.vue';
import VirtualHorizontalList from './VirtualHorizontalList.vue';
import { fuzzyFilterItems } from '@/utils/fuzzy';

export interface MediaCardItem {
  id: number;
  title: string;
  subtitle: string;
  thumbnail?: string | null;
  trackCount?: number;
}

const props = withDefaults(
  defineProps<{
    title: string;
    items: MediaCardItem[];
    cardType?: 'playlist' | 'album';
    showAction?: boolean;
    emptyIcon?: string;
    emptyTitle?: string;
  }>(),
  {
    cardType: 'playlist',
    showAction: false,
    emptyIcon: 'folder',
    emptyTitle: 'Данных нет',
  },
);

defineEmits<{
  'item-click': [item: MediaCardItem];
  'item-play': [item: MediaCardItem];
}>();

const searchQuery = ref('');

const filteredItems = computed(() =>
  fuzzyFilterItems(searchQuery.value.trim(), props.items, (item) => [item.title, item.subtitle]),
);
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.section {
  flex-shrink: 0;
  padding: $spacing-xl $spacing-xl 0;
  margin-bottom: $spacing-2xl;
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

.cardList {
  margin: 0 (-$spacing-xl);
  padding: 0 $spacing-xl;
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
