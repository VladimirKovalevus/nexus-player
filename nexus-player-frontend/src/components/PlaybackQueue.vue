<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCard from './common/MediaCard.vue';
import PlaylistCreateSimpleModal from './PlaylistCreateSimpleModal.vue';
import VirtualList from './common/VirtualList.vue';
import MarqueeText from './common/MarqueeText.vue';
import AppIcon from './common/AppIcon.vue';
import { useAudioStore } from '../stores/audio';
import { useThemeStore } from '../stores/theme';
import type { QueueEntryDto } from '@/types/types';

const audioStore = useAudioStore();
const themeStore = useThemeStore();
const loading = ref(true);
const error = ref<string | null>(null);
const isModalOpen = ref(false);
const isSavingPlaylist = ref(false);
const expandedContainers = ref<Set<number>>(new Set());
const playlistInitialData = ref<{
  title?: string;
  description?: string;
  artworkUrl?: string;
}>({});

// Modal mode: 'queue' for whole queue, or {index, entry} for single container
const modalTarget = ref<'queue' | { index: number }>('queue');

const mapContentType = (type: string) => {
  if (!type) return null;
  return type.charAt(0).toUpperCase() + type.slice(1);
};

const queue = computed(() => {
  return audioStore.entries.map((entry, index) => ({
    entry,
    index,
    isCurrentItem: index === audioStore.currentPosition?.itemIndex,
  }));
});

const calculateItemHeight = (index: number) => {
  const entry = audioStore.entries[index];
  if (!entry) return 0;
  if (entry.entryType === 'track') return 88;
  if (entry.entryType === 'container') {
    if (expandedContainers.value.has(index) && entry.tracks) {
      const trackHeight = themeStore.activeTheme === 'retro' ? 32 : 42;
      return 88 + entry.tracks.length * trackHeight; // 88 header + tracks
    }
    return 88;
  }
  return 88;
};

const isEntryEmpty = computed(() => audioStore.entries.length === 0);

const fetchQueue = async () => {
  try {
    loading.value = true;
    error.value = null;

    const queueState = await invoke<any>('get_queue_state');
    audioStore.updateQueueState(queueState);
  } catch (e) {
    error.value = `Failed to load queue: ${e}`;
    console.error('Error loading queue:', e);
  } finally {
    loading.value = false;
  }
};

const playQueueItem = async (
  itemIndex: number,
  trackIndexInContainer?: number,
) => {
  audioStore.startLoading();
  try {
    await invoke('play_queue_item', {
      itemIndex,
      trackIndexInContainer: trackIndexInContainer ?? null,
    });
    await fetchQueue();
  } catch (e) {
    console.error('Failed to play queue item:', e);
    audioStore.stopLoading();
  }
};

const expandContainer = async (itemIndex: number) => {
  try {
    const entry = audioStore.entries[itemIndex];
    if (entry?.entryType === 'container' && !entry.tracks) {
      const result = await invoke<any>('expand_queue_container', { itemIndex });
      audioStore.updateQueueState(result);
    }
    expandedContainers.value.add(itemIndex);
  } catch (e) {
    console.error('Failed to expand container:', e);
  }
};

const toggleContainer = (itemIndex: number) => {
  if (expandedContainers.value.has(itemIndex)) {
    expandedContainers.value.delete(itemIndex);
  } else {
    expandContainer(itemIndex);
  }
};

const isContainerExpanded = (itemIndex: number) =>
  expandedContainers.value.has(itemIndex);

const playAtIndex = async (index: number) => {
  audioStore.startLoading();
  try {
    await invoke('play_queue_item', {
      itemIndex: index,
      trackIndexInContainer: null,
    });
    await fetchQueue();
  } catch (e) {
    console.error('Failed to play at index:', e);
    audioStore.stopLoading();
  }
};

