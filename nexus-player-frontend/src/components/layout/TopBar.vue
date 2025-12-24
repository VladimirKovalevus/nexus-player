<template>
  <header :class="$style.topbar">
    <div ref="searchContainerRef" :class="$style.searchContainer">
      <div :class="$style.search">
        <span :class="$style.searchIcon">🔍</span>
        <input
          type="text"
          :class="$style.searchInput"
          placeholder="Поиск музыки (YouTube, SoundCloud, Bandcamp...)"
          v-model="searchQuery"
          @keyup.enter="handleSearch"
          @focus="handleInputFocus"
        />
        <button
          v-if="searchQuery"
          :class="$style.clearButton"
          @click="clearSearch"
        >
          ✕
        </button>
      </div>

      <div v-if="showFilters" :class="$style.filtersDropdown">
        <div :class="$style.filterRow">
          <label>Provider:</label>
          <select v-model="selectedProvider" :class="$style.filterSelect">
            <option value="bandcamp">Bandcamp</option>
            <option value="youtube">YouTube</option>
            <option value="soundcloud">SoundCloud</option>
            <option value="ytdlp">YT-DLP (All)</option>
          </select>
        </div>

        <div :class="$style.filterRow">
          <label>Type:</label>
          <select v-model="selectedContentType" :class="$style.filterSelect">
            <option :value="null">All</option>
            <option value="track">Track</option>
            <option value="playlist">Playlist</option>
            <option value="album">Album</option>
            <option value="channel">Channel</option>
          </select>
        </div>

        <div :class="$style.filterRow">
          <label>Limit:</label>
          <select v-model="limit" :class="$style.filterSelect">
            <option :value="5">5</option>
            <option :value="10">10</option>
            <option :value="20">20</option>
          </select>
        </div>
      </div>

      <div
        v-if="showResults && searchResults.length > 0"
        :class="$style.resultsDropdown"
      >
        <div :class="$style.resultsHeader">
          <span>{{ searchResults.length }} results</span>
          <button @click="closeResults" :class="$style.closeButton">✕</button>
        </div>
        <div :class="$style.resultsList">
          <div
            v-for="(result, index) in searchResults"
            :key="index"
            :class="[$style.resultItem, $style[`result-${result.type}`]]"
            @click="handleResultClick(result)"
          >
            <div :class="$style.resultThumbnail">
              <img
                v-if="result.thumbnail"
                :src="result.thumbnail"
                :alt="result.type === 'channel' ? result.name : result.title"
              />
              <div v-else :class="$style.thumbnailPlaceholder">
                {{ result.type === 'channel' ? '👤' : '🎵' }}
              </div>
            </div>

            <div :class="$style.resultContent">
              <div :class="$style.resultTitle">
                <template v-if="result.type === 'playlist'">📂</template>
                <template v-else-if="result.type === 'album'">💿</template>
                <template v-else-if="result.type === 'channel'">📺</template>
                {{ result.type === 'channel' ? result.name : result.title }}
              </div>
              <div :class="$style.resultMeta">
                <template v-if="result.type === 'track'">
                  {{ result.artist }} •
                  {{ formatDuration(result.durationSecs) }}
                </template>
                <template v-else-if="result.type === 'playlist'">
                  by {{ result.creator }} • {{ result.trackCount }} tracks
                </template>
                <template v-else-if="result.type === 'album'">
                  {{ result.artist }} • {{ result.trackCount }} tracks
                </template>
                <template v-else-if="result.type === 'channel'">
                  {{ formatNumber(result.subscriberCount) }} subscribers
                </template>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-if="loading" :class="$style.loadingMessage">Searching...</div>
      <div v-if="error" :class="$style.errorMessage">{{ error }}</div>
    </div>

    <div :class="$style.user">
      <button :class="$style.userButton">
        <span :class="$style.userAvatar">👤</span>
        <span :class="$style.userName">CoNo_Oavka</span>
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { onClickOutside } from '@vueuse/core';
import type { SearchResult } from '@/types/types';

const searchQuery = ref('');
const selectedProvider = ref('bandcamp');
const selectedContentType = ref<string | null>(null);
const limit = ref(10);

