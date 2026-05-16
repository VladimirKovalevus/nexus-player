<template>
  <div :class="$style.tree">
    <div :class="$style.head">
      <span>{{ $t('tree.title') }}</span>
      <button :class="$style.add" @click="$emit('create')">+ {{ $t('tree.add') }}</button>
    </div>
    <button
      v-for="playlist in store.playlists"
      :key="playlist.id"
      :class="[$style.item, { [$style.active]: selectedId === playlist.id }]"
      :title="playlist.title"
      @click="$emit('select', playlist.id)"
    >
      <AppImage :src="playlist.thumbnail ?? undefined" :alt="playlist.title" :class="$style.art">
        <template #placeholder>
          <span :class="$style.placeholder"><AppIcon name="playlist" :size="15" /></span>
        </template>
      </AppImage>
      <span :class="$style.name">{{ playlist.title }}</span>
      <span :class="$style.count">{{ playlist.trackCount }}</span>
    </button>
    <p v-if="store.playlists.length === 0" :class="$style.empty">{{ $t('tree.empty') }}</p>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue';
import AppIcon from '@/components/common/AppIcon.vue';
import AppImage from '@/components/common/AppImage.vue';
import { usePlaylistsStore } from '@/stores/playlists';

defineProps<{ selectedId: number | null }>();
defineEmits<{ select: [id: number]; create: [] }>();

const store = usePlaylistsStore();

onMounted(store.fetch);
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.tree {
  padding: 10px 0;
}

.head {
  @include flex-between;
  gap: 8px;
  padding: 0 12px 8px;
  font: 600 0.625rem $font-family-mono;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--muted);
}

.add {
  color: var(--accent);
  font: inherit;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  white-space: nowrap;
}

.item {
  width: 100%;
  display: grid;
  grid-template-columns: 30px minmax(0, 1fr) auto;
  align-items: center;
  gap: 9px;
  height: 38px;
  padding: 0 12px;
  text-align: left;

  &:hover {
    background: var(--bg-selected);
  }
}

.active {
  background: var(--bg-selected);
  color: var(--accent);
  font-weight: 600;
}

.art {
  display: block;
  width: 30px;
  height: 30px;
  object-fit: cover;
  overflow: hidden;
  background: var(--key);
}

.placeholder {
  @include flex-center;
  width: 30px;
  height: 30px;
  color: var(--muted);
}

.name {
  @include text-ellipsis;
}

.count {
  font: 500 var(--font-size-xs) $font-family-mono;
  color: var(--muted);
}

.empty {
  padding: 8px 12px;
  color: var(--muted);
  font-size: var(--font-size-sm);
}
</style>
