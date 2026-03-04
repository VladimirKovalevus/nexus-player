<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { SearchResult } from '@/types/types';
import MediaCard from '@/components/common/MediaCard.vue';
import PlaylistCreateSimpleModal from './PlaylistCreateSimpleModal.vue';
import { storeToRefs } from 'pinia';
import { useStreamingStore } from '@/stores/streaming';

const store = useStreamingStore();
const { query, selectedProvider, selectedContentType, limit, offset, searchResults, error } = storeToRefs(store);

const loading = ref(false);
const isModalOpen = ref(false);
const isSavingPlaylist = ref(false);
const modalTargetIndex = ref<number | null>(null);
const expandedContainers = ref<Set<number>>(new Set());

const providers = [
  { value: 'bandcamp', label: 'Bandcamp' },
  { value: 'soundcloud', label: 'SoundCloud' },
  { value: 'vk', label: 'VK' },
];

const contentTypes = [
  { value: null, label: 'All' },
  { value: 'track', label: 'Track' },
  { value: 'playlist', label: 'Playlist' },
  { value: 'album', label: 'Album' },
  { value: 'channel', label: 'Channel' },
];

const emit = defineEmits<{
  refetch: [];
}>();

const search_q = ()=>{
    search(0)
}
const search = async (newOffset = 0) => {
  if (!query.value.trim()) {
    error.value = 'Please enter a search query';
    return;
  }

  try {
    loading.value = true;
    error.value = null;
    offset.value = newOffset;
    console.log(offset.value,"OFFSET")

    const results = await invoke<SearchResult[]>('search_streaming', {
      provider: selectedProvider.value,
      contentType: selectedContentType.value,
      query: query.value,
      limit: limit.value,
      offset: offset.value,
    });

    searchResults.value = results;
    expandedContainers.value.clear();
  } catch (e) {
    error.value = `Search failed: ${e}`;
    console.error('Search error:', e);
  } finally {
    loading.value = false;
  }
};

const nextPage = () => {
  search(offset.value + limit.value);
};

const prevPage = () => {
  search(Math.max(0, offset.value - limit.value));
};

const isContainerExpanded = (index: number) => expandedContainers.value.has(index);

const toggleContainer = (index: number) => {
  if (expandedContainers.value.has(index)) {
    expandedContainers.value.delete(index);
  } else {
    expandedContainers.value.add(index);
  }
};

const handleResultClick = async (result: SearchResult, index: number) => {
  console.log('Clicked result:', result, 'at index:', index);
  try {
    await invoke('handle_search', {
      system: selectedProvider.value,
      contentType: selectedContentType.value,
      query: query.value,
      limit: limit.value,
      offset: offset.value,
      selectedIndex: index,
    });
  } catch (e) {
    console.warn('handle_search failed, falling back:', e);
    await invoke('handle_streaming_track', {
      searchResult: result,
      strategy: 'cache',
      query: query.value,
      searchType: selectedContentType.value,
    });
  }
  emit('refetch');
};

const handleResultPlay = async (result: SearchResult, index: number) => {
  await invoke('clear_queue');
  await handleResultClick(result, index);
};

// Loads a streaming container into queue and expands it to get its tracks.
// Side effect: replaces the current playback queue.
const getContainerTracks = async (result: SearchResult): Promise<any[] | null> => {
  try {
    await invoke('handle_streaming_track', {
      searchResult: result,
      strategy: 'cache',
      searchType: selectedContentType.value,
    });
    const queueState = await invoke<any>('get_queue_state');
    const entry = queueState.entries?.[0];
    if (!entry) return null;
    if (entry.entryType === 'container' && !entry.tracks) {
      const expanded = await invoke<any>('expand_queue_container', { itemIndex: 0 });
      return expanded.entries?.[0]?.tracks ?? null;
    }
    return entry.tracks ?? null;
  } catch (e) {
    console.error('Failed to get container tracks:', e);
    return null;
  }
};

const handleContainerCreatePlaylist = (index: number) => {
  modalTargetIndex.value = index;
  isModalOpen.value = true;
};

const handleContainerAddToPlaylist = async (index: number, playlistId: number) => {
  const result = searchResults.value[index];
  if (!result || result.type === 'track' || result.type === 'channel') return;

  try {
    const tracks = await getContainerTracks(result);
    if (!tracks) return;

    const title = result.title;
    const description = result.type === 'album' ? result.artist : result.creator;

    const childId = await invoke<number>('create_playlist_with_tracks', {
      request: { title, description, tracks },
    });
    await invoke('add_playlist_to_playlist', {
      parentPlaylistId: playlistId,
      childPlaylistId: childId,
    });
    emit('refetch');
  } catch (e) {
    console.error('Failed to add container to playlist:', e);
  }
};

const handleContainerRemove = (index: number) => {
  searchResults.value.splice(index, 1);
};

const handleCreatePlaylist = async (name: string) => {
  if (isSavingPlaylist.value || modalTargetIndex.value === null) return;

  const result = searchResults.value[modalTargetIndex.value];
  if (!result || result.type === 'track' || result.type === 'channel') return;

  isSavingPlaylist.value = true;
  error.value = null;
  try {
    const tracks = await getContainerTracks(result);
    if (!tracks) {
      error.value = 'Не удалось загрузить треки';
      return;
    }

    const description = result.type === 'album' ? result.artist : result.creator;

    await invoke<number>('create_playlist_with_tracks', {
      request: { title: name, description, tracks },
    });
    isModalOpen.value = false;
    emit('refetch');
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e);
    error.value = `Ошибка: ${errorMessage}`;
    console.error('Failed to create playlist:', e);
  } finally {
    isSavingPlaylist.value = false;
  }
};
</script>