const searchResults = ref<SearchResult[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const showResults = ref(false);
const showFilters = ref(false);

const searchContainerRef = ref<HTMLElement | null>(null);

const emit = defineEmits<{
  refetch: [];
}>();

// Close dropdowns when clicking outside
onClickOutside(searchContainerRef, () => {
  showFilters.value = false;
  showResults.value = false;
});

const handleSearch = async () => {
  if (!searchQuery.value.trim()) {
    error.value = 'Please enter a search query';
    return;
  }

  try {
    loading.value = true;
    error.value = null;
    showResults.value = false;

    const results = await invoke<SearchResult[]>('search_streaming', {
      provider: selectedProvider.value,
      contentType: selectedContentType.value,
      query: searchQuery.value,
      limit: limit.value,
    });

    searchResults.value = results;
    showResults.value = true;
    showFilters.value = false;
  } catch (e) {
    error.value = `Search failed: ${e}`;
    console.error('Search error:', e);
  } finally {
    loading.value = false;
  }
};

const handleResultClick = async (result: SearchResult) => {
  console.log('Clicked result:', result);
  try {
    await invoke('handle_streaming_track', {
      searchResult: result,
      strategy: 'cache',
    });
    emit('refetch');
    closeResults();
  } catch (e) {
    console.error('Failed to handle streaming track:', e);
  }
};

const clearSearch = () => {
  searchQuery.value = '';
  searchResults.value = [];
  showResults.value = false;
  error.value = null;
};

const closeResults = () => {
  showResults.value = false;
};

const handleInputFocus = () => {
  showFilters.value = true;
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
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.topbar {
  height: $topbar-height;
  background-color: var(--color-bg-primary);
  border-bottom: 1px solid var(--color-border);
  padding: 0 $spacing-xl;
  @include flex-between;
  position: relative;
}

.searchContainer {
  position: relative;
  flex: 1;
  max-width: 600px;
}

.search {
  position: relative;
  width: 100%;
  @include flex-start;
  gap: $spacing-sm;
  background-color: var(--color-bg-secondary);
  border-radius: $radius-lg;
  padding: $spacing-sm $spacing-md;
  transition: all $transition-fast;

  &:focus-within {
    background-color: var(--color-bg-hover);
    box-shadow: 0 0 0 2px var(--color-accent-primary);
  }
}

.searchIcon {
  font-size: $font-size-lg;
  color: var(--color-text-secondary);
}

.searchInput {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: var(--color-text-primary);
  font-size: $font-size-base;
  font-family: $font-family-primary;

  &::placeholder {
    color: var(--color-text-muted);
  }
}

.clearButton {
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: $spacing-xs;
  border-radius: 50%;
  font-size: $font-size-base;
  transition: all $transition-fast;

  &:hover {
    background-color: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
}

// Filters Dropdown
.filtersDropdown {
  position: absolute;
  top: calc(100% + $spacing-sm);
  left: 0;
  right: 0;
  background-color: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  padding: $spacing-md;
  z-index: 1000;
  display: flex;
  flex-direction: column;
  gap: $spacing-sm;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.filterRow {
  @include flex-start;
  gap: $spacing-sm;

  label {
    font-size: $font-size-sm;
    color: var(--color-text-secondary);
    min-width: 70px;
  }
}

.filterSelect {
  flex: 1;
  background-color: var(--color-bg-primary);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border);
  border-radius: $radius-sm;
  padding: $spacing-xs $spacing-sm;
  font-size: $font-size-sm;
  cursor: pointer;

  &:focus {
    outline: none;
    border-color: var(--color-accent-primary);
  }
}

// Results Dropdown
.resultsDropdown {
  position: absolute;
  top: calc(100% + $spacing-sm);
  left: 0;
  right: 0;
  max-height: 60vh;
  background-color: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  z-index: 1000;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.resultsHeader {
  @include flex-between;
  padding: $spacing-sm $spacing-md;
  border-bottom: 1px solid var(--color-border);
  background-color: var(--color-bg-primary);

  span {
    font-size: $font-size-sm;
    color: var(--color-text-secondary);
    font-weight: $font-weight-medium;
  }
}

.closeButton {
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: $spacing-xs;
  font-size: $font-size-base;
  border-radius: 50%;
  transition: all $transition-fast;

  &:hover {
    background-color: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
}

.resultsList {
  overflow-y: auto;
  @include custom-scrollbar(
    var(--color-border-light),
    var(--color-bg-secondary)
  );
}

.resultItem {
  @include flex-start;
  gap: $spacing-md;
  padding: $spacing-md;
  cursor: pointer;
  transition: background-color $transition-fast;
  border-left: 3px solid transparent;

  &:hover {
    background-color: var(--color-bg-hover);
  }

  &.result-track {
    border-left-color: #4caf50;
  }

  &.result-playlist {
    border-left-color: #2196f3;
  }

  &.result-album {
    border-left-color: #ff9800;
  }

  &.result-channel {
    border-left-color: #9c27b0;
  }
}

.resultThumbnail {
  width: 48px;
  height: 48px;
  flex-shrink: 0;
  border-radius: $radius-sm;
  overflow: hidden;
  background-color: var(--color-bg-primary);
  @include flex-center;

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
}

.thumbnailPlaceholder {
  font-size: $font-size-xl;
}

.resultContent {
  flex: 1;
  min-width: 0;
}

.resultTitle {
  font-size: $font-size-base;
  font-weight: $font-weight-medium;
  color: var(--color-text-primary);
  @include text-ellipsis;
}

.resultMeta {
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
  @include text-ellipsis;
  margin-top: 2px;
}

.loadingMessage,
.errorMessage {
  position: absolute;
  top: calc(100% + $spacing-sm);
  left: 0;
  right: 0;
  padding: $spacing-sm $spacing-md;
  background-color: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  font-size: $font-size-sm;
  z-index: 1000;
}

.loadingMessage {
  color: var(--color-text-secondary);
}

.errorMessage {
  color: #f44336;
}

// User Section
.user {
  @include flex-start;
  gap: $spacing-md;
}

.userButton {
  @include flex-start;
  gap: $spacing-sm;
  background-color: var(--color-bg-secondary);
  border: none;
  border-radius: $radius-lg;
  padding: $spacing-sm $spacing-md;
  color: var(--color-text-primary);
  cursor: pointer;
  transition: background-color $transition-fast;

  &:hover {
    background-color: var(--color-bg-hover);
  }
}

.userAvatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: linear-gradient(
    135deg,
    var(--color-accent-primary) 0%,
    var(--color-accent-purple) 100%
  );
  @include flex-center;
  font-size: $font-size-lg;
}

.userName {
  font-weight: $font-weight-medium;
  color: var(--color-text-primary);
}
</style>
