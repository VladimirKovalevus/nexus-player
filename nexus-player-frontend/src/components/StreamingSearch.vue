<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface StreamingTrack {
  type: 'track';
  title: string;
  artist: string;
  durationSecs?: number;
  thumbnail?: string;
  url: string;
}

interface StreamingPlaylist {
  type: 'playlist';
  id: string;
  title: string;
  creator: string;
  trackCount: number;
  thumbnail?: string;
  url: string;
}

interface StreamingAlbum {
  type: 'album';
  id: string;
  title: string;
  artist: string;
  trackCount: number;
  year?: number;
  thumbnail?: string;
  url: string;
}

interface StreamingChannel {
  type: 'channel';
  id: string;
  name: string;
  subscriberCount?: number;
  thumbnail?: string;
  url: string;
}

type SearchResult =
  | StreamingTrack
  | StreamingPlaylist
  | StreamingAlbum
  | StreamingChannel;

const query = ref('');
const selectedProvider = ref('youtube');
const selectedContentType = ref<string | null>(null);
const limit = ref(10);

const searchResults = ref<SearchResult[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

const providers = [
  { value: 'youtube', label: 'YouTube' },
  { value: 'soundcloud', label: 'SoundCloud' },
  { value: 'bandcamp', label: 'Bandcamp' },
  { value: 'ytdlp', label: 'YT-DLP (All)' },
];

const contentTypes = [
  { value: null, label: 'All' },
  { value: 'track', label: 'Track' },
  { value: 'playlist', label: 'Playlist' },
  { value: 'album', label: 'Album' },
  { value: 'channel', label: 'Channel' },
];

const search = async () => {
  if (!query.value.trim()) {
    error.value = 'Please enter a search query';
    return;
  }

  try {
    loading.value = true;
    error.value = null;

    const results = await invoke<SearchResult[]>('search_streaming', {
      provider: selectedProvider.value,
      contentType: selectedContentType.value,
      query: query.value,
      limit: limit.value,
    });

    searchResults.value = results;
  } catch (e) {
    error.value = `Search failed: ${e}`;
    console.error('Search error:', e);
  } finally {
    loading.value = false;
  }
};

const formatDuration = (seconds?: number): string => {
  if (!seconds) return '--:--';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

const formatNumber = (num?: number): string => {
  if (!num) return 'N/A';
  if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
  if (num >= 1000) return `${(num / 1000).toFixed(1)}K`;
  return num.toString();
};

const emit = defineEmits<{
  refetch: [];
}>();
const handleResultClick = async (result: SearchResult) => {
  console.log('Clicked result:', result);
  await invoke('handle_streaming_track', {
    searchResult: result,
    strategy: 'cache',
  });
  emit('refetch');
  // TODO(human): Implement result click handler
  // For tracks: Add to queue or play immediately
  // For playlists/albums: Expand and show tracks
  // For channels: Navigate to channel view
};
</script>

<template>
  <div class="streaming-search">
    <h2>Streaming Search</h2>

    <!-- Search Controls -->
    <div class="search-controls">
      <div class="search-input-group">
        <input
          v-model="query"
          type="text"
          placeholder="Search for music..."
          class="search-input"
          @keyup.enter="search"
        />
        <button @click="search" :disabled="loading" class="search-btn">
          {{ loading ? 'Searching...' : 'Search' }}
        </button>
      </div>

      <div class="filters">
        <div class="filter-group">
          <label for="provider-select">Provider:</label>
          <select
            id="provider-select"
            v-model="selectedProvider"
            class="filter-select"
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
            class="filter-select"
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
          <select id="limit-select" v-model="limit" class="filter-select">
            <option :value="5">5</option>
            <option :value="10">10</option>
            <option :value="20">20</option>
            <option :value="50">50</option>
          </select>
        </div>
      </div>
    </div>

    <!-- Error Message -->
    <div v-if="error" class="error-message">{{ error }}</div>

    <!-- Search Results -->
    <div v-if="searchResults.length > 0" class="results">
      <h3>Results ({{ searchResults.length }})</h3>
      <div class="results-list">
        <div
          v-for="(result, index) in searchResults"
          :key="index"
          class="result-item"
          :class="`result-${result.type}`"
          @click="handleResultClick(result)"
        >
          <!-- Thumbnail -->
          <div class="result-thumbnail">
            <img
              v-if="result.thumbnail"
              :src="result.thumbnail"
              :alt="result.type === 'channel' ? result.name : result.title"
            />
            <div v-else class="thumbnail-placeholder">
              {{ result.type === 'channel' ? '👤' : '🎵' }}
            </div>
          </div>

          <!-- Track Result -->
          <div v-if="result.type === 'track'" class="result-content">
            <div class="result-title">{{ result.title }}</div>
            <div class="result-meta">{{ result.artist }}</div>
            <div class="result-meta">
              {{ formatDuration(result.durationSecs) }}
            </div>
          </div>

          <!-- Playlist Result -->
          <div v-else-if="result.type === 'playlist'" class="result-content">
            <div class="result-title">📂 {{ result.title }}</div>
            <div class="result-meta">by {{ result.creator }}</div>
            <div class="result-meta">{{ result.trackCount }} tracks</div>
          </div>

          <!-- Album Result -->
          <div v-else-if="result.type === 'album'" class="result-content">
            <div class="result-title">💿 {{ result.title }}</div>
            <div class="result-meta">{{ result.artist }}</div>
            <div class="result-meta">
              {{ result.trackCount }} tracks
              <span v-if="result.year"> • {{ result.year }}</span>
            </div>
          </div>

          <!-- Channel Result -->
          <div v-else-if="result.type === 'channel'" class="result-content">
            <div class="result-title">📺 {{ result.name }}</div>
            <div class="result-meta" v-if="result.subscriberCount">
              {{ formatNumber(result.subscriberCount) }} subscribers
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- No Results -->
    <div
      v-else-if="!loading && searchResults.length === 0 && query"
      class="no-results"
    >
      No results found for "{{ query }}"
    </div>
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
  font-size: 16px;
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
  font-size: 16px;
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

.filter-select {
  padding: 8px 12px;
  border: 2px solid #ddd;
  border-radius: 6px;
  font-size: 14px;
  cursor: pointer;
  outline: none;
  transition: border-color 0.2s;
}

.filter-select:focus {
  border-color: #4caf50;
}

.error-message {
  padding: 12px;
  background-color: #ffebee;
  color: #c62828;
  border-radius: 6px;
  margin-bottom: 20px;
}

.results-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.result-item {
  display: flex;
  gap: 15px;
  padding: 15px;
  background-color: #f5f5f5;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.result-item:hover {
  background-color: #e8f5e9;
  transform: translateX(5px);
}

.result-thumbnail {
  width: 80px;
  height: 80px;
  flex-shrink: 0;
  border-radius: 6px;
  overflow: hidden;
  background-color: #ddd;
  display: flex;
  align-items: center;
  justify-content: center;
}

.result-thumbnail img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.thumbnail-placeholder {
  font-size: 32px;
}

.result-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
}

.result-title {
  font-weight: 600;
  font-size: 16px;
  color: #333;
}

.result-meta {
  font-size: 14px;
  color: #666;
}

.result-track {
  border-left: 4px solid #4caf50;
}

.result-playlist {
  border-left: 4px solid #2196f3;
}

.result-album {
  border-left: 4px solid #ff9800;
}

.result-channel {
  border-left: 4px solid #9c27b0;
}

.no-results {
  text-align: center;
  padding: 40px;
  color: #999;
  font-size: 18px;
}
</style>
