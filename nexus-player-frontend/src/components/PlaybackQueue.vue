<script setup lang="ts">
import { errorMessage } from '@/utils/errorMessage';
import type { QueueState } from '@/types/types';
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@/utils/providerAuth';
import MediaRow from './common/MediaRow.vue';
import PlaylistCreateSimpleModal from './PlaylistCreateSimpleModal.vue';
import { usePlaylistActions } from '@/composables/usePlaylistActions';
import VirtualList from './common/VirtualList.vue';
import AppIcon from './common/AppIcon.vue';
import { useI18n } from 'vue-i18n';
import { useAudioStore } from '../stores/audio';
import { useRowHeights } from '@/composables/useRowHeights';

defineEmits<{ close: [] }>();

const { t } = useI18n();
const audioStore = useAudioStore();
const loading = ref(true);
const error = ref<string | null>(null);
const isModalOpen = ref(false);
const isSavingPlaylist = ref(false);
const { addContainerToPlaylist } = usePlaylistActions();
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

const CONTAINER_TRACK_HEIGHT = 42;
const rowHeights = useRowHeights();

const calculateItemHeight = (index: number) => {
  const entry = audioStore.entries[index];
  if (!entry) return 0;
  const rowHeight = rowHeights.compact.value;
  if (entry.entryType === 'container' && expandedContainers.value.has(index) && entry.tracks) {
    return rowHeight + entry.tracks.length * CONTAINER_TRACK_HEIGHT;
  }
  return rowHeight;
};

const isEntryEmpty = computed(() => audioStore.entries.length === 0);

