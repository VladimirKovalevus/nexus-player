<template>
  <div :class="$style.playlistDetail">
    <div :class="$style.header">
      <div :class="$style.headerActions">
        <button :class="$style.backButton" @click="emit('back')" title="Назад">
          <AppIcon name="arrow-left" :size="18" />
        </button>
        <div :class="$style.dropdownWrapper">
          <button ref="actionButtonRef" :class="$style.actionButton" @click.stop="toggleDropdown">
            <AppIcon name="more-horizontal" :size="18" />
          </button>

          <Teleport to="body">
            <div v-if="dropdownOpen" ref="dropdownPortalRef" :class="$style.dropdown" :style="dropdownStyle" @click.stop @wheel.stop>
              <div v-if="!showPlaylistSubmenu">
                <button :class="$style.dropdownItem" @click="handleShowPlaylistSubmenu">
                  <span :class="$style.itemIcon"><AppIcon name="playlist" :size="16" /></span>
                  <span :class="$style.itemLabel">Добавить в плейлист</span>
                  <AppIcon name="chevron-right" :size="14" :class="$style.itemArrow" />
                </button>
                <button :class="$style.dropdownItem" @click="handleCachePlaylist">
                  <span :class="$style.itemIcon"><AppIcon name="download-tray" :size="16" /></span>
                  <span :class="$style.itemLabel">Кэшировать плейлист</span>
                </button>
                <div :class="$style.divider"></div>
                <button :class="[$style.dropdownItem, $style.dangerItem]" @click="handleDeletePlaylist">
                  <span :class="$style.itemIcon"><AppIcon name="trash" :size="16" /></span>
                  <span :class="$style.itemLabel">Удалить плейлист</span>
                </button>
              </div>

              <div v-else :class="$style.submenu">
                <button :class="$style.dropdownItem" @click="showPlaylistSubmenu = false">
                  <span :class="$style.itemIcon"><AppIcon name="chevron-left" :size="14" /></span>
                  <span :class="$style.itemLabel">Назад</span>
                </button>
                <div :class="$style.divider"></div>
                <div :class="$style.searchWrapper">
                  <input
                    v-model="playlistFilter"
                    type="text"
                    placeholder="Поиск плейлиста..."
                    :class="$style.searchInput"
                    @click.stop
                  />
                </div>
                <button
                  v-for="pl in filteredPlaylists"
                  :key="pl.id"
                  :class="$style.dropdownItem"
                  @click="handleAddToPlaylist(pl.id)"
                >
                  <span :class="$style.itemIcon"><AppIcon name="playlist" :size="16" /></span>
                  <span :class="$style.itemLabel">{{ pl.title }}</span>
                </button>
                <div v-if="playlists.length === 0" :class="$style.emptyPlaylist">
                  Нет плейлистов
                </div>
              </div>
            </div>
          </Teleport>
        </div>
      </div>

      <div v-if="playlist" :class="$style.playlistInfo">
        <div :class="$style.thumbnail">
          <AppImage :src="playlist.thumbnail" :alt="playlist.title" :class="$style.thumbnailImage">
            <template #placeholder>
              <div :class="$style.thumbnailPlaceholder">&#128194;</div>
            </template>
          </AppImage>
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
        <!-- Track item -->
        <MediaCard
          v-if="item.itemType === 'track'"
          type="track"
          show-action
          context="playlist"
          :track="item.track"
          :active="audioStore.currentTrack?.id === item.track.id"
          @click="handleTrackClick(item.track)"
          @play="handleTrackPlay(index)"
          @refresh="fetchPlaylist"
          @remove="handleRemoveTrack(item.track)"
        />

        <!-- Sub-playlist item -->
        <div v-else :class="$style.containerWrapper">
          <MediaCard
            type="playlist"
            :playlist="item.playlist"
            show-action
            context="playlist"
            container-mode
            :is-expanded="expanded[item.playlist.id]"
            @click="toggleExpand(item.playlist.id)"
            @toggle-expand="toggleExpand(item.playlist.id)"
            @remove="handleRemovePlaylistFromPlaylist(item.playlist.id)"
          />

          <!-- Expanded tracks inside sub-playlist -->
          <div
            v-if="expanded[item.playlist.id] && subPlaylistItems[item.playlist.id]"
            :class="$style.containerTracks"
          >
            <div
              v-for="(subItem, trackIdx) in subPlaylistItems[item.playlist.id]"
              :key="subItemKey(item.playlist.id, subItem)"
              :class="$style.containerTrack"
              @click="subItem.itemType === 'track' ? handleSubTrackPlay(item.playlist.id, trackIdx) : emit('navigate', subItem.playlist.id)"
            >
              <span v-if="subItem.itemType === 'track'" :class="$style.trackNumber">{{ trackIdx + 1 }}</span>
              <span v-else :class="$style.trackNumber"><AppIcon name="folder" :size="14" /></span>
              
              <div :class="$style.trackInfo">
                <div :class="$style.trackTitle">
                  {{ subItem.itemType === 'track' ? subItem.track.title : subItem.playlist.title }}
                </div>
                <div :class="$style.trackArtist">
                  {{ subItem.itemType === 'track' ? subItem.track.artist : subItem.playlist.creator }}
                </div>
              </div>

              <!-- Actions for sub-items could be added here if needed -->
            </div>
          </div>

          <!-- Loading state for sub-playlist -->
          <div
            v-else-if="expanded[item.playlist.id] && subPlaylistLoading[item.playlist.id]"
            :class="$style.containerLoading"
          >
            Loading tracks...
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, inject, reactive, nextTick, computed, watch } from 'vue';
import { invoke } from '@/utils/providerAuth';
import MediaCard from '@/components/common/MediaCard.vue';
import AppIcon from '@/components/common/AppIcon.vue';
import AppImage from '@/components/common/AppImage.vue';
import type { PlaylistDto, PlaylistItemDto, PlaylistWithItemsDto, TrackDto } from '@/types/types';
import { useAudioStore } from '@/stores/audio';
import { useNotificationStore } from '@/stores/notifications';

