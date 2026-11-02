<template>
  <div :class="$style.playlistDetail">
    <div class="view-header">
      <button class="deck-btn" @click="emit('back')">
        <AppIcon name="arrow-left" :size="13" /> {{ $t('common.back') }}
      </button>
      <template v-if="playlist">
        <AppImage :src="playlist.thumbnail" :alt="playlist.title" :class="$style.art">
          <template #placeholder>
            <span :class="$style.artPlaceholder"><AppIcon name="playlist" :size="15" /></span>
          </template>
        </AppImage>
        <div :class="$style.titleBlock">
          <h1 class="view-title" :class="$style.title" :title="playlist.title">{{ playlist.title }}</h1>
          <span class="view-count">
            {{ $t('playlist.count', { count: items.length }) }}<template v-if="playlist.creator"> · {{ playlist.creator }}</template>
          </span>
        </div>
      </template>
      <span class="view-spacer" />
      <button class="deck-btn deck-btn--primary" @click="handlePlayAll">
        <AppIcon name="play" :size="12" /> {{ $t('playlist.playAll') }}
      </button>
      <button class="deck-btn" @click="handleCachePlaylist">{{ $t('playlist.cache') }}</button>
      <ActionMenu ref="addMenuRef" :label="$t('media.addToPlaylist')" trigger-class="deck-btn">
        <template #trigger>{{ $t('media.addToPlaylist') }}</template>
        <PlaylistPicker :exclude-id="playlistId" @pick="handleAddToPlaylist" />
      </ActionMenu>
      <button class="deck-btn deck-btn--danger" @click="handleDeletePlaylist">{{ $t('playlist.delete') }}</button>
    </div>

    <p v-if="playlist?.description" :class="$style.description">{{ playlist.description }}</p>

    <div v-if="loading" :class="$style.message">{{ $t('common.loading') }}</div>
    <div v-else-if="items.length === 0" :class="$style.message">{{ $t('playlist.empty') }}</div>

    <div v-else class="track-table">
      <TrackTableHeader />
      <div
        v-for="(item, index) in items"
        :key="itemKey(item)"
        draggable="true"
        :class="[$style.draggableItem, draggingIndex === index && $style.dragging]"
        @dragstart="handleDragStart($event, index)"
        @dragover="handleDragOver($event)"
        @dragend="handleDragEnd"
        @drop="handleDrop($event, index)"
      >
        <MediaRow
          v-if="item.itemType === 'track'"
          type="track"
          draggable
          context="playlist"
          :track="item.track"
          :active="audioStore.currentTrack?.id === item.track.id"
          @click="handleTrackPlay(index)"
          @play="handleTrackPlay(index)"
          @refresh="fetchPlaylist"
          @remove="handleRemoveTrack(item.track)"
        />

        <template v-else>
          <MediaRow
            type="playlist"
            draggable
            expandable
            context="playlist"
            :playlist="item.playlist"
            :is-expanded="expanded[item.playlist.id]"
            @click="toggleExpand(item.playlist.id)"
            @toggle-expand="toggleExpand(item.playlist.id)"
            @open="emit('navigate', item.playlist.id)"
            @remove="handleRemovePlaylistFromPlaylist(item.playlist.id)"
          />

          <div v-if="expanded[item.playlist.id] && subPlaylistItems[item.playlist.id]" :class="$style.containerTracks">
            <div
              v-for="(subItem, trackIdx) in subPlaylistItems[item.playlist.id]"
              :key="subItemKey(item.playlist.id, subItem)"
              :class="$style.containerTrack"
              @click="subItem.itemType === 'track' ? handleSubTrackPlay(item.playlist.id, trackIdx) : emit('navigate', subItem.playlist.id)"
            >
              <span v-if="subItem.itemType === 'track'" :class="$style.trackNumber">{{ trackIdx + 1 }}</span>
              <span v-else :class="$style.trackNumber"><AppIcon name="folder" :size="13" /></span>
              <div :class="$style.trackTitle">
                {{ subItem.itemType === 'track' ? subItem.track.title : subItem.playlist.title }}
              </div>
              <div :class="$style.trackArtist">
                {{ subItem.itemType === 'track' ? subItem.track.artist : subItem.playlist.creator }}
              </div>
            </div>
          </div>

          <div
            v-else-if="expanded[item.playlist.id] && subPlaylistLoading[item.playlist.id]"
            :class="$style.message"
          >
            {{ $t('common.loading') }}
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, inject, reactive, watch } from 'vue';
import { usePlaylistActions } from '@/composables/usePlaylistActions';
import { invoke } from '@/utils/providerAuth';
import MediaRow from '@/components/common/MediaRow.vue';
import TrackTableHeader from '@/components/common/TrackTableHeader.vue';
import ActionMenu from '@/components/common/ActionMenu.vue';
import PlaylistPicker from '@/components/common/PlaylistPicker.vue';
import AppIcon from '@/components/common/AppIcon.vue';
import AppImage from '@/components/common/AppImage.vue';
import type { PlaylistDto, PlaylistItemDto, PlaylistWithItemsDto, TrackDto } from '@/types/types';
import { useAudioStore } from '@/stores/audio';

const audioStore = useAudioStore();
const { addToPlaylist, cachePlaylist, deletePlaylist } = usePlaylistActions();
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

watch(
  () => audioStore.lastCachedTrack,
  (info) => {
    if (!info || !info.success) return;
    for (const item of items.value) {
      if (item.itemType === 'track' && item.track.id === info.trackId) {
        item.track.state = { type: 'cached' };
      }
    }
  },
);

