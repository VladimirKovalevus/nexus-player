<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import AppIcon from '@/components/common/AppIcon.vue';

const emit = defineEmits<{
  'playlist-loaded-to-queue': []
}>();

// Playlists will be fetched from backend
const playlists = ref<any[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const openMenuId = ref<number | null>(null);

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

// Load playlist to queue
const loadPlaylistToQueue = async (playlistId: number) => {
  try {
    await invoke('load_playlist_to_queue', { playlistId });
    emit('playlist-loaded-to-queue');
  } catch (e) {
    console.error('Error loading playlist to queue:', e);
  }
};

// Format timestamp to readable date
const formatDate = (timestamp: number): string => {
  return new Date(timestamp * 1000).toLocaleDateString();
};

// Toggle menu for a playlist
const toggleMenu = (playlistId: number, event: Event) => {
  event.stopPropagation();
  openMenuId.value = openMenuId.value === playlistId ? null : playlistId;
};

// Close menu when clicking outside
const closeMenu = () => {
  openMenuId.value = null;
};

// Delete playlist
const deletePlaylist = async (playlistId: number, event: Event) => {
  event.stopPropagation();
  try {
    await invoke('delete_playlist', { playlistId });
    openMenuId.value = null;
    await reloadPlaylists();
  } catch (e) {
    console.error('Error deleting playlist:', e);
  }
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

    <div v-else class="playlists" @click="closeMenu">
      <div
        v-for="playlist in playlists"
        :key="playlist.id"
        class="playlist-item"
        @click="loadPlaylistToQueue(playlist.id)"
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
        <div class="playlist-actions">
          <button class="menu-button" @click="toggleMenu(playlist.id, $event)">
            <AppIcon name="more-horizontal" :size="16" class="dots" style="transform: rotate(90deg)" />
          </button>
          <div v-if="openMenuId === playlist.id" class="dropdown-menu">
            <button class="menu-item delete" @click="deletePlaylist(playlist.id, $event)">
              Delete
            </button>
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
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
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

.playlist-actions {
  position: relative;
  flex-shrink: 0;
}

.menu-button {
  background: none;
  border: none;
  cursor: pointer;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  clip-path: none !important;
  color: #666;
  font-size: 1.25rem;
  line-height: 1;
}

.menu-button:hover {
  background-color: rgba(0, 0, 0, 0.1);
}

.dots {
  font-weight: bold;
}

.dropdown-menu {
  position: absolute;
  top: 100%;
  right: 0;
  background: #fff;
  border: 1px solid #e0e0e0;
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  z-index: 100;
  min-width: 120px;
  overflow: hidden;
}

.menu-item {
  display: block;
  width: 100%;
  padding: 0.625rem 1rem;
  border: none;
  background: none;
  text-align: left;
  cursor: pointer;
  font-size: 0.875rem;
  color: #333;
}

.menu-item:hover {
  background-color: #f5f5f5;
}

.menu-item.delete {
  color: #e74c3c;
}

.menu-item.delete:hover {
  background-color: #ffe6e6;
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

  .menu-button {
    color: #aaa;
  }

  .menu-button:hover {
    background-color: rgba(255, 255, 255, 0.1);
  }

  .dropdown-menu {
    background: #2a2a2a;
    border-color: #444;
  }

  .menu-item {
    color: #f6f6f6;
  }

  .menu-item:hover {
    background-color: #333;
  }

  .menu-item.delete {
    color: #ff6b6b;
  }

  .menu-item.delete:hover {
    background-color: #3d2020;
  }
}
</style>