const clearQueue = async () => {
  try {
    await invoke('clear_queue');
    await fetchQueue();
  } catch (e) {
    console.error('Failed to clear queue:', e);
  }
};

const openCreatePlaylistModal = () => {
  if (queue.value.length > 0) {
    playlistInitialData.value = {
      title: `Очередь ${new Date().toLocaleDateString()}`,
      description: `Сформирован из очереди ${new Date().toLocaleDateString()}`,
      artworkUrl: '',
    };
    modalTarget.value = 'queue';
    isModalOpen.value = true;
  }
};

// Ensure all containers have their tracks loaded
const ensureAllContainersExpanded = async () => {
  const entries = audioStore.entries;
  for (let i = 0; i < entries.length; i++) {
    const entry = entries[i];
    if (entry.entryType === 'container' && !entry.tracks) {
      const result = await invoke<any>('expand_queue_container', {
        itemIndex: i,
      });
      audioStore.updateQueueState(result);
    }
  }
};

const handleCreatePlaylist = async (data: {
  title: string;
  description: string | null;
  artworkUrl: string | null;
}) => {
  if (isSavingPlaylist.value) return;

  isSavingPlaylist.value = true;
  error.value = null;
  try {
    if (modalTarget.value !== 'queue') {
      // Creating playlist from a single container
      const idx = modalTarget.value.index;
      const entry = audioStore.entries[idx];
      if (entry?.entryType === 'container') {
        // Prefer create_playlist_from_container for streaming containers
        await invoke<number>('create_playlist_from_container', {
          provider: entry.meta.system,
          contentType: mapContentType(entry.meta.kind),
          id: entry.meta.id,
          title: data.title,
          description: data.description,
          artworkUrl: data.artworkUrl,
        });
      }
      isModalOpen.value = false;
      return;
    }

    // Creating playlist from whole queue — preserve structure
    await ensureAllContainersExpanded();

    const entries = audioStore.entries;
    if (entries.length === 0) {
      isModalOpen.value = false;
      return;
    }

    // Check if queue has only tracks (no containers) — use simple path
    const hasContainers = entries.some((e) => e.entryType === 'container');

    if (!hasContainers) {
      const tracks = audioStore.tracks;
      await invoke<number>('create_playlist_with_tracks', {
        request: {
          title: data.title,
          description: data.description,
          artworkUrl: data.artworkUrl,
          tracks,
        },
      });
    } else {
      // Create parent playlist
      const parentId = await invoke<number>('create_playlist', {
        request: {
          title: data.title,
          description: data.description,
          artworkUrl: data.artworkUrl,
        },
      });

      for (const entry of entries) {
        if (entry.entryType === 'track') {
          await invoke('add_track_to_playlist', {
            playlistId: parentId,
            track: entry.track,
          });
        } else if (entry.entryType === 'container' && entry.tracks) {
          // Create sub-playlist from container tracks
          const childId = await invoke<number>('create_playlist_with_tracks', {
            request: {
              title: entry.meta.title,
              description: `${entry.meta.artist}`,
              tracks: entry.tracks,
            },
          });
          // Nest it
          await invoke('add_playlist_to_playlist', {
            parentPlaylistId: parentId,
            childPlaylistId: childId,
          });
        }
      }
    }

    isModalOpen.value = false;
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e);
    error.value = `Ошибка: ${errorMessage}`;
    console.error('Failed to create playlist:', e);
  } finally {
    isSavingPlaylist.value = false;
  }
};

const handleContainerCreatePlaylist = (itemIndex: number) => {
  const entry = audioStore.entries[itemIndex];
  if (entry && entry.entryType === 'container') {
    playlistInitialData.value = {
      title: entry.meta.title,
      description: entry.meta.artist,
      artworkUrl: entry.meta.thumbnail || '',
    };
  }
  modalTarget.value = { index: itemIndex };
  isModalOpen.value = true;
};

