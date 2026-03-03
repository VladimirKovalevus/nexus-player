<template>
  <div :class="$style.playlistDetail">
    <div :class="$style.header">
      <button :class="$style.backButton" @click="emit('back')">
        &larr; Назад
      </button>

      <div v-if="playlist" :class="$style.playlistInfo">
        <div :class="$style.thumbnail">
          <img
            v-if="playlist.thumbnail"
            :src="playlist.thumbnail"
            :alt="playlist.title"
            :class="$style.thumbnailImage"
          />
          <div v-else :class="$style.thumbnailPlaceholder">&#128194;</div>
        </div>
        <div :class="$style.details">
          <h1 :class="$style.title">{{ playlist.title }}</h1>
          <p v-if="playlist.description" :class="$style.description">{{ playlist.description }}</p>
          <div :class="$style.meta">
            <span>{{ items.length }} items</span>
            <span v-if="playlist.creator"> &bull; {{ playlist.creator }}</span>
          </div>
        </div>
        <button :class="$style.playAllButton" @click="handlePlayAll">
          &#9654; Play All
        </button>
      </div>
    </div>

    <div v-if="loading" :class="$style.loading">Loading...</div>

    <div v-else :class="$style.itemList">
      <template v-for="item in items" :key="itemKey(item)">
        <!-- Track item -->
        <MediaCard
          v-if="item.itemType === 'track'"
          type="track"
          show-action
          :track="item.track"
          @click="handleTrackClick(item.track)"
          @play="handleTrackPlay(item.track)"
          @refresh="fetchPlaylist"
        />

        <!-- Sub-playlist item -->
        <div v-else :class="$style.subPlaylist">
          <div :class="$style.subPlaylistHeader" @click="toggleExpand(item.playlist.id)">
            <div :class="$style.subPlaylistInfo">
              <div :class="$style.subPlaylistIcon">&#128194;</div>
              <div>
                <div :class="$style.subPlaylistTitle">{{ item.playlist.title }}</div>
                <div :class="$style.subPlaylistMeta">
                  {{ item.playlist.trackCount }} items
                  <span v-if="item.playlist.creator"> &bull; {{ item.playlist.creator }}</span>
                </div>
              </div>
            </div>
            <div :class="$style.expandActions">
              <button :class="$style.playSubButton" @click.stop="handlePlaySubPlaylist(item.playlist)">
                &#9654;
              </button>
              <button :class="$style.navigateButton" @click.stop="emit('navigate', item.playlist.id)">
                &#8594;
              </button>
              <span :class="[$style.expandArrow, expanded[item.playlist.id] && $style.expanded]">
                &#9660;
              </span>
            </div>
          </div>

          <div v-if="expanded[item.playlist.id]" :class="$style.subPlaylistTracks">
            <div v-if="subPlaylistLoading[item.playlist.id]" :class="$style.loadingInline">
              Loading...
            </div>
            <template v-else-if="subPlaylistItems[item.playlist.id]">
              <template v-for="subItem in subPlaylistItems[item.playlist.id]" :key="subItemKey(item.playlist.id, subItem)">
                <MediaCard
                  v-if="subItem.itemType === 'track'"
                  type="track"
                  show-action
                  :track="subItem.track"
                  @click="handleTrackClick(subItem.track)"
                  @play="handleTrackPlay(subItem.track)"
                  @refresh="fetchPlaylist"
                />
                <div
                  v-else
                  :class="$style.nestedPlaylistLink"
                  @click="emit('navigate', subItem.playlist.id)"
                >
                  &#128194; {{ subItem.playlist.title }} ({{ subItem.playlist.trackCount }} items) &rarr;
                </div>
              </template>
            </template>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, inject, reactive } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCard from '@/components/common/MediaCard.vue';
import type { PlaylistDto, PlaylistItemDto, PlaylistWithItemsDto, TrackDto } from '@/types/types';

const props = defineProps<{
  playlistId: number;
}>();

const emit = defineEmits<{
  navigate: [playlistId: number];
  back: [];
}>();

const refreshQueue = inject<() => void>('refreshQueue', () => {});

const playlist = ref<PlaylistDto | null>(null);
const items = ref<PlaylistItemDto[]>([]);
const loading = ref(true);

const expanded = reactive<Record<number, boolean>>({});
const subPlaylistItems = reactive<Record<number, PlaylistItemDto[]>>({});
const subPlaylistLoading = reactive<Record<number, boolean>>({});

const itemKey = (item: PlaylistItemDto) => {
  return `${item.itemType}-${item.itemId}`;
};

const subItemKey = (parentId: number, item: PlaylistItemDto) => {
  return `${parentId}-${item.itemType}-${item.itemId}`;
};

const fetchPlaylist = async () => {
  loading.value = true;
  try {
    const result = await invoke<PlaylistWithItemsDto | null>('get_playlist_with_tracks', {
      playlistId: props.playlistId,
    });
    if (result) {
      playlist.value = result.playlist;
      items.value = result.items;
    }
  } catch (e) {
    console.error('Failed to fetch playlist:', e);
  } finally {
    loading.value = false;
  }
};

const toggleExpand = async (childId: number) => {
  expanded[childId] = !expanded[childId];

  if (expanded[childId] && !subPlaylistItems[childId]) {
    subPlaylistLoading[childId] = true;
    try {
      const result = await invoke<PlaylistWithItemsDto | null>('get_playlist_with_tracks', {
        playlistId: childId,
      });
      if (result) {
        subPlaylistItems[childId] = result.items;
      }
    } catch (e) {
      console.error('Failed to fetch sub-playlist:', e);
    } finally {
      subPlaylistLoading[childId] = false;
    }
  }
};