const audioStore = useAudioStore();
const notifications = useNotificationStore();
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

const dropdownOpen = ref(false);
const showPlaylistSubmenu = ref(false);
const playlists = ref<Array<{ id: number; title: string }>>([]);
const playlistFilter = ref('');
const actionButtonRef = ref<HTMLElement | null>(null);
const dropdownPortalRef = ref<HTMLElement | null>(null);
const dropdownPos = ref({ top: 0, right: 0 });

const dropdownStyle = computed(() => ({
  top: `${dropdownPos.value.top}px`,
  right: `${dropdownPos.value.right}px`,
}));

const toggleDropdown = async () => {
  dropdownOpen.value = !dropdownOpen.value;
  showPlaylistSubmenu.value = false;

  if (dropdownOpen.value) {
    await nextTick();
    if (actionButtonRef.value) {
      const rect = actionButtonRef.value.getBoundingClientRect();
      const viewportHeight = window.innerHeight;
      
      let top = rect.bottom + 4;
      let right = window.innerWidth - rect.right;
      
      if (dropdownPortalRef.value) {
        const dropdownHeight = dropdownPortalRef.value.offsetHeight;
        if (top + dropdownHeight > viewportHeight) {
          top = rect.top - dropdownHeight - 4;
        }
      }
      
      dropdownPos.value = { top, right };
    }
    if (playlists.value.length === 0) {
      await fetchPlaylists();
    }
    document.addEventListener('mousedown', handleDocumentClick);
  } else {
    document.removeEventListener('mousedown', handleDocumentClick);
  }
};

const fetchPlaylists = async () => {
  try {
    const allPlaylists = await invoke<Array<{ id: number; title: string }>>('get_all_playlists');
    playlists.value = allPlaylists;
  } catch (e) {
    console.error('Failed to fetch playlists:', e);
  }
};