const handleContainerAddToPlaylist = async (
  itemIndex: number,
  playlistId: number,
) => {
  const entry = audioStore.entries[itemIndex];
  if (entry?.entryType !== 'container') return;

  try {
    if (!entry.tracks) {
      const result = await invoke<any>('expand_queue_container', { itemIndex });
      audioStore.updateQueueState(result);
    }
    const freshEntry = audioStore.entries[itemIndex];
    if (freshEntry?.entryType === 'container' && freshEntry.tracks) {
      const childId = await invoke<number>('create_playlist_with_tracks', {
        request: {
          title: freshEntry.meta.title,
          description: `${freshEntry.meta.artist}`,
          tracks: freshEntry.tracks,
        },
      });
      await invoke('add_playlist_to_playlist', {
        parentPlaylistId: playlistId,
        childPlaylistId: childId,
      });
    }
  } catch (e) {
    console.error('Failed to add container to playlist:', e);
  }
};

const handleContainerRemove = async (itemIndex: number) => {
  try {
    await invoke('remove_from_queue', { index: itemIndex });
    await fetchQueue();
  } catch (e) {
    console.error('Failed to remove container from queue:', e);
  }
};

onMounted(() => {
  fetchQueue();
});

defineExpose({ fetchQueue, openCreatePlaylistModal, clearQueue });
</script>

<template>
  <div :class="$style.playbackQueue">
    <div :class="$style.dragHandle" aria-hidden="true" />
    <div :class="$style.queueHeader">
      <h2 :class="$style.title">В очереди</h2>

      <div :class="$style.actionsMenu" v-if="queue.length > 0">
        <button :class="$style.menuToggle">
          <AppIcon name="more-horizontal" :size="16" style="transform: rotate(90deg)" />
        </button>
        <div :class="$style.menuContent">
          <button @click="openCreatePlaylistModal">
            Сформировать плейлист
          </button>
          <button @click="clearQueue" :class="$style.clearAction">
            Очистить
          </button>
        </div>
      </div>
    </div>

    <div v-if="audioStore.searchContext" :class="$style.searchBadge">
      Search: "{{ audioStore.searchContext.query }}" ({{
        audioStore.searchContext.system
      }})
    </div>

    <div v-if="loading" :class="$style.loading">Загрузка...</div>
    <div v-else-if="error" :class="$style.error">{{ error }}</div>
    <div v-else-if="isEntryEmpty" :class="$style.empty">
      <div :class="$style.emptyIcon"><AppIcon name="music-note" :size="64" /></div>
      <div :class="$style.emptyText">Очередь пуста</div>
      <div :class="$style.emptyHint">Добавьте треки для воспроизведения</div>
    </div>

    <div v-else :class="[$style.queueList, 'queue-scroll-container']">
      <VirtualList
        :items="queue"
        :item-height="calculateItemHeight"
        :overscan="8"
        container-selector=".queue-scroll-container"
      >
        <template #default="{ item }">
          <!-- Track entry -->
          <div
            v-if="item.entry.entryType === 'track'"
            :class="[
              $style.trackWrapper,
              {
                [$style.currentlyPlaying]:
                  item.isCurrentItem &&
                  audioStore.currentPosition?.trackWithinContainer == null,
              },
            ]"
          >
            <MediaCard
              type="track"
              :track="item.entry.track"
              :queue-index="item.index"
              :active="
                item.isCurrentItem &&
                audioStore.currentPosition?.trackWithinContainer == null
              "
              context="queue"
              showAction
              @click="playQueueItem(item.index)"
              @play="playQueueItem(item.index)"
              @refresh="fetchQueue"
            />
          </div>

          <!-- Container entry -->
          <div
            v-else-if="item.entry.entryType === 'container'"
            :class="[
              $style.containerWrapper,
              { [$style.currentlyPlayingList]: item.isCurrentItem },
            ]"
          >
            <MediaCard
              type="album"
              :album="item.entry.meta"
              :active="item.isCurrentItem"
              show-action
              context="queue"
              container-mode
              :is-expanded="isContainerExpanded(item.index)"
              @click="toggleContainer(item.index)"
              @toggle-expand="toggleContainer(item.index)"
              @container-create-playlist="
                handleContainerCreatePlaylist(item.index)
              "
              @container-add-to-playlist="
                (plId) => handleContainerAddToPlaylist(item.index, plId)
              "
              @container-remove="handleContainerRemove(item.index)"
            />

            <!-- Expanded tracks inside container -->
            <div
              v-if="isContainerExpanded(item.index) && item.entry.tracks"
              :class="$style.containerTracks"
            >
              <div
                v-for="(track, trackIdx) in item.entry.tracks"
                :key="trackIdx"
                :class="[
                  $style.containerTrack,
                  {
                    [$style.currentlyPlaying]:
                      item.isCurrentItem &&
                      audioStore.currentPosition?.trackWithinContainer ===
                        trackIdx,
                  },
                ]"
                @click="playQueueItem(item.index, trackIdx)"
              >
                <span :class="$style.trackNumber">{{ trackIdx + 1 }}</span>
                <div :class="$style.trackInfo">
                  <div :class="$style.trackTitle">
                    <MarqueeText
                      :active="
                        item.isCurrentItem &&
                        audioStore.currentPosition?.trackWithinContainer ===
                          trackIdx
                      "
                    >
                      {{ track.title }}
                    </MarqueeText>
                  </div>
                  <div :class="$style.trackArtist">
                    <MarqueeText
                      :active="
                        item.isCurrentItem &&
                        audioStore.currentPosition?.trackWithinContainer ===
                          trackIdx
                      "
                    >
                      {{ track.artist }}
                    </MarqueeText>
                  </div>
                </div>
              </div>
            </div>

            <!-- Not expanded and no tracks loaded yet -->
            <div
              v-else-if="isContainerExpanded(item.index) && !item.entry.tracks"
              :class="$style.containerLoading"
            >
              Loading tracks...
            </div>
          </div>
        </template>
      </VirtualList>
    </div>

    <PlaylistCreateSimpleModal
      :is-open="isModalOpen"
      :is-loading="isSavingPlaylist"
      :initial-data="playlistInitialData"
      @close="isModalOpen = false"
      @save="handleCreatePlaylist"
    />
  </div>
