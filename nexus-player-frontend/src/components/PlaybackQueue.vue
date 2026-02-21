<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCard from './common/MediaCard.vue';
import PlaylistCreateSimpleModal from './PlaylistCreateSimpleModal.vue';
import { useAudioStore } from '../stores/audio';
import type { QueueEntryDto } from '@/types/types';

const audioStore = useAudioStore();
const loading = ref(true);
const error = ref<string | null>(null);
const isModalOpen = ref(false);
const isSavingPlaylist = ref(false);
const expandedContainers = ref<Set<number>>(new Set());

const queue = computed(() => {
  return audioStore.entries.map((entry, index) => ({
    entry,
    index,
    isCurrentItem: index === audioStore.currentPosition?.itemIndex,
  }));
});

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

const playQueueItem = async (itemIndex: number, trackIndexInContainer?: number) => {
  try {
    await invoke('play_queue_item', {
      itemIndex,
      trackIndexInContainer: trackIndexInContainer ?? null,
    });
    await fetchQueue();
  } catch (e) {
    console.error('Failed to play queue item:', e);
  }
};

const expandContainer = async (itemIndex: number) => {
  try {
    const entry = audioStore.entries[itemIndex];
    if (entry?.entryType === 'container' && !entry.tracks) {
      await invoke('expand_queue_container', { itemIndex });
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

const isContainerExpanded = (itemIndex: number) => expandedContainers.value.has(itemIndex);

const playAtIndex = async (index: number) => {
  try {
    await invoke('play_queue_item', { itemIndex: index, trackIndexInContainer: null });
    await fetchQueue();
  } catch (e) {
    console.error('Failed to play at index:', e);
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
    isModalOpen.value = true;
  }
};

const handleCreatePlaylist = async (name: string) => {
  if (isSavingPlaylist.value) return;

  isSavingPlaylist.value = true;
  error.value = null;
  try {
    const tracks = audioStore.tracks;

    if (tracks.length === 0) {
      isModalOpen.value = false;
      return;
    }

    await invoke<number>('create_playlist_with_tracks', {
      request: {
        title: name,
        description: `Сформирован из очереди ${new Date().toLocaleDateString()}`,
        tracks: tracks,
      },
    });

    isModalOpen.value = false;
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e);
    error.value = `Ошибка: ${errorMessage}`;
    console.error('Failed to create playlist:', e);
  } finally {
    isSavingPlaylist.value = false;
  }
};

onMounted(() => {
  fetchQueue();
});

defineExpose({ fetchQueue });
</script>

<template>
  <div :class="$style.playbackQueue">
    <div :class="$style.queueHeader">
      <h2 :class="$style.title">В очереди</h2>

      <div :class="$style.actionsMenu" v-if="queue.length > 0">
        <button :class="$style.menuToggle">⋮</button>
        <div :class="$style.menuContent">
          <button @click="openCreatePlaylistModal">Сформировать плейлист</button>
          <button @click="clearQueue" :class="$style.clearAction">
            Очистить
          </button>
        </div>
      </div>
    </div>

    <div v-if="audioStore.searchContext" :class="$style.searchBadge">
      Search: "{{ audioStore.searchContext.query }}" ({{ audioStore.searchContext.system }})
    </div>

    <div v-if="loading" :class="$style.loading">Загрузка...</div>
    <div v-else-if="error" :class="$style.error">{{ error }}</div>
    <div v-else-if="isEntryEmpty" :class="$style.empty">
      <div :class="$style.emptyIcon">🎵</div>
      <div :class="$style.emptyText">Очередь пуста</div>
      <div :class="$style.emptyHint">Добавьте треки для воспроизведения</div>
    </div>

    <div v-else :class="$style.queueList">
      <template v-for="item in queue" :key="item.index">
        <!-- Track entry -->
        <div
          v-if="item.entry.entryType === 'track'"
          :class="[
            $style.trackWrapper,
            { [$style.currentlyPlaying]: item.isCurrentItem && audioStore.currentPosition?.trackWithinContainer == null },
          ]"
        >
          <MediaCard
            type="track"
            :track="item.entry.track"
            :queue-index="item.index"
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
          :class="[$style.containerWrapper, { [$style.currentlyPlaying]: item.isCurrentItem }]"
        >
          <div :class="$style.containerHeader">
            <MediaCard
              type="album"
              :album="item.entry.meta"
              :show-action="false"
              context="queue"
              @click="toggleContainer(item.index)"
            />
            <button :class="$style.expandBtn" @click="toggleContainer(item.index)">
              {{ isContainerExpanded(item.index) ? '▲' : '▼' }}
            </button>
          </div>

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
                    audioStore.currentPosition?.trackWithinContainer === trackIdx,
                },
              ]"
              @click="playQueueItem(item.index, trackIdx)"
            >
              <span :class="$style.trackNumber">{{ trackIdx + 1 }}</span>
              <div :class="$style.trackInfo">
                <div :class="$style.trackTitle">{{ track.title }}</div>
                <div :class="$style.trackArtist">{{ track.artist }}</div>
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
    </div>

    <PlaylistCreateSimpleModal
      :is-open="isModalOpen"
      :is-loading="isSavingPlaylist"
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
  color: var(--color-text-primary);
  margin: 0;
}

.actionsMenu {
  position: relative;

  .menuToggle {
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: $radius-md;
    color: var(--color-text-secondary);
    cursor: pointer;
    font-size: 1.5rem;
    line-height: 1;
    width: 36px;
    height: 36px;
    transition: all $transition-fast;

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
  position: relative;

  &::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: $radius-md;
    background: linear-gradient(
      135deg,
      rgba(102, 126, 234, 0.1) 0%,
      rgba(118, 75, 162, 0.1) 100%
    );
    border: 2px solid var(--color-accent-primary);
    pointer-events: none;
    animation: pulse 2s ease-in-out infinite;
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
}

.containerHeader {
  display: flex;
  align-items: center;
  gap: $spacing-xs;

  > :first-child {
    flex: 1;
    min-width: 0;
  }
}

.expandBtn {
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: $spacing-xs $spacing-md $spacing-xs $spacing-xs;
  font-size: $font-size-sm;
  flex-shrink: 0;
}

.containerTracks {
  border-top: 1px solid var(--color-border);
  padding: $spacing-xs 0;
  border-bottom-left-radius: $radius-md;
  border-bottom-right-radius: $radius-md;
  overflow: hidden;
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

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
    box-shadow: 0 0 20px rgba(102, 126, 234, 0.3);
  }
  50% {
    opacity: 0.7;
    box-shadow: 0 0 10px rgba(102, 126, 234, 0.2);
  }
}
</style>