const handleShowPlaylistSubmenu = () => {
  showPlaylistSubmenu.value = true;
  console.log(items.value,'ITEMS')
};

const filteredPlaylists = computed(() => {
  const query = playlistFilter.value.toLowerCase();
  return playlists.value
    .filter(p => p.id !== props.playlistId)
    .filter(p => p.title.toLowerCase().includes(query));
});

const handleAddToPlaylist = async (targetPlaylistId: number) => {
  try {
    await invoke('add_playlist_to_playlist', {
      parentPlaylistId: targetPlaylistId,
      childPlaylistId: props.playlistId,
    });
    closeDropdown();
  } catch (e) {
    console.error('Failed to add playlist to playlist:', e);
  }
};

const closeDropdown = () => {
  dropdownOpen.value = false;
  showPlaylistSubmenu.value = false;
  playlistFilter.value = '';
  document.removeEventListener('mousedown', handleDocumentClick);
};

const handleDocumentClick = (e: MouseEvent) => {
  const target = e.target as Node;
  if (dropdownPortalRef.value?.contains(target)) return;
  if (actionButtonRef.value?.contains(target)) return;
  closeDropdown();
};

const handleCachePlaylist = async () => {
  if (!playlist.value) return;
  closeDropdown();

  const title = playlist.value.title;
  notifications.addNotification(`Начинаю кэширование плейлиста "${title}"`, 'info');

  try {
    let successCount = 0;
    let failCount = 0;

    for (const item of items.value) {
      if (item.itemType === 'track') {
        try {
          await invoke('save_track_to_file', { track: item.track });
          successCount++;
        } catch (e) {
          console.error(`Failed to cache track ${item.track.title}:`, e);
          failCount++;
        }
      }
    }

    if (failCount === 0) {
      notifications.addNotification(
        `Плейлист "${title}" поставлен на загрузку (${successCount} треков)`,
        'success',
      );
    } else {
      notifications.addNotification(
        `Загрузка "${title}": ${successCount} ок, ${failCount} ошибок`,
        'info',
      );
    }
  } catch (e) {
    console.error('Failed to cache playlist:', e);
    notifications.addNotification(`Ошибка при кэшировании плейлиста: ${e}`, 'error');
  }
};

