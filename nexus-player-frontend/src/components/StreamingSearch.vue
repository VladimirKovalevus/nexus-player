<script setup lang="ts">
import { ref, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@/utils/providerAuth';
import type { SearchResult, StreamingTrack } from '@/types/types';
import MediaRow from '@/components/common/MediaRow.vue';
import TrackTableHeader from '@/components/common/TrackTableHeader.vue';
import AppIcon from '@/components/common/AppIcon.vue';
import PlaylistCreateSimpleModal from './PlaylistCreateSimpleModal.vue';
import { storeToRefs } from 'pinia';
import { useStreamingStore } from '@/stores/streaming';
import { useAudioStore } from '@/stores/audio';
import { useNotificationStore } from '@/stores/notifications';
import { usePlaylistActions } from '@/composables/usePlaylistActions';

const { t } = useI18n();
const store = useStreamingStore();
const audioStore = useAudioStore();
const notifications = useNotificationStore();
const { addContainerToPlaylist } = usePlaylistActions();
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
  error,
} = storeToRefs(store);

const loading = ref(false);
const isModalOpen = ref(false);
const isSavingPlaylist = ref(false);
const modalTargetIndex = ref<number | null>(null);
const playlistInitialData = ref<{ title?: string; description?: string; artworkUrl?: string }>({});

const providers = [
  { value: 'bandcamp', label: 'Bandcamp' },
  { value: 'soundcloud', label: 'SoundCloud' },
];

const contentTypes = [
  { value: null, key: 'all' },
  { value: 'Track', key: 'track' },
  { value: 'Playlist', key: 'playlist' },
  { value: 'Album', key: 'album' },
  { value: 'Channel', key: 'channel' },
];

const limits = [5, 10, 20, 50];

const emit = defineEmits<{
  refetch: [];
}>();

const mapContentType = (type: string) => type.charAt(0).toUpperCase() + type.slice(1);

const currentPage = computed(() => Math.floor(offset.value / limit.value) + 1);

const containerRow = (result: Exclude<SearchResult, StreamingTrack>) => {
  const common = { id: result.id, system: result.system, thumbnail: result.thumbnail ?? null };
  switch (result.type) {
    case 'album':
      return { ...common, kind: 'album' as const, title: result.title, artist: result.artist, trackCount: result.trackCount };
    case 'playlist':
      return { ...common, kind: 'playlist' as const, title: result.title, artist: result.creator, trackCount: result.trackCount };
    case 'channel':
      return { ...common, kind: 'channel' as const, title: result.name, artist: '' };
  }
};

const URL_REGEX = /^https?:\/\/.+/;

const submitSearch = async () => {
  const value = query.value.trim();
  if (!value) {
    error.value = t('search.emptyQuery');
    return;
  }

  if (URL_REGEX.test(value)) {
    await resolveDirectLink(value);
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
    offset.value = 0;
    activeQuery.value = url;
    activeProvider.value = selectedProvider.value;
    activeContentType.value = selectedContentType.value;
  } catch (e) {
    error.value = t('common.error', { error: String(e) });
    console.error('direct_link_resolve error:', e);
  } finally {
    loading.value = false;
  }
};

const search = async (newOffset: number) => {
  try {
    loading.value = true;
    error.value = null;
    offset.value = newOffset;
    searchResults.value = await invoke<SearchResult[]>('search_streaming', {
      provider: activeProvider.value,
      contentType: activeContentType.value?.toLowerCase(),
      query: activeQuery.value,
      limit: limit.value,
      offset: offset.value,
    });
  } catch (e) {
    error.value = t('search.failed', { error: String(e) });
    console.error('Search error:', e);
  } finally {
    loading.value = false;
  }
};

const nextPage = () => search(offset.value + limit.value);
const prevPage = () => search(Math.max(0, offset.value - limit.value));

const handleResultPlay = async (result: SearchResult, index: number) => {
  // The backend queues channels as a placeholder that cannot play.
  if (result.type === 'channel') {
    notifications.addNotification(t('search.channelUnsupported'), 'info');
    return;
  }

  audioStore.startLoading();
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
        play: true,
      });
    } catch (fallbackError) {
      console.error('Failed to play search result:', fallbackError);
      audioStore.stopLoading();
      error.value = t('common.error', { error: String(fallbackError) });
      return;
    }
  }
  emit('refetch');
};

const handleContainerCreatePlaylist = (index: number) => {
  const result = searchResults.value[index];
  if (!result || result.type === 'track' || result.type === 'channel') return;

  playlistInitialData.value = {
    title: result.title,
    description: result.type === 'album' ? result.artist : result.creator,
    artworkUrl: result.thumbnail || '',
  };
  modalTargetIndex.value = index;
  isModalOpen.value = true;
};

const handleContainerAddToPlaylist = async (index: number, playlistId: number) => {
  const result = searchResults.value[index];
  if (!result || result.type === 'track' || result.type === 'channel') return;

  await addContainerToPlaylist(playlistId, result.title, () =>
    invoke<number>('create_playlist_from_container', {
      provider: result.system || activeProvider.value,
      contentType: mapContentType(result.type),
      id: result.id,
      title: result.title,
      description: result.type === 'album' ? result.artist : result.creator,
      artworkUrl: result.thumbnail ?? null,
    }),
  );
  emit('refetch');
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
      provider: result.system || activeProvider.value,
      contentType: mapContentType(result.type),
      id: result.id,
      title: data.title,
      description: data.description,
      artworkUrl: data.artworkUrl,
    });
    isModalOpen.value = false;
    emit('refetch');
  } catch (e) {
    error.value = t('common.error', { error: e instanceof Error ? e.message : String(e) });
    console.error('Failed to create playlist:', e);
  } finally {
    isSavingPlaylist.value = false;
  }
};
</script>

