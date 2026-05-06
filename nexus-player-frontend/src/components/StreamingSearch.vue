<script setup lang="ts">
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { SearchResult } from '@/types/types';
import MediaCard from '@/components/common/MediaCard.vue';
import AppIcon from '@/components/common/AppIcon.vue';
import PlaylistCreateSimpleModal from './PlaylistCreateSimpleModal.vue';
import { storeToRefs } from 'pinia';
import { useStreamingStore } from '@/stores/streaming';
import { useAudioStore } from '@/stores/audio';

const store = useStreamingStore();
const audioStore = useAudioStore();
const {
  query,
  activeQuery,
  selectedProvider,
  activeProvider,
  selectedContentType,
  activeContentType,
  limit,
  offset,
  searchResults,
  error
} = storeToRefs(store);

const loading = ref(false);
const isModalOpen = ref(false);
const isSavingPlaylist = ref(false);
const modalTargetIndex = ref<number | null>(null);
const playlistInitialData = ref<{ title?: string; description?: string; artworkUrl?: string }>({});
const expandedContainers = ref<Set<number>>(new Set());

const providers = [
  { value: 'bandcamp', label: 'Bandcamp' },
  { value: 'soundcloud', label: 'SoundCloud' },
];

const contentTypes = [
  { value: null, label: 'All' },
  { value: 'Track', label: 'Track' },
  { value: 'Playlist', label: 'Playlist' },
  { value: 'Album', label: 'Album' },
  { value: 'Channel', label: 'Channel' },
];

const emit = defineEmits<{
  refetch: [];
}>();

const mapContentType = (type: string) => {
  if (!type) return null;
  return type.charAt(0).toUpperCase() + type.slice(1);
};

const currentPage = computed(() => Math.floor(offset.value / limit.value) + 1);

const URL_REGEX = /^https?:\/\/.+/;

const search_q = async () => {
  if (!query.value.trim()) {
    error.value = 'Please enter a search query';
    return;
  }

  if (URL_REGEX.test(query.value.trim())) {
    await resolveDirectLink(query.value.trim());
    return;
  }

  activeQuery.value = query.value;
  activeProvider.value = selectedProvider.value;
  activeContentType.value = selectedContentType.value;
  search(0);
};

const resolveDirectLink = async (url: string) => {
  try {
    loading.value = true;
    error.value = null;
    const result = await invoke<SearchResult>('direct_link_resolve', {
      provider: selectedProvider.value,
      url,
    });
    searchResults.value = [result];
    expandedContainers.value.clear();
    activeQuery.value = url;
    activeProvider.value = selectedProvider.value;
    activeContentType.value = selectedContentType.value;
  } catch (e) {
    error.value = `Ошибка: ${e}`;
    console.error('direct_link_resolve error:', e);
  } finally {
    loading.value = false;
  }
};