const handleDeletePlaylist = async () => {
  if (!confirm(`Удалить плейлист "${playlist.value?.title}"?`)) return;
  
  try {
    await invoke('delete_playlist', { playlistId: props.playlistId });
    closeDropdown();
    emit('back');
  } catch (e) {
    console.error('Failed to delete playlist:', e);
  }
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

const handleTrackClick = async (track: TrackDto) => {
  try {
    await invoke('add_to_queue', { trackId: track.id });
    refreshQueue();
  } catch (e) {
    console.error('Failed to add track to queue:', e);
  }
};

const handleSubTrackPlay = async (subPlaylistId: number, index: number) => {
  audioStore.startLoading();
  try {
    const startIndex = subPlaylistItems[subPlaylistId]
      .slice(0, index)
      .filter((subItem) => subItem.itemType === 'track').length;
    await invoke('load_playlist_and_play', { playlistId: subPlaylistId, startIndex });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play track from sub-playlist:', e);
    audioStore.stopLoading();
  }
};

const handleTrackPlay = async (index: number) => {
  audioStore.startLoading();
  try {
    let trackIndex = 0;
    for (let i = 0; i < index; i++) {
      if (items.value[i].itemType === 'track') {
        trackIndex++;
      }
    }
    await invoke('load_playlist_and_play', { playlistId: props.playlistId, startIndex: trackIndex });
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
      parent_id: props.playlistId,
      child_id: childPlaylistId,
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
  padding: $spacing-xl;
  overflow-y: auto;
  height: 100%;
}

.header {
  margin-bottom: $spacing-xl;
}

.headerActions {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: $spacing-lg;
}

.backButton {
  background: transparent;
  border: 1px solid var(--color-border);
  color: var(--color-text-secondary);
  padding: $spacing-xs $spacing-md;
  border-radius: $radius-sm;
  clip-path: none !important;
  cursor: pointer;
  font-size: $font-size-sm;
  transition: all $transition-fast;

  &:hover {
    color: var(--color-text-primary);
    border-color: var(--color-text-secondary);
  }
}

.dropdownWrapper {
  position: relative;
}

.actionButton {
  width: 32px;
  height: 32px;
  padding: 0 !important;
  border-radius: 50%;
  clip-path: none !important;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  font-size: $font-size-xl;
  cursor: pointer;
  @include flex-center;
  transition: all $transition-fast;

  &:hover {
    background-color: var(--color-bg-secondary);
    color: var(--color-text-primary);
  }
}

.dropdown {
  position: fixed;
  background-color: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  width: 250px;
  z-index: 10000;
  max-height: 400px;
  overflow-y: auto;
}

.dropdownItem {
  width: 100%;
  display: flex;
  align-items: center;
  gap: $spacing-sm;
  padding: $spacing-sm $spacing-md;
  background: transparent;
  border: none;
  color: var(--color-text-primary);
  font-size: $font-size-sm;
  text-align: left;
  cursor: pointer;
  transition: background-color $transition-fast;

  &:hover {
    background-color: var(--color-bg-hover);
  }
}

.itemIcon {
  font-size: $font-size-base;
}

.itemLabel {
  flex: 1;
}

.itemArrow {
  font-size: $font-size-lg;
  color: var(--color-text-muted);
}

.divider {
  height: 1px;
  background-color: var(--color-border);
  margin: $spacing-xs 0;
}

.searchWrapper {
  padding: $spacing-xs $spacing-md;
}

.searchInput {
  width: 100%;
  padding: $spacing-xs $spacing-sm;
  background-color: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-sm;
  color: var(--color-text-primary);
  font-size: $font-size-sm;
  outline: none;

  &:focus {
    border-color: var(--color-accent-primary);
  }
}

.submenu {
  animation: slideIn 0.15s ease-out;
}

.emptyPlaylist {
  padding: $spacing-lg;
  text-align: center;
  color: var(--color-text-muted);
  font-size: $font-size-sm;
}

@keyframes slideIn {
  from {
    opacity: 0;
    transform: translateX(-10px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

.dangerItem {
  color: #e74c3c;

  &:hover {
    background-color: rgba(231, 76, 60, 0.1);
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

.draggableItem {
  cursor: grab;
  transition: transform $transition-fast;

  &:active {
    cursor: grabbing;
  }
}

.dragging {
  opacity: 0.5;
  transform: scale(0.98);
}

.containerWrapper {
  border-radius: $radius-md;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  position: relative;
  overflow: hidden;

  &:has([data-dropdown-open]) {
    z-index: 1000;
  }
}

.containerTracks {
  border-top: 1px solid var(--color-border);
  padding: $spacing-xs 0;
  border-bottom-left-radius: $radius-md;
  border-bottom-right-radius: $radius-md;
  background-color: var(--color-bg-primary);
}

.containerTrack {
  display: flex;
  align-items: center;
  gap: $spacing-sm;
  padding: $spacing-sm $spacing-md $spacing-sm $spacing-xl;
  cursor: pointer;
  transition: background-color $transition-fast;

  &:hover {
    background-color: var(--color-bg-hover);
  }
}

.trackNumber {
  color: var(--color-text-secondary);
  font-size: $font-size-sm;
  min-width: 24px;
  text-align: right;
}

.trackInfo {
  flex: 1;
  min-width: 0;
}

.trackTitle {
  color: var(--color-text-primary);
  font-size: $font-size-sm;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.trackArtist {
  color: var(--color-text-secondary);
  font-size: $font-size-xs;
}

.containerLoading {
  padding: $spacing-md;
  text-align: center;
  color: var(--color-text-secondary);
  font-size: $font-size-sm;
}
</style>