<template>
  <div :class="$style.search">
    <div class="view-header">
      <h1 class="view-title">{{ $t('nav.search') }}</h1>
      <span v-if="searchResults.length > 0" class="view-count">
        {{ $t('search.count', { count: searchResults.length }) }}
      </span>
      <span class="view-spacer" />
      <template v-if="searchResults.length > 0 && (offset > 0 || searchResults.length >= limit)">
        <button
          class="deck-btn"
          :title="$t('common.previousPage')"
          :aria-label="$t('common.previousPage')"
          :disabled="offset === 0 || loading"
          @click="prevPage"
        >
          <AppIcon name="chevron-left" :size="13" />
        </button>
        <span class="view-count">{{ $t('search.page', { page: currentPage }) }}</span>
        <button
          class="deck-btn"
          :title="$t('common.nextPage')"
          :aria-label="$t('common.nextPage')"
          :disabled="searchResults.length < limit || loading"
          @click="nextPage"
        >
          <AppIcon name="chevron-right" :size="13" />
        </button>
      </template>
    </div>

    <form :class="$style.box" @submit.prevent="submitSearch">
      <AppIcon name="search" :size="16" />
      <input
        v-model="query"
        type="text"
        :placeholder="$t('search.placeholder')"
        :aria-label="$t('nav.search')"
        autocomplete="off"
        spellcheck="false"
      />
      <button type="submit" class="deck-btn deck-btn--primary" :disabled="loading">
        {{ loading ? $t('search.searching') : $t('search.submit') }}
      </button>
    </form>

    <div :class="$style.filters">
      <div :class="$style.filter">
        <span class="section-label">{{ $t('search.provider') }}</span>
        <div class="seg">
          <button
            v-for="provider in providers"
            :key="provider.value"
            type="button"
            :aria-pressed="selectedProvider === provider.value"
            @click="selectedProvider = provider.value"
          >
            {{ provider.label }}
          </button>
        </div>
      </div>
      <div :class="$style.filter">
        <span class="section-label">{{ $t('search.type') }}</span>
        <div class="seg">
          <button
            v-for="type in contentTypes"
            :key="type.key"
            type="button"
            :aria-pressed="selectedContentType === type.value"
            @click="selectedContentType = type.value"
          >
            {{ $t(`search.types.${type.key}`) }}
          </button>
        </div>
      </div>
      <div :class="$style.filter">
        <span class="section-label">{{ $t('search.limit') }}</span>
        <div class="seg">
          <button
            v-for="value in limits"
            :key="value"
            type="button"
            :aria-pressed="limit === value"
            @click="limit = value"
          >
            {{ value }}
          </button>
        </div>
      </div>
    </div>

    <p v-if="error" :class="$style.error">{{ error }}</p>

    <div v-if="searchResults.length > 0" class="track-table track-table--kinds">
      <TrackTableHeader kind-column />
      <MediaRow
        v-for="(result, index) in searchResults"
        :key="index"
        :type="result.type === 'track' ? 'track' : 'album'"
        :track="result.type === 'track' ? result : undefined"
        :album="result.type === 'track' ? undefined : containerRow(result)"
        :index="offset + index"
        :kind-label="$t(`search.types.${result.type}`)"
        context="search"
        :active="result.type === 'track' && result.id !== undefined && audioStore.currentTrack?.id === result.id"
        :container-mode="result.type !== 'track'"
        @click="handleResultPlay(result, index)"
        @play="handleResultPlay(result, index)"
        @refresh="emit('refetch')"
        @container-create-playlist="handleContainerCreatePlaylist(index)"
        @container-add-to-playlist="(playlistId) => handleContainerAddToPlaylist(index, playlistId)"
        @container-remove="handleContainerRemove(index)"
      />
    </div>

    <p v-else-if="!loading && activeQuery" :class="$style.message">
      {{ $t('search.noResults', { query: activeQuery }) }}
    </p>
    <p v-else-if="!loading" :class="$style.message">{{ $t('search.hint') }}</p>

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

.search {
  height: 100%;
  overflow-y: auto;
}

.box {
  display: flex;
  align-items: center;
  gap: 9px;
  height: 42px;
  margin: 10px 12px 8px;
  padding: 0 7px 0 12px;
  background: var(--screen);
  border-radius: var(--radius-sm);
  color: var(--led);

  input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: none;
    color: var(--led);
    font: 500 var(--font-size-lg) $font-family-mono;

    &::placeholder {
      color: var(--screen-muted);
    }
  }
}

.filters {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 18px;
  padding: 0 12px 10px;
  border-bottom: 1px solid var(--line);
}

.filter {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px 8px;
}

.error {
  padding: 10px 12px;
  color: var(--danger);
  overflow-wrap: anywhere;
}

.message {
  padding: 24px 12px;
  text-align: center;
  color: var(--muted);
  overflow-wrap: anywhere;
}

@media (max-width: $mobile-breakpoint) {
  .filter {
    flex-wrap: nowrap;
    width: 100%;

    > :first-child {
      flex: none;
      width: 62px;
    }
  }
}
</style>
