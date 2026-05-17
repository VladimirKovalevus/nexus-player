<template>
  <section :class="[$style.section, { [$style.mobileOnly]: mobileOnly }]">
    <div class="section-head">
      <span class="section-label">{{ title }}</span>
    </div>

    <label v-if="items.length > 0" class="filter-box">
      <AppIcon name="search" :size="14" />
      <input v-model="searchQuery" type="text" :placeholder="$t('common.filter')" autocomplete="off" spellcheck="false" />
      <button v-if="searchQuery" type="button" :aria-label="$t('common.clear')" @click="searchQuery = ''">
        <AppIcon name="close" :size="13" />
      </button>
    </label>

    <p v-if="filteredItems.length === 0" :class="$style.empty">
      {{ searchQuery ? $t('common.nothingFound') : emptyTitle }}
    </p>

    <div v-else :class="$style.grid">
      <MediaTile
        v-for="item in filteredItems"
        :key="item.id"
        :title="item.title"
        :subtitle="item.subtitle"
        :thumbnail="item.thumbnail"
        :placeholder-icon="placeholderIcon"
        @click="$emit('item-click', item)"
        @play="$emit('item-play', item)"
      >
        <template v-if="$slots.menu" #menu="menuProps">
          <slot name="menu" v-bind="{ ...menuProps, item }" />
        </template>
      </MediaTile>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import AppIcon from './AppIcon.vue';
import MediaTile from './MediaTile.vue';
import { fuzzyFilterItems } from '@/utils/fuzzy';

export interface MediaTileItem {
  id: number;
  title: string;
  subtitle: string;
  thumbnail?: string | null;
}

const props = defineProps<{
  title: string;
  items: MediaTileItem[];
  emptyTitle: string;
  placeholderIcon: string;
  /** The playlist tree already lists these on wider screens. */
  mobileOnly?: boolean;
}>();

defineEmits<{ 'item-click': [item: MediaTileItem]; 'item-play': [item: MediaTileItem] }>();

const searchQuery = ref('');
const filteredItems = computed(() =>
  fuzzyFilterItems(searchQuery.value.trim(), props.items, (item) => [item.title, item.subtitle]),
);
</script>

<style module lang="scss">
.section {
  flex: none;
}

.mobileOnly {
  display: none;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 1px;
  background: var(--line-soft);
  border-block: 1px solid var(--line);
}

.empty {
  padding: 4px 12px 12px;
  color: var(--muted);
  font-size: var(--font-size-sm);
}

@media (max-width: 640px) {
  .mobileOnly {
    display: block;
  }

  .grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
