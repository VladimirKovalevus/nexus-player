<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

// Playlists will be fetched from backend
const playlists = ref<any[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);

// Fetch all playlists on component mount
onMounted(async () => {
  try {
    loading.value = true;
    playlists.value = await invoke('get_all_playlists');
  } catch (e) {
    error.value = `Failed to load playlists: ${e}`;
    console.error('Error loading playlists:', e);
  } finally {
    loading.value = false;
  }
});

// Reload playlists
const reloadPlaylists = async () => {
  try {
    loading.value = true;
    error.value = null;
    playlists.value = await invoke('get_all_playlists');
  } catch (e) {
    error.value = `Failed to reload playlists: ${e}`;
    console.error('Error reloading playlists:', e);
  } finally {
    loading.value = false;
  }
};

// Format timestamp to readable date
const formatDate = (timestamp: number): string => {
  return new Date(timestamp * 1000).toLocaleDateString();
};

// Expose reload method to parent
defineExpose({ reloadPlaylists });
</script>

<template>
  <div class="playlist-list">
    <h2>Playlists</h2>

    <div v-if="loading" class="loading">Loading playlists...</div>
    <div v-else-if="error" class="error">{{ error }}</div>
    <div v-else-if="playlists.length === 0" class="empty">No playlists yet. Create one!</div>

    <div v-else class="playlists">
      <div
        v-for="playlist in playlists"
        :key="playlist.id"
        class="playlist-item"
      >
        <div class="playlist-info">
          <div class="playlist-title">{{ playlist.title }}</div>
          <div class="playlist-meta">
            <span>{{ playlist.trackCount }} tracks</span>
            <span v-if="playlist.description"> • {{ playlist.description }}</span>
          </div>
          <div class="playlist-date">
            Created: {{ formatDate(playlist.createdAt) }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.playlist-list {
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

.playlists {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.playlist-item {
  padding: 1rem;
  background-color: #ffffff;
  border-radius: 8px;
  border: 1px solid #e0e0e0;
  transition: all 0.2s;
  cursor: pointer;
}

.playlist-item:hover {
  background-color: #f5f5f5;
  border-color: #396cd8;
  transform: translateY(-2px);
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.1);
}

.playlist-info {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.playlist-title {
  font-weight: 600;
  font-size: 1.1rem;
  color: #0f0f0f;
  margin-bottom: 0.25rem;
}

.playlist-meta {
  font-size: 0.875rem;
  color: #666;
}

.playlist-date {
  font-size: 0.75rem;
  color: #999;
  margin-top: 0.5rem;
}

@media (prefers-color-scheme: dark) {
  h2 {
    color: #f6f6f6;
  }

  .playlist-item {
    background-color: #1a1a1a;
    border-color: #333;
  }

  .playlist-item:hover {
    background-color: #252525;
    border-color: #24c8db;
  }

  .playlist-title {
    color: #f6f6f6;
  }

  .playlist-meta {
    color: #aaa;
  }

  .playlist-date {
    color: #777;
  }
}
</style>
