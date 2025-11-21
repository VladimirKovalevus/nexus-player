<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface QueueTrack {
  index: number;
  trackId: number;
  title: string;
  artist: string | null;
  album: string | null;
  durationSecs: number | null;
}

const queue = ref<QueueTrack[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);

const fetchQueue = async () => {
  try {
    loading.value = true;
    error.value = null;
    queue.value = await invoke('get_queue');
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

const formatDuration = (seconds: number | null): string => {
  if (!seconds || !isFinite(seconds)) return '0:00';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

onMounted(() => {
  fetchQueue();
});

defineExpose({ fetchQueue });
</script>

<template>
  <div class="playback-queue">
    <div class="queue-header">
      <h2>Playback Queue</h2>
      <button
        v-if="queue.length > 0"
        @click="clearQueue"
        class="clear-btn"
        title="Clear Queue"
      >
        Clear All
      </button>
    </div>

    <div v-if="loading" class="loading">Loading queue...</div>
    <div v-else-if="error" class="error">{{ error }}</div>
    <div v-else-if="queue.length === 0" class="empty">
      Queue is empty. Add tracks to start playing!
    </div>

    <div v-else class="queue-list">
      <div
        v-for="track in queue"
        :key="track.index"
        class="queue-item"
        @dblclick="playAtIndex(track.index)"
      >
        <div class="track-number">{{ track.index + 1 }}</div>
        <div class="track-info">
          <div class="track-title">{{ track.title }}</div>
          <div class="track-meta">
            <span v-if="track.artist">{{ track.artist }}</span>
            <span v-if="track.artist && track.album"> • </span>
            <span v-if="track.album">{{ track.album }}</span>
          </div>
        </div>
        <div class="track-duration">
          {{ formatDuration(track.durationSecs) }}
        </div>
        <button
          @click.stop="removeFromQueue(track.index)"
          class="remove-btn"
          title="Remove from queue"
        >
          ✕
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.playback-queue {
  width: 100%;
  max-width: 900px;
  margin: 0 auto;
}

.queue-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
}

h2 {
  margin: 0;
  color: #333;
}

.clear-btn {
  padding: 0.5em 1em;
  font-size: 0.9rem;
  background-color: #e74c3c;
  color: white;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
}

.clear-btn:hover {
  background-color: #c0392b;
  transform: translateY(-1px);
}

.loading,
.error,
.empty {
  padding: 2rem;
  text-align: center;
  color: #666;
}

.error {
  color: #e74c3c;
  background-color: #ffe6e6;
  border-radius: 8px;
}

.queue-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  max-height: 500px;
  overflow-y: auto;
}

.queue-item {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.75rem 1rem;
  background-color: #ffffff;
  border-radius: 8px;
  border: 1px solid #e0e0e0;
  transition: all 0.2s;
  cursor: pointer;
}

.queue-item:hover {
  background-color: #f5f5f5;
  border-color: #396cd8;
  transform: translateX(4px);
}

.track-number {
  font-weight: 600;
  font-size: 1rem;
  color: #999;
  min-width: 30px;
  text-align: center;
}

.track-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  overflow: hidden;
}

.track-title {
  font-weight: 600;
  font-size: 1rem;
  color: #0f0f0f;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-meta {
  font-size: 0.85rem;
  color: #666;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-duration {
  font-family: 'Courier New', monospace;
  font-size: 0.9rem;
  color: #666;
  min-width: 50px;
  text-align: right;
}

.remove-btn {
  width: 32px;
  height: 32px;
  padding: 0;
  background: none;
  border: 2px solid #e74c3c;
  border-radius: 50%;
  color: #e74c3c;
  font-size: 1rem;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.remove-btn:hover {
  background-color: #e74c3c;
  color: white;
  transform: scale(1.1);
}

@media (prefers-color-scheme: dark) {
  h2 {
    color: #f6f6f6;
  }

  .queue-item {
    background-color: #1a1a1a;
    border-color: #333;
  }

  .queue-item:hover {
    background-color: #252525;
    border-color: #24c8db;
  }

  .track-title {
    color: #f6f6f6;
  }

  .track-meta {
    color: #aaa;
  }

  .track-duration {
    color: #aaa;
  }

  .track-number {
    color: #666;
  }

  .remove-btn {
    border-color: #e74c3c;
    color: #e74c3c;
  }

  .remove-btn:hover {
    background-color: #e74c3c;
    color: white;
  }
}
</style>
