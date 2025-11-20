<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

// Tracks will be fetched from backend
const tracks = ref<any[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);

// Fetch all tracks on component mount
onMounted(async () => {
  try {
    loading.value = true;
    tracks.value = await invoke('get_all_tracks');
  } catch (e) {
    error.value = `Failed to load tracks: ${e}`;
    console.error('Error loading tracks:', e);
  } finally {
    loading.value = false;
  }
});

// Reload tracks (can be called after adding new track)
const reloadTracks = async () => {
  try {
    loading.value = true;
    error.value = null;
    tracks.value = await invoke('get_all_tracks');
  } catch (e) {
    error.value = `Failed to reload tracks: ${e}`;
    console.error('Error reloading tracks:', e);
  } finally {
    loading.value = false;
  }
};
const trackClick = async (track)=>{
    console.log(track,'track Click')
    const res =await invoke('add_to_queue',{trackId:track.id})
    console.log(res,'res Click')
}

// Format duration from seconds to MM:SS
const formatDuration = (seconds: number | null): string => {
  if (!seconds) return '--:--';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

// Expose reload method to parent
defineExpose({ reloadTracks });
</script>

<template>
  <div class="track-list">
    <h2>Tracks</h2>

    <div v-if="loading" class="loading">Loading tracks...</div>
    <div v-else-if="error" class="error">{{ error }}</div>
    <div v-else-if="tracks.length === 0" class="empty">No tracks yet. Add some!</div>

    <div v-else class="tracks">
      <div
        v-for="track in tracks"
        :key="track.id"
        class="track-item"
        @click="trackClick(track)"
      >
        <div class="track-info">
          <div class="track-title">{{ track.title }}</div>
          <div class="track-meta">
            <span v-if="track.artist">{{ track.artist }}</span>
            <span v-if="track.album"> • {{ track.album }}</span>
          </div>
        </div>
        <div class="track-duration">
          {{ formatDuration(track.durationSecs) }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.track-list {
  width: 100%;
  max-width: 800px;
  margin: 0 auto;
}

h2 {
  margin-bottom: 1rem;
  color: #333;
}

.loading, .error, .empty {
  padding: 2rem;
  text-align: center;
  color: #666;
}

.error {
  color: #e74c3c;
  background-color: #ffe6e6;
  border-radius: 8px;
}

.tracks {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.track-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1rem;
  background-color: #ffffff;
  border-radius: 8px;
  border: 1px solid #e0e0e0;
  transition: all 0.2s;
  cursor: pointer;
}

.track-item:hover {
  background-color: #f5f5f5;
  border-color: #396cd8;
  transform: translateY(-2px);
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.1);
}

.track-info {
  flex: 1;
  min-width: 0;
}

.track-title {
  font-weight: 600;
  font-size: 1rem;
  color: #0f0f0f;
  margin-bottom: 0.25rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-meta {
  font-size: 0.875rem;
  color: #666;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-duration {
  font-family: 'Courier New', monospace;
  color: #888;
  margin-left: 1rem;
}

@media (prefers-color-scheme: dark) {
  h2 {
    color: #f6f6f6;
  }

  .track-item {
    background-color: #1a1a1a;
    border-color: #333;
  }

  .track-item:hover {
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
    color: #888;
  }
}
</style>