</template>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.playbackQueue {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: $spacing-lg;
  overflow: hidden;
}

.dragHandle {
  display: none;
  width: 40px;
  height: 4px;
  background-color: var(--color-border-light);
  border-radius: 2px;
  margin: 0 auto $spacing-md;
  flex-shrink: 0;

  @media (max-width: 480px) {
    display: block;
  }
}

.queueHeader {
  @include flex-between;
  align-items: center;
  margin-bottom: $spacing-lg;
  padding-bottom: $spacing-md;
  border-bottom: 1px solid var(--color-border);
}

.title {
  font-size: $font-size-lg;
  font-weight: $font-weight-semibold;
  color: var(--color-accent-primary);
  margin: 0;
}

.actionsMenu {
  position: relative;

  .menuToggle {
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: $radius-md;
    clip-path: none !important;
    padding: 0 !important;
    color: var(--color-text-secondary);
    cursor: pointer;
    font-size: $font-size-xl;
    line-height: 1 !important;
    padding-bottom: var(--ellipsis-offset-bottom) !important;
    width: 36px;
    height: 36px;
    @include flex-center;
    align-items: var(--ellipsis-v-align, center) !important;
    // transition: all $transition-fast;

    &:hover {
      background-color: var(--color-bg-hover);
      border-color: var(--color-border-light);
    }
  }

  .menuContent {
    display: none;
    position: absolute;
    right: 0;
    top: 100%;
    margin-top: $spacing-xs;
    background-color: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: $radius-md;
    box-shadow: $shadow-lg;
    z-index: 10;
    width: 200px;
    padding: $spacing-xs;

    button {
      display: block;
      width: 100%;
      padding: $spacing-sm $spacing-md;
      background: none;
      border: none;
      text-align: left;
      font-size: $font-size-sm;
      color: var(--color-text-primary);
      cursor: pointer;
      border-radius: $radius-sm;
      transition: background-color $transition-fast;

      &:hover {
        background-color: var(--color-bg-hover);
      }
    }

    .clearAction {
      color: #e74c3c;
      &:hover {
        background-color: rgba(231, 76, 60, 0.1);
      }
    }
  }

  &:hover .menuContent,
  .menuToggle:focus + .menuContent,
  .menuContent:hover {
    display: block;
  }
}

