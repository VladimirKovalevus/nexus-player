<template>
  <div>
    <MenuItem v-if="showBack" icon="chevron-left" @click="$emit('back')">{{ $t('common.back') }}</MenuItem>
    <label class="filter-box" :class="$style.filter">
      <input v-model="query" type="text" :placeholder="$t('playlist.searchPlaylist')" autocomplete="off" @click.stop />
    </label>
    <MenuItem
      v-for="playlist in visible"
      :key="playlist.id"
      icon="playlist"
      @click="$emit('pick', playlist.id)"
    >
      {{ playlist.title }}
    </MenuItem>
    <p v-if="visible.length === 0" :class="$style.empty">{{ $t('tree.empty') }}</p>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import MenuItem from './MenuItem.vue';
import { usePlaylistsStore } from '@/stores/playlists';

const props = defineProps<{ excludeId?: number; showBack?: boolean }>();
defineEmits<{ pick: [playlistId: number]; back: [] }>();

const store = usePlaylistsStore();
const query = ref('');

const visible = computed(() => {
  const needle = query.value.trim().toLowerCase();
  return store.playlists
    .filter((playlist) => playlist.id !== props.excludeId)
    .filter((playlist) => playlist.title.toLowerCase().includes(needle));
});

onMounted(() => {
  if (store.playlists.length === 0) store.fetch();
});
</script>

<style module lang="scss">
.filter {
  margin: 8px;
}

.empty {
  padding: 8px 11px;
  color: var(--muted);
  font-size: var(--font-size-sm);
}
</style>