const expanded = reactive<Record<number, boolean>>({});
const subPlaylistItems = reactive<Record<number, PlaylistItemDto[]>>({});
const subPlaylistLoading = reactive<Record<number, boolean>>({});

const addMenuRef = ref<InstanceType<typeof ActionMenu> | null>(null);

const handleAddToPlaylist = async (targetPlaylistId: number) => {
  addMenuRef.value?.close();
  await addToPlaylist(targetPlaylistId, playlist.value?.title ?? '', 'add_playlist_to_playlist', {
    parentPlaylistId: targetPlaylistId,
    childPlaylistId: props.playlistId,
  });
};

const handleCachePlaylist = () => {
  if (playlist.value) cachePlaylist(props.playlistId, playlist.value.title);
};

const handleDeletePlaylist = async () => {
  if (await deletePlaylist(props.playlistId, playlist.value?.title ?? '')) emit('back');
};

const draggingIndex = ref<number | null>(null);

const handleDragStart = (event: DragEvent, index: number) => {
  draggingIndex.value = index;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', index.toString());
    // Create a ghost image if needed, but default is fine
  }
};

const handleDragOver = (e: DragEvent) => {
  e.preventDefault();
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = 'move';
  }
};

const handleDragEnd = () => {
  draggingIndex.value = null;
};

const handleDrop = async (event: DragEvent, targetIndex: number) => {
  event.preventDefault();
  
  if (draggingIndex.value === null || draggingIndex.value === targetIndex) {
    draggingIndex.value = null;
    return;
  }

  const sourceIndex = draggingIndex.value;
  draggingIndex.value = null;

  const newItems = [...items.value];
  const [movedItem] = newItems.splice(sourceIndex, 1);
  newItems.splice(targetIndex, 0, movedItem);
  
  // Optimistic UI update
  items.value = newItems;

  // Save to backend
  try {
    const itemIds = newItems.map(item => item.id).filter(id => id !== null && id !== undefined);
    await invoke('reorder_playlist_items', {
      playlistId: props.playlistId,
      itemIds: itemIds,
    });
  } catch (e) {

    console.error('Failed to reorder playlist items:', e);
    // Rollback on error
    await fetchPlaylist();
  }
};

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
  audioStore.startLoading();
  try {
    await invoke('load_playlist_and_play', { playlistId: props.playlistId, startIndex: 0 });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play playlist:', e);
    audioStore.stopLoading();
  }
};

const handleSubTrackPlay = async (subPlaylistId: number, index: number) => {
  audioStore.startLoading();
  try {
    // Each item is one queue row: a nested playlist is a container row.
    await invoke('load_playlist_and_play', { playlistId: subPlaylistId, startIndex: index });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play track from sub-playlist:', e);
    audioStore.stopLoading();
  }
};

const handleTrackPlay = async (index: number) => {
  audioStore.startLoading();
  try {
    // Each item is one queue row: a nested playlist is a container row.
    await invoke('load_playlist_and_play', { playlistId: props.playlistId, startIndex: index });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play track from playlist:', e);
    audioStore.stopLoading();
  }
};

const handleRemoveTrack = async (track: TrackDto) => {
  try {
    await invoke('remove_track_from_playlist', {
      playlistId: props.playlistId,
      trackId: track.id,
    });
    await fetchPlaylist();
  } catch (e) {
    console.error('Failed to remove track from playlist:', e);
  }
};

const handleRemovePlaylistFromPlaylist = async (childPlaylistId: number) => {
  try {
    await invoke('remove_playlist_from_playlist', {
      parentPlaylistId: props.playlistId,
      childPlaylistId,
    });
    await fetchPlaylist();
  } catch (e) {
    console.error('Failed to remove playlist from playlist:', e);
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
  height: 100%;
  overflow-y: auto;
}

.art,
.artPlaceholder {
  display: block;
  flex: none;
  width: 32px;
  height: 32px;
  object-fit: cover;
}

.artPlaceholder {
  @include flex-center;
  background: var(--key);
  color: var(--muted);
}

.titleBlock {
  display: flex;
  flex-direction: column;
  min-width: 0;
  max-width: 340px;
  line-height: 1.2;
}

.title {
  @include text-ellipsis;
}

.description {
  padding: 8px 12px;
  border-bottom: 1px solid var(--line-soft);
  font-size: var(--font-size-sm);
  color: var(--muted);
}

.message {
  padding: 24px 12px;
  text-align: center;
  color: var(--muted);
}

.draggableItem {
  &:hover {
    position: relative;
  }
}

.dragging {
  opacity: 0.4;
}

.containerTracks {
  background: var(--bg-panel);
  border-bottom: 1px solid var(--line);
}

.containerTrack {
  display: grid;
  grid-template-columns: 38px minmax(0, 1.6fr) minmax(0, 1fr);
  align-items: center;
  column-gap: 8px;
  height: var(--row-height);
  padding-right: 12px;
  cursor: pointer;

  &:hover {
    background: var(--bg-selected);
  }
}

.trackNumber {
  display: flex;
  justify-content: flex-end;
  font: 500 var(--font-size-sm) $font-family-mono;
  color: var(--muted);
}

.trackTitle {
  font-weight: 500;
  @include text-ellipsis;
}

.trackArtist {
  color: var(--muted);
  @include text-ellipsis;
}

@media (max-width: $mobile-breakpoint) {
  .titleBlock {
    flex: 1 1 calc(100% - 120px);
    max-width: none;
  }

  .containerTrack {
    grid-template-columns: 30px minmax(0, 1fr);
    grid-auto-rows: min-content;
    align-content: center;
  }

  .trackNumber {
    grid-row: span 2;
  }

  .trackArtist {
    grid-column: 2;
    font-size: var(--font-size-xs);
  }
}
</style>