const handlePlayAll = async () => {
  try {
    await invoke('load_playlist_to_queue', { playlistId: props.playlistId });
    await invoke('play_at_index', { index: 0 });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play playlist:', e);
  }
};

const handlePlaySubPlaylist = async (sub: PlaylistDto) => {
  try {
    await invoke('load_playlist_to_queue', { playlistId: sub.id });
    await invoke('play_at_index', { index: 0 });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play sub-playlist:', e);
  }
};

const handleTrackClick = async (track: TrackDto) => {
  try {
    await invoke('add_to_queue', { trackId: track.id });
    refreshQueue();
  } catch (e) {
    console.error('Failed to add track to queue:', e);
  }
};

const handleTrackPlay = async (track: TrackDto) => {
  try {
    await invoke('clear_queue');
    await invoke('add_to_queue', { trackId: track.id });
    await invoke('play_at_index', { index: 0 });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play track:', e);
  }
};

onMounted(() => {
  fetchPlaylist();
});
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.playlistDetail {
  padding: $spacing-xl;
  overflow-y: auto;
  height: 100%;
}

.header {
  margin-bottom: $spacing-xl;
}

.backButton {
  background: transparent;
  border: 1px solid var(--color-border);
  color: var(--color-text-secondary);
  padding: $spacing-xs $spacing-md;
  border-radius: $radius-sm;
  cursor: pointer;
  font-size: $font-size-sm;
  margin-bottom: $spacing-lg;
  transition: all $transition-fast;

  &:hover {
    color: var(--color-text-primary);
    border-color: var(--color-text-secondary);
  }
}

.playlistInfo {
  display: flex;
  align-items: center;
  gap: $spacing-lg;
}

.thumbnail {
  width: 120px;
  height: 120px;
  border-radius: $radius-md;
  overflow: hidden;
  flex-shrink: 0;
  background-color: var(--color-bg-secondary);
}

.thumbnailImage {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.thumbnailPlaceholder {
  width: 100%;
  height: 100%;
  @include flex-center;
  font-size: 48px;
  background: black; 
}

.details {
  flex: 1;
  min-width: 0;
}

.title {
  font-size: $font-size-2xl;
  font-weight: $font-weight-bold;
  color: var(--color-text-primary);
  margin: 0 0 $spacing-xs 0;
}

.description {
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
  margin: 0 0 $spacing-sm 0;
}

.meta {
  font-size: $font-size-sm;
  color: var(--color-text-muted);
}

.playAllButton {
  padding: $spacing-md $spacing-xl;
  font-size: $font-size-base;
  font-weight: $font-weight-semibold;
  background: black;
  color: white;
  border: none;
  border-radius: $radius-lg;
  cursor: pointer;
  transition: all $transition-fast;
  white-space: nowrap;

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 16px rgba(102, 126, 234, 0.4);
  }
}

.loading {
  text-align: center;
  padding: $spacing-2xl;
  color: var(--color-text-muted);
}

.itemList {
  display: flex;
  flex-direction: column;
  gap: $spacing-sm;
}

// Sub-playlist styles
.subPlaylist {
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  overflow: hidden;
  border-left: 3px solid #2196f3;
}

.subPlaylistHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: $spacing-md;
  background-color: var(--color-bg-secondary);
  cursor: pointer;
  transition: background-color $transition-fast;

  &:hover {
    background-color: var(--color-bg-hover);
  }
}

.subPlaylistInfo {
  display: flex;
  align-items: center;
  gap: $spacing-md;
}

.subPlaylistIcon {
  font-size: 24px;
}

.subPlaylistTitle {
  font-size: $font-size-base;
  font-weight: $font-weight-semibold;
  color: var(--color-text-primary);
}

.subPlaylistMeta {
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
}

.expandActions {
  display: flex;
  align-items: center;
  gap: $spacing-sm;
}

.playSubButton,
.navigateButton {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: transparent;
  border: 1px solid var(--color-border);
  color: var(--color-text-secondary);
  @include flex-center;
  cursor: pointer;
  transition: all $transition-fast;

  &:hover {
    background-color: var(--color-accent-primary);
    color: white;
    border-color: var(--color-accent-primary);
  }
}

.expandArrow {
  font-size: $font-size-sm;
  color: var(--color-text-muted);
  transition: transform $transition-fast;
  display: inline-block;

  &.expanded {
    transform: rotate(180deg);
  }
}

.subPlaylistTracks {
  padding: $spacing-sm $spacing-md $spacing-md;
  display: flex;
  flex-direction: column;
  gap: $spacing-xs;
  background-color: var(--color-bg-primary);
  border-top: 1px solid var(--color-border);
}

.loadingInline {
  padding: $spacing-md;
  text-align: center;
  color: var(--color-text-muted);
  font-size: $font-size-sm;
}

.nestedPlaylistLink {
  padding: $spacing-sm $spacing-md;
  color: var(--color-accent-primary);
  font-size: $font-size-sm;
  cursor: pointer;
  border-radius: $radius-sm;
  transition: background-color $transition-fast;

  &:hover {
    background-color: var(--color-bg-hover);
  }
}
</style>