const fetchQueue = async () => {
  try {
    loading.value = true;
    error.value = null;

    const queueState = await invoke<QueueState>('get_queue_state');
    audioStore.updateQueueState(queueState);
  } catch (e) {
    error.value = t('queue.loadFailed', { error: errorMessage(e) });
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
      const result = await invoke<QueueState>('expand_queue_container', { itemIndex });
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
      title: t('queue.playlistTitle', { date: new Date().toLocaleDateString() }),
      description: t('queue.playlistDescription', { date: new Date().toLocaleDateString() }),
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
      const result = await invoke<QueueState>('expand_queue_container', {
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
    error.value = t('common.error', { error: errorMessage(e) });
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

  await addContainerToPlaylist(playlistId, entry.meta.title, async () => {
    if (!entry.tracks) {
      const result = await invoke<QueueState>('expand_queue_container', { itemIndex });
      audioStore.updateQueueState(result);
    }
    const freshEntry = audioStore.entries[itemIndex];
    if (freshEntry?.entryType !== 'container' || !freshEntry.tracks) {
      throw new Error('The container has no tracks');
    }
    return invoke<number>('create_playlist_with_tracks', {
      request: {
        title: freshEntry.meta.title,
        description: `${freshEntry.meta.artist}`,
        tracks: freshEntry.tracks,
      },
    });
  });
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
      <h2 :class="$style.title">{{ $t('queue.title') }}</h2>
      <span :class="$style.count">{{ queue.length }}</span>
      <button :class="$style.close" :title="$t('common.close')" :aria-label="$t('common.close')" @click="$emit('close')">
        <AppIcon name="close" :size="14" />
      </button>
    </div>

    <div v-if="audioStore.searchContext" :class="$style.searchBadge">
      {{ $t('queue.searchContext', { query: audioStore.searchContext.query, system: audioStore.searchContext.system }) }}
    </div>

    <div v-if="loading" :class="$style.loading">{{ $t('common.loading') }}</div>
    <div v-else-if="error" :class="$style.error">{{ error }}</div>
    <div v-else-if="isEntryEmpty" :class="$style.empty">
      <div :class="$style.emptyIcon"><AppIcon name="music-note" :size="40" /></div>
      <div :class="$style.emptyText">{{ $t('queue.empty') }}</div>
      <div :class="$style.emptyHint">{{ $t('queue.emptyHint') }}</div>
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
            <MediaRow
              type="track"
              layout="compact"
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
            <MediaRow
              type="album"
              layout="compact"
              :album="item.entry.meta"
              :active="item.isCurrentItem"
              show-action
              context="queue"
              container-mode
              expandable
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
                    {{ track.title }}
                  </div>
                  <div :class="$style.trackArtist">
                    {{ track.artist }}
                  </div>
                </div>
              </div>
            </div>

            <!-- Not expanded and no tracks loaded yet -->
            <div
              v-else-if="isContainerExpanded(item.index) && !item.entry.tracks"
              :class="$style.containerLoading"
            >
              {{ $t('common.loading') }}
            </div>
          </div>
        </template>
      </VirtualList>
    </div>

    <div v-if="queue.length > 0" :class="$style.queueFooter">
      <button :class="$style.footerButton" @click="openCreatePlaylistModal">
        {{ $t('queue.createPlaylist') }}
      </button>
      <button :class="[$style.footerButton, $style.clearAction]" @click="clearQueue">
        {{ $t('queue.clear') }}
      </button>
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
  overflow: hidden;
  position: relative;
}

.dragHandle {
  display: none;
}

.queueHeader {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 45px;
  padding: 0 12px;
  border-bottom: 1px solid var(--line);
}

.title {
  font: 600 var(--font-size-lg) $font-family-mono;
}

.count {
  flex: 1;
  font: 500 var(--font-size-xs) $font-family-mono;
  color: var(--muted);
}

.close {
  @include flex-center;
  width: 28px;
  height: 28px;
  background: var(--key);
  border-radius: var(--radius-sm);
  box-shadow: 0 1px 0 var(--key-edge);
}

.searchBadge {
  flex: none;
  padding: 6px 12px;
  font-size: var(--font-size-xs);
  color: var(--muted);
  border-bottom: 1px solid var(--line-soft);
  @include text-ellipsis;
}

.loading,
.error {
  padding: 24px 12px;
  text-align: center;
  color: var(--muted);
}

.error {
  color: var(--danger);
}

.empty {
  flex: 1;
  @include flex-center;
  flex-direction: column;
  gap: 6px;
  padding: 24px 12px;
  text-align: center;
  color: var(--muted);
}

.emptyIcon {
  opacity: 0.5;
}

.emptyText {
  font-weight: 600;
  color: var(--text);
}

.emptyHint {
  font-size: var(--font-size-sm);
}

.queueList {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}

.trackWrapper,
.containerWrapper {
  width: 100%;
}

.currentlyPlaying,
.currentlyPlayingList {
  background: var(--bg-selected);
}

.containerTracks {
  background: var(--bg-table);
}

.containerTrack {
  display: grid;
  grid-template-columns: 26px minmax(0, 1fr);
  align-items: center;
  gap: 6px;
  height: 42px;
  padding: 0 12px;
  cursor: pointer;

  &:hover {
    background: var(--bg-selected);
  }
}

.trackNumber {
  font: 500 var(--font-size-xs) $font-family-mono;
  color: var(--muted);
  text-align: right;
}

.trackInfo {
  min-width: 0;
  line-height: 1.2;
}

.trackTitle {
  font-weight: 600;
  @include text-ellipsis;
}

.trackArtist {
  font-size: var(--font-size-xs);
  color: var(--muted);
  @include text-ellipsis;
}

.containerLoading {
  padding: 12px;
  color: var(--muted);
  font-size: var(--font-size-sm);
}

.queueFooter {
  flex: none;
  display: flex;
  gap: 6px;
  padding: 9px 12px;
  border-top: 1px solid var(--line);
}

.footerButton {
  height: 30px;
  padding: 0 10px;
  background: var(--key);
  border-radius: var(--radius-sm);
  box-shadow: 0 1px 0 var(--key-edge);
  font: 600 0.625rem $font-family-mono;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  white-space: nowrap;

  &:first-child {
    flex: 1;
    min-width: 0;
    @include text-ellipsis;
  }
}

.clearAction {
  color: var(--danger);
}

@media (max-width: $mobile-breakpoint) {
  .dragHandle {
    display: block;
    flex: none;
    width: 36px;
    height: 3px;
    margin: 8px auto 0;
    background: var(--muted);
  }

  .footerButton {
    height: 40px;
  }
}
</style>