const search = async (newOffset = 0) => {
  const currentQ = activeQuery.value;
  const currentProvider = activeProvider.value;
  const currentContentType = activeContentType.value;

  if (!currentQ.trim()) {
    error.value = 'Please enter a search query';
    return;
  }

  try {
    loading.value = true;
    error.value = null;
    offset.value = newOffset;
    console.log({
      provider: currentProvider,
      contentType: currentContentType,
      query: currentQ,
      limit: limit.value,
      offset: offset.value,
    },"OFFSET")

    const results = await invoke<SearchResult[]>('search_streaming', {
      provider: currentProvider,
      contentType: currentContentType?.toLowerCase(),
      query: currentQ,
      limit: limit.value,
      offset: offset.value,
    });
    console.log(results,"AAA")

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

const handleResultClick = async (result: SearchResult, index: number, play = false) => {
  console.log('Clicked result:', result, 'at index:', index);
  try {
    await invoke('handle_search', {
      system: activeProvider.value,
      contentType: activeContentType.value?.toLowerCase(),
      query: activeQuery.value,
      limit: limit.value,
      offset: offset.value,
      selectedIndex: index,
    });
  } catch (e) {
    console.warn('handle_search failed, falling back:', e);
    try {
      await invoke('handle_streaming_track', {
        searchResult: result,
        strategy: 'cache',
        query: activeQuery.value,
        searchType: activeContentType.value,
        play,
      });
    } catch (innerE) {
        console.error('fallback handle_streaming_track failed:', innerE);
        throw innerE;
    }
  }
  emit('refetch');
};

const handleResultPlay = async (result: SearchResult, index: number) => {
  audioStore.startLoading();
  try {
    await handleResultClick(result, index, true);
  } catch (e) {
    console.error('Failed to play search result:', e);
    audioStore.stopLoading();
  }
};

// Loads a streaming container into queue and expands it to get its tracks.
// Side effect: replaces the current playback queue.
const getContainerTracks = async (result: SearchResult): Promise<any[] | null> => {
  try {
    await invoke('handle_streaming_track', {
      searchResult: result,
      strategy: 'cache',
      searchType: activeContentType.value,
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
  const result = searchResults.value[index];
  if (result) {
    let description = '';
    if (result.type === 'album') {
      description = result.artist || '';
    } else if (result.type === 'playlist') {
      description = result.creator || '';
    }

    if (result.description) {
      description = description ? `${description} (${result.description})` : result.description;
    }

    playlistInitialData.value = {
      title: result.title,
      description: description,
      artworkUrl: result.thumbnail || '',
    };
  }
  modalTargetIndex.value = index;
  isModalOpen.value = true;
};

const handleContainerAddToPlaylist = async (index: number, playlistId: number) => {
  const result = searchResults.value[index];
  if (!result || result.type === 'track' || result.type === 'channel') return;

  try {
    const title = result.title;
    const description = result.type === 'album' ? result.artist : result.creator;

    const childId = await invoke<number>('create_playlist_from_container', {
      provider: result.source?.system || activeProvider.value,
      contentType: mapContentType(result.type),
      id: result.id,
      title,
      description,
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

const handleCreatePlaylist = async (data: { title: string; description: string | null; artworkUrl: string | null }) => {
  if (isSavingPlaylist.value || modalTargetIndex.value === null) return;

  const result = searchResults.value[modalTargetIndex.value];
  if (!result || result.type === 'track' || result.type === 'channel') return;

  isSavingPlaylist.value = true;
  error.value = null;
  try {
    await invoke<number>('create_playlist_from_container', {
      provider: result.source?.system || activeProvider.value,
      contentType: mapContentType(result.type),
      id: result.id,
      title: data.title,
      description: data.description,
      artworkUrl: data.artworkUrl,
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
  <div :class="$style.streamingSearch">
    <h2 :class="$style.title">Поиск</h2>

    <div :class="$style.searchControls">
      <div :class="$style.searchInputGroup">
        <input
          v-model="query"
          type="text"
          placeholder="Search for music..."
          :class="$style.searchInput"
          @keyup.enter="search_q"
        />
        <button @click="search_q" :disabled="loading" :class="$style.searchBtn">
          {{ loading ? 'Searching...' : 'Search' }}
        </button>
      </div>

      <div :class="$style.filters">
        <div :class="$style.filterGroup">
          <label for="provider-select">Provider:</label>
          <select
            id="provider-select"
            v-model="selectedProvider"
            :class="$style.providerSelect"
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

        <div :class="$style.filterGroup">
          <label for="content-type-select">Type:</label>
          <select
            id="content-type-select"
            v-model="selectedContentType"
            :class="$style.providerSelect"
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

        <div :class="$style.filterGroup">
          <label for="limit-select">Limit:</label>
          <select id="limit-select" v-model="limit" :class="$style.providerSelect">
            <option :value="5">5</option>
            <option :value="10">10</option>
            <option :value="20">20</option>
            <option :value="50">50</option>
          </select>
        </div>
      </div>
    </div>

    <div v-if="error" :class="$style.errorMessage">{{ error }}</div>

    <div v-if="searchResults.length > 0" :class="$style.results">
      <div :class="$style.resultsHeader">
        <button v-if="offset > 0" @click="prevPage" :class="$style.paginationBtn" title="Previous page">
          <AppIcon name="arrow-left" :size="16" />
        </button>
        <h3 :class="$style.resultsTitle">Results ({{ searchResults.length }}) - Page {{ currentPage }}</h3>
        <button v-if="searchResults.length >= limit" @click="nextPage" :class="$style.paginationBtn" title="Next page">
          <AppIcon name="arrow-right" :size="16" />
        </button>
      </div>
      <div :class="$style.resultsList">
        <MediaCard
          v-for="(result, index) in searchResults"
          :key="index"
          :type="result.type === 'track' ? 'track' : result.type === 'playlist' ? 'playlist' : 'album'"
          :track="result.type === 'track' ? result : undefined"
          :playlist="result.type === 'playlist' ? result : undefined"
          :album="result.type === 'album' || result.type === 'channel' ? result : undefined"
          show-action
          :active="audioStore.currentTrack?.id === result.id"
          :container-mode="result.type !== 'track'"
          :is-expanded="isContainerExpanded(index)"
          @click=" handleResultClick(result, index) "
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
      :class="$style.noResults"
    >
      No results found for "{{ query }}"
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

.streamingSearch {
  padding: $spacing-xl;
  max-width: 1200px;
  margin: 0 auto;
}

.title {
  margin-bottom: $spacing-lg;
  color: var(--color-text-primary);
  font-size: $font-size-2xl;
}

.resultsTitle {
  margin-bottom: 0;
  color: var(--color-text-secondary);
  font-size: $font-size-lg;
}

.searchControls {
  margin-bottom: $spacing-2xl;
}

.searchInputGroup {
  display: flex;
  gap: $spacing-md;
  margin-bottom: $spacing-lg;
}

.searchInput {
  flex: 1;
  padding: $spacing-md $spacing-lg;
  font-size: $font-size-lg;
  background-color: var(--search-input-bg);
  color: var(--search-input-color);
  border: 2px solid var(--color-border);
  border-radius: $radius-md;
  outline: none;
  transition: border-color $transition-fast;

  &:focus {
    border-color: var(--color-accent-primary);
  }

  &::placeholder {
    color: var(--search-input-color);
    opacity: 0.6;
  }
}

.searchBtn {
  padding: $spacing-md $spacing-2xl;
  font-size: $font-size-lg;
  background-color: var(--color-accent-primary);
  color: white;
  border: none;
  border-radius: $radius-md;
  cursor: pointer;
  transition: background-color $transition-fast;
  min-width: 120px;

  &:hover:not(:disabled) {
    background-color: var(--color-accent-hover);
  }

  &:disabled {
    background-color: var(--color-bg-hover);
    color: var(--color-text-muted);
    cursor: not-allowed;
  }
}

.filters {
  display: flex;
  gap: $spacing-xl;
  flex-wrap: wrap;
}

.filterGroup {
  display: flex;
  align-items: start;
  flex-direction: column;

  label {
    font-weight: $font-weight-medium;
    color: var(--color-text-secondary);
    font-size: $font-size-sm;
  }
}

.providerSelect {
  padding: $spacing-sm $spacing-md;
  appearance: none;
  -webkit-appearance: none;
  -moz-appearance: none;
  background: var(--search-input-bg);
  color: var(--search-input-color);
  border: 2px solid var(--color-border);
  border-radius: $radius-sm;
  font-size: $font-size-sm;
  cursor: pointer;
  outline: none;
  transition: border-color $transition-fast;

  &:focus {
    border-color: var(--color-accent-primary);
  }

  option {
    background-color: var(--search-input-bg);
    color: var(--search-input-color);
  }
}

.errorMessage {
  padding: $spacing-md;
  background-color: rgba(231, 76, 60, 0.1);
  color: #e74c3c;
  border-radius: $radius-sm;
  margin-bottom: $spacing-lg;
  border: 1px solid rgba(231, 76, 60, 0.2);
}

.resultsHeader {
  display: flex;
  align-items: center;
  gap: $spacing-md;
  margin-bottom: $spacing-lg;
}

.paginationBtn {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-sm;
  padding: $spacing-xs $spacing-md;
  cursor: pointer;
  font-size: $font-size-lg;
  color: var(--color-text-secondary);
  transition: all $transition-fast;
  display: flex;
  align-items: center;
  justify-content: center;

  &:hover {
    background-color: var(--color-bg-hover);
    border-color: var(--color-accent-primary);
    color: var(--color-accent-primary);
  }
}

.resultsList {
  display: flex;
  flex-direction: column;
  gap: $spacing-md;
}

.noResults {
  text-align: center;
  padding: $spacing-3xl;
  color: var(--color-text-muted);
  font-size: $font-size-lg;
}
</style>
