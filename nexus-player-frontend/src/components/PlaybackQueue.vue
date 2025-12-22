<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCardCompact from './common/MediaCardCompact.vue';

interface TrackDto {
  id: number;
  title: string;
  artist: string | null;
  album: string | null;
  durationSecs: number | null;
  artworkUrl: string | null;
  playCount: number;
}

interface QueueStateDto {
  tracks: TrackDto[];
  currentIndex: number | null;
  repeatMode: string;
  shuffleEnabled: boolean;
}

interface QueueTrack {
  index: number;
  id: number;
  title: string;
  artist: string | null;
  thumbnail: string | null;
  duration: number | null;
  isCurrentlyPlaying: boolean;
}

const queue = ref<QueueTrack[]>([]);
const currentIndex = ref<number | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);

const fetchQueue = async () => {
  try {
    loading.value = true;
    error.value = null;

    const queueState = await invoke<QueueStateDto>('get_queue_state');
    currentIndex.value = queueState.currentIndex;

    queue.value = queueState.tracks.map((track, index) => {
      const isPlaying = index === queueState.currentIndex;
      console.log(`Track ${index}: ${track.title}, isPlaying: ${isPlaying}`);

      return {
        index,
        id: track.id,
        title: track.title,
        artist: track.artist,
        thumbnail: track.artworkUrl,
        duration: track.durationSecs,
        isCurrentlyPlaying: isPlaying,
      };
    });
  } catch (e) {
    error.value = `Failed to load queue: ${e}`;
    console.error('Error loading queue:', e);
  } finally {
    loading.value = false;
  }
};

const playAtIndex = async (index: number) => {
  try {
    await invoke('play_at_index', { index });
    await fetchQueue();
  } catch (e) {
    console.error('Failed to play at index:', e);
  }
};

const removeFromQueue = async (index: number) => {
  try {
    await invoke('remove_from_queue', { index });
    await fetchQueue();
  } catch (e) {
    console.error('Failed to remove from queue:', e);
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

onMounted(() => {
  fetchQueue();
});

defineExpose({ fetchQueue });
</script>

<template>
  <div :class="$style.playbackQueue">
    <div :class="$style.queueHeader">
      <h2 :class="$style.title">В очереди</h2>
      <button
        v-if="queue.length > 0"
        @click="clearQueue"
        :class="$style.clearButton"
        title="Clear Queue"
      >
        Очистить
      </button>
    </div>

    <div v-if="loading" :class="$style.loading">Загрузка...</div>
    <div v-else-if="error" :class="$style.error">{{ error }}</div>
    <div v-else-if="queue.length === 0" :class="$style.empty">
      <div :class="$style.emptyIcon">🎵</div>
      <div :class="$style.emptyText">Очередь пуста</div>
      <div :class="$style.emptyHint">Добавьте треки для воспроизведения</div>
    </div>

    <div v-else :class="$style.queueList">
      <div
        v-for="track in queue"
        :key="track.index"
        :class="[
          $style.trackWrapper,
          { [$style.currentlyPlaying]: track.isCurrentlyPlaying },
        ]"
      >
        <MediaCardCompact
          type="track"
          :title="track.title"
          :artist="track.artist || 'Unknown Artist'"
          :thumbnail="track.thumbnail || undefined"
          @click="playAtIndex(track.index)"
          @play="playAtIndex(track.index)"
        />
        <div v-if="track.isCurrentlyPlaying" :class="$style.playingIndicator">
          ▶
        </div>
      </div>
    </div>
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
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: $spacing-md;
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

.currentlyPlaying {
  background: linear-gradient(
    135deg,
    rgba(102, 126, 234, 0.15) 0%,
    rgba(118, 75, 162, 0.15) 100%
  );
  border: 2px solid var(--color-accent-primary);
  box-shadow: 0 0 20px rgba(102, 126, 234, 0.3);

  &::after {
    content: '';
    position: absolute;
    top: -2px;
    left: -2px;
    right: -2px;
    bottom: -2px;
    border-radius: $radius-md;
    border: 2px solid var(--color-accent-primary);
    pointer-events: none;
    animation: pulse 2s ease-in-out infinite;
  }
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}

.playingIndicator {
  position: absolute;
  top: $spacing-sm;
  left: $spacing-sm;
  background: linear-gradient(
    135deg,
    var(--color-accent-primary) 0%,
    var(--color-accent-purple) 100%
  );
  color: white;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  @include flex-center;
  font-size: $font-size-sm;
  z-index: 10;
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.5);
  animation: playingPulse 1.5s ease-in-out infinite;
}

@keyframes playingPulse {
  0%,
  100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.1);
  }
}
</style>