.clearButton {
  padding: $spacing-xs $spacing-md;
  font-size: $font-size-sm;
  background-color: transparent;
  color: var(--color-text-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  cursor: pointer;
  transition: all $transition-fast;

  &:hover {
    background-color: #e74c3c;
    border-color: #e74c3c;
    color: white;
  }
}

.loading,
.error {
  padding: $spacing-xl;
  text-align: center;
  color: var(--color-text-secondary);
  font-size: $font-size-sm;
}

.error {
  color: #e74c3c;
  background-color: rgba(231, 76, 60, 0.1);
  border-radius: $radius-md;
}

.empty {
  flex: 1;
  @include flex-center;
  flex-direction: column;
  gap: $spacing-md;
  padding: $spacing-2xl;
}

.emptyIcon {
  font-size: 64px;
  opacity: 0.3;
}

.emptyText {
  font-size: $font-size-lg;
  font-weight: $font-weight-semibold;
  color: var(--color-text-primary);
}

.emptyHint {
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
}

.queueList {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: $spacing-sm;
  overflow-y: auto;
  @include custom-scrollbar(
    var(--color-border-light),
    var(--color-bg-secondary)
  );
  padding-right: $spacing-xs;
}

.trackWrapper {
  position: relative;
  border-radius: $radius-md;
  transition: all $transition-fast;
  color: var(--color-bg-primary) !important;
}

.searchBadge {
  padding: $spacing-xs $spacing-md;
  margin-bottom: $spacing-md;
  background: rgba(102, 126, 234, 0.15);
  border-radius: $radius-md;
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
}

.currentlyPlaying {
  border-radius: $radius-md;
  animation: bg-pulse 2s ease-in-out infinite;

  // Делаем фон MediaCard прозрачным, чтобы была видна анимация обертки
  & > div {
    background-color: var(--media-card-current-playing) !important;
  }
}
.currentlyPlayingList {
  border-radius: $radius-md;
  & > div {
    background-color: transparent !important;
  }
}

.containerWrapper {
  border-radius: $radius-md;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  position: relative;

  &:has([data-dropdown-open]) {
    z-index: 1000;
  }

  &.currentlyPlaying {
    border-color: transparent;
  }
}

.containerTracks {
  border-top: 1px solid var(--color-border);
  padding: $spacing-xs 0;
  border-bottom-left-radius: $radius-md;
  border-bottom-right-radius: $radius-md;
}

.containerTrack {
  display: flex;
  align-items: center;
  gap: $spacing-sm;
  padding: $spacing-xs $spacing-md $spacing-xs $spacing-xl;
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
  @include text-ellipsis;
}

.trackArtist {
  color: var(--color-text-secondary);
  font-size: $font-size-xs;
  @include text-ellipsis;
}

.containerLoading {
  padding: $spacing-md;
  text-align: center;
  color: var(--color-text-secondary);
  font-size: $font-size-sm;
}

@keyframes bg-pulse {
  0%,
  100% {
    background-color: var(--playback-queue-pulse, rgba(76, 175, 80, 0.1));
  }
  50% {
    background-color: transparent;
  }
}
</style>