<template>
  <div class="streaming-search">
    <h2>Streaming Search</h2>

    <div class="search-controls">
      <div class="search-input-group">
        <input
          v-model="query"
          type="text"
          placeholder="Search for music..."
          class="search-input"
          @keyup.enter="search_q"
        />
        <button @click="search_q" :disabled="loading" class="search-btn">
          {{ loading ? 'Searching...' : 'Search' }}
        </button>
      </div>

      <div class="filters">
        <div class="filter-group">
          <label for="provider-select">Provider:</label>
          <select
            id="provider-select"
            v-model="selectedProvider"
            class="provider-select"
          >
            <option
              v-for="provider in providers"
              :key="provider.value"
              :value="provider.value"
            >
              {{ provider.label }}
            </option>
          </select>
        </div>

        <div class="filter-group">
          <label for="content-type-select">Type:</label>
          <select
            id="content-type-select"
            v-model="selectedContentType"
            class="provider-select"
          >
            <option
              v-for="type in contentTypes"
              :key="type.value ?? 'all'"
              :value="type.value"
            >
              {{ type.label }}
            </option>
          </select>
        </div>

        <div class="filter-group">
          <label for="limit-select">Limit:</label>
          <select id="limit-select" v-model="limit" class="provider-select">
            <option :value="5">5</option>
            <option :value="10">10</option>
            <option :value="20">20</option>
            <option :value="50">50</option>
          </select>
        </div>
      </div>
    </div>

    <div v-if="error" class="error-message">{{ error }}</div>

    <div v-if="searchResults.length > 0" class="results">
      <div class="results-header">
        <button v-if="offset > 0" @click="prevPage" class="pagination-btn" title="Previous page">
          ←
        </button>
        <h3>Results ({{ searchResults.length }})</h3>
        <button v-if="searchResults.length >= limit" @click="nextPage" class="pagination-btn" title="Next page">
          →
        </button>
      </div>
      <div class="results-list">
        <MediaCard
          v-for="(result, index) in searchResults"
          :key="index"
          :type="result.type === 'track' ? 'track' : result.type === 'playlist' ? 'playlist' : 'album'"
          :track="result.type === 'track' ? result : undefined"
          :playlist="result.type === 'playlist' ? result : undefined"
          :album="result.type === 'album' || result.type === 'channel' ? result : undefined"
          show-action
          :container-mode="result.type !== 'track'"
          :is-expanded="isContainerExpanded(index)"
          @click="result.type === 'track' ? handleResultClick(result, index) : toggleContainer(index)"
          @play="handleResultPlay(result, index)"
          @refresh="search"
          @toggle-expand="toggleContainer(index)"
          @container-create-playlist="handleContainerCreatePlaylist(index)"
          @container-add-to-playlist="(plId) => handleContainerAddToPlaylist(index, plId)"
          @container-remove="handleContainerRemove(index)"
        />
      </div>
    </div>

    <div
      v-else-if="!loading && searchResults.length === 0 && query"
      class="no-results"
    >
      No results found for "{{ query }}"
    </div>

    <PlaylistCreateSimpleModal
      :is-open="isModalOpen"
      :is-loading="isSavingPlaylist"
      @close="isModalOpen = false"
      @save="handleCreatePlaylist"
    />
  </div>
</template>

<style scoped>
.streaming-search {
  padding: 20px;
  max-width: 1200px;
  margin: 0 auto;
}

h2 {
  margin-bottom: 20px;
  color: #333;
}

h3 {
  margin-bottom: 15px;
  color: #555;
}

.search-controls {
  margin-bottom: 30px;
}

.search-input-group {
  display: flex;
  gap: 10px;
  margin-bottom: 15px;
}

.search-input {
  flex: 1;
  padding: 12px 16px;
  font-size: 36px;
  border: 2px solid #ddd;
  border-radius: 8px;
  outline: none;
  transition: border-color 0.2s;
}

.search-input:focus {
  border-color: #4caf50;
}

.search-btn {
  padding: 12px 32px;
  font-size: 36px;
  background-color: #4caf50;
  color: white;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.2s;
  min-width: 120px;
}

.search-btn:hover:not(:disabled) {
  background-color: #45a049;
}

.search-btn:disabled {
  background-color: #ccc;
  cursor: not-allowed;
}

.filters {
  display: flex;
  gap: 20px;
  flex-wrap: wrap;
}

.filter-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

.filter-group label {
  font-weight: 500;
  color: #666;
}

.provider-select {
  padding: 8px 12px;
  border: 2px solid #ddd;
  border-radius: 6px;
  font-size: 14px;
  cursor: pointer;
  outline: none;
  transition: border-color 0.2s;
}

.provider-select:focus {
  border-color: #4caf50;
}

.error-message {
  padding: 12px;
  background-color: #ffebee;
  color: #c62828;
  border-radius: 6px;
  margin-bottom: 20px;
}

.results-header {
  display: flex;
  align-items: center;
  gap: 15px;
  margin-bottom: 15px;
}

.results-header h3 {
  margin-bottom: 0;
}

.pagination-btn {
  background: none;
  border: 1px solid #ddd;
  border-radius: 4px;
  padding: 4px 12px;
  cursor: pointer;
  font-size: 20px;
  color: #666;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.pagination-btn:hover {
  background-color: #f5f5f5;
  border-color: #4caf50;
  color: #4caf50;
}

.results-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.no-results {
  text-align: center;
  padding: 40px;
  color: #999;
  font-size: 18px;
}
</style>
