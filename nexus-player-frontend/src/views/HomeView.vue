<template>
  <div ref="scrollEl" class="home-scroll" :class="$style.home">
    <div>
      <div class="view-header">
        <h1 class="view-title">{{ $t('nav.home') }}</h1>
        <span class="view-count">{{ $t('home.trackCount', { count: tracks.length }) }}</span>
        <span class="view-spacer" />
        <button class="deck-btn" @click="openNewPlaylist">+ {{ $t('home.addPlaylist') }}</button>
      </div>

      <p v-if="error" :class="$style.error">{{ error }}</p>

      <section v-if="showAllPlaylists" data-testid="home-playlists-grid">
        <div class="section-head">
          <button class="deck-btn" data-testid="home-playlists-back" @click="closeAllPlaylists">
            <AppIcon name="chevron-left" :size="13" />
            {{ $t('common.back') }}
          </button>
          <span class="view-spacer" />
          <span class="view-count">{{ $t('home.allPlaylists', { count: playlistsStore.playlists.length }) }}</span>
        </div>
        <MediaTileSection
          :title="$t('home.playlists')"
          :items="playlistItems"
          :empty-title="$t('home.noPlaylists')"
          placeholder-icon="playlist"
          @item-click="handlePlaylistClick"
          @item-play="handlePlaylistPlay"
        >
          <template #menu="{ close, item }">
            <MenuItem icon="download-tray" @click="cachePlaylist(item.id, item.title); close()">
              {{ $t('home.cachePlaylist') }}
            </MenuItem>
            <MenuItem icon="trash" danger @click="deletePlaylist(item.id, item.title); close()">
              {{ $t('common.delete') }}
            </MenuItem>
          </template>
        </MediaTileSection>
      </section>

      <section v-if="!showAllPlaylists && isMobile && playlistsStore.playlists.length > 0" data-testid="home-playlists">
        <div class="section-head">
          <span class="section-label">{{ $t('home.playlists') }}</span>
          <span class="view-spacer" />
          <button class="deck-btn" data-testid="home-playlists-all-link" @click="openAllPlaylists">
            {{ $t('home.allPlaylists', { count: playlistsStore.playlists.length }) }} ›
          </button>
        </div>
        <div :class="$style.row">
          <div
            v-for="item in rowItems"
            :key="item.id"
            data-testid="home-playlist-tile"
            :class="$style.cell"
          >
            <MediaTile
              :title="item.title"
              :subtitle="item.subtitle"
              :thumbnail="item.thumbnail"
              placeholder-icon="playlist"
              @click="handlePlaylistClick(item)"
              @play="handlePlaylistPlay(item)"
            >
              <template #menu="{ close }">
                <MenuItem icon="download-tray" @click="cachePlaylist(item.id, item.title); close()">
                  {{ $t('home.cachePlaylist') }}
                </MenuItem>
                <MenuItem icon="trash" danger @click="deletePlaylist(item.id, item.title); close()">
                  {{ $t('common.delete') }}
                </MenuItem>
              </template>
            </MediaTile>
          </div>
          <button
            v-if="hasMore"
            type="button"
            data-testid="home-playlists-all-tile"
            :class="[$style.cell, $style.allTile]"
            @click="openAllPlaylists"
          >
            {{ $t('home.allPlaylists', { count: playlistsStore.playlists.length }) }} ›
          </button>
        </div>
      </section>

      <TrackListSection
        v-if="!showAllPlaylists"
        :title="$t('home.tracks')"
        :tracks="tracks"
        :active-track-id="audioStore.currentTrack?.id ?? null"
        :empty-title="$t('home.noTracks')"
        scroll-selector=".home-scroll"
        sticky-head
        @track-click="handleTrackClick"
        @track-play="handleTrackPlay"
        @track-refresh="fetchTracks"
      >
        <template #header-extra>
          <div v-if="pageCursor.totalPages > 1" :class="$style.pagination">
            <button
              class="deck-btn"
              :title="$t('common.previousPage')"
              :aria-label="$t('common.previousPage')"
              :disabled="pageCursor.currentPage === 0 || isPageLoading"
              @click="goToPage(pageCursor.currentPage - 1)"
            >
              <AppIcon name="chevron-left" :size="13" />
            </button>
            <span class="view-count">{{ pageCursor.currentPage + 1 }} / {{ pageCursor.totalPages }}</span>
            <button
              class="deck-btn"
              :title="$t('common.nextPage')"
              :aria-label="$t('common.nextPage')"
              :disabled="pageCursor.currentPage >= pageCursor.totalPages - 1 || isPageLoading"
              @click="goToPage(pageCursor.currentPage + 1)"
            >
              <AppIcon name="chevron-right" :size="13" />
            </button>
          </div>
          <button v-if="tracks.length > 0" class="deck-btn" @click="toggleSort">
            {{ $t(`home.sort.${sortField}`) }}
            <AppIcon :name="sortDirection === 'asc' ? 'arrow-up' : 'arrow-down'" :size="11" />
          </button>
          <button class="deck-btn" @click="handleAddTrack">+ {{ $t('home.addTrack') }}</button>
        </template>
      </TrackListSection>
    </div>
  </div>
</template>

<script setup lang="ts">
import { errorMessage } from '@/utils/errorMessage';
import { ref, onMounted, onBeforeUnmount, inject, computed, watch, nextTick } from 'vue';
import { useI18n } from 'vue-i18n';
import { useMediaQuery } from '@vueuse/core';
import { invoke } from '@/utils/providerAuth';
import AppIcon from '@/components/common/AppIcon.vue';
import MenuItem from '@/components/common/MenuItem.vue';
import MediaTile from '@/components/common/MediaTile.vue';
import MediaTileSection, { type MediaTileItem } from '@/components/common/MediaTileSection.vue';
import TrackListSection from '@/components/common/TrackListSection.vue';
import { useAudioStore } from '@/stores/audio';
import { usePlaylistsStore } from '@/stores/playlists';
import { usePlaylistActions } from '@/composables/usePlaylistActions';
import type { TrackDto, LibraryPageDto } from '@/types/types';

const { t } = useI18n();
const audioStore = useAudioStore();
const emit = defineEmits<{
  'navigate-playlist': [playlistId: number];
}>();

const refreshQueue = inject<() => void>('refreshQueue', () => {});
const openNewPlaylist = inject<() => void>('openNewPlaylist', () => {});
const playlistsStore = usePlaylistsStore();
const { cachePlaylist, deletePlaylist } = usePlaylistActions();

const isMobile = useMediaQuery('(max-width: 640px)');
const showAllPlaylists = ref(false);
const ROW_LIMIT = 12;
const scrollEl = ref<HTMLElement | null>(null);
let savedScroll = 0;

const onPopState = () => {
  if (showAllPlaylists.value) hideGrid();
};

const hideGrid = () => {
  showAllPlaylists.value = false;
  window.removeEventListener('popstate', onPopState);
  const top = savedScroll;
  nextTick(() => {
    if (scrollEl.value) scrollEl.value.scrollTop = top;
  });
};

const openAllPlaylists = () => {
  if (showAllPlaylists.value) return;
  savedScroll = scrollEl.value?.scrollTop ?? 0;
  history.pushState({ homeAllPlaylists: true }, '');
  window.addEventListener('popstate', onPopState);
  showAllPlaylists.value = true;
};

const closeAllPlaylists = () => {
  if (showAllPlaylists.value) history.back();
};

watch(isMobile, (mobile) => {
  if (!mobile && showAllPlaylists.value) {
    hideGrid();
    history.back();
  }
});

onBeforeUnmount(() => {
  if (!showAllPlaylists.value) return;
  window.removeEventListener('popstate', onPopState);
  showAllPlaylists.value = false;
  history.back();
});

const tracks = ref<TrackDto[]>([]);
const loading = ref(false);

const playlistItems = computed<MediaTileItem[]>(() =>
  playlistsStore.playlists.map((playlist) => ({
    id: playlist.id,
    title: playlist.title,
    subtitle: t('home.trackCount', { count: playlist.trackCount }),
    thumbnail: playlist.thumbnail,
  })),
);

const rowItems = computed(() => playlistItems.value.slice(0, ROW_LIMIT));
const hasMore = computed(() => playlistItems.value.length > ROW_LIMIT);

watch(
  () => audioStore.lastUnavailableTrackId,
  (trackId) => {
    if (trackId === null) return;
    const track = tracks.value.find((t) => t.id === trackId);
    if (track) track.unavailable = true;
  },
);

watch(
  () => audioStore.lastCachedTrack,
  (info) => {
    if (!info || !info.success) return;
    const track = tracks.value.find((t) => t.id === info.trackId);
    if (track) track.state = { type: 'cached' };
  },
);
const error = ref<string | null>(null);

const sortField = ref<'default' | 'title' | 'artist' | 'added_at'>('default');
const sortDirection = ref<'asc' | 'desc'>('asc');

const PAGE_SIZE = 50;
const pageCursor = ref({ currentPage: 0, totalPages: 0 });
const isPageLoading = ref(false);

const toggleSort = () => {
  if (sortDirection.value === 'asc') {
    sortDirection.value = 'desc';
  } else {
    sortDirection.value = 'asc';
    const fields: (typeof sortField.value)[] = [
      'default',
      'title',
      'artist',
      'added_at',
    ];
    const currentIndex = fields.indexOf(sortField.value);
    sortField.value = fields[(currentIndex + 1) % fields.length];
  }
  fetchTracks();
};

const fetchTracks = async () => {
  try {
    const order = { [sortField.value]: sortDirection.value };
    const trackDtos = await invoke<TrackDto[]>('get_all_tracks', { order });
    tracks.value = trackDtos;
  } catch (e) {
    console.error('Failed to fetch tracks:', e);
    error.value = t('common.error', { error: errorMessage(e) });
  }
};

const loadData = async () => {
  loading.value = true;
  error.value = null;

  await Promise.all([playlistsStore.fetch(), fetchTracks()]);

  loading.value = false;
};

onMounted(() => {
  loadData();
});

const handlePlaylistClick = (item: MediaTileItem) => {
  emit('navigate-playlist', item.id);
};

const handlePlaylistPlay = async (item: MediaTileItem) => {
  audioStore.startLoading();
  try {
    await invoke('load_playlist_and_play', {
      playlistId: item.id,
      startIndex: 0,
    });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play playlist:', e);
    audioStore.stopLoading();
  }
};

const handleAddTrack = async () => {
  try {
    const filePath = await invoke<string | null>('select_audio_file');
    if (filePath) {
      await invoke('add_track_from_file', { path: filePath });
      await fetchTracks();
    }
  } catch (e) {
    console.error('Failed to add track:', e);
  }
};

const playVisibleTracks = async (track: TrackDto, visibleTracks: TrackDto[]) => {
  audioStore.startLoading();
  try {
    const trackIds = visibleTracks.map((t) => t.id);
    await invoke('load_and_play_library_tracks', { trackIds, selectedId: track.id });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play filtered tracks:', e);
    audioStore.stopLoading();
  }
};

const handleTrackClick = async (
  track: TrackDto,
  visibleTracks: TrackDto[],
  isFiltered: boolean,
) => {
  if (isFiltered) {
    await playVisibleTracks(track, visibleTracks);
    return;
  }

  isPageLoading.value = true;
  audioStore.startLoading();
  try {
    const dto = await invoke<LibraryPageDto>(
      'load_and_play_library_page_at_track',
      {
        trackId: track.id,
        pageSize: PAGE_SIZE,
        order: { [sortField.value]: sortDirection.value },
      },
    );
    pageCursor.value = {
      currentPage: dto.currentPage,
      totalPages: dto.totalPages,
    };
    refreshQueue();
  } catch (e) {
    console.error('Failed to load library page:', e);
    audioStore.stopLoading();
  } finally {
    isPageLoading.value = false;
  }
};

const goToPage = async (page: number) => {
  isPageLoading.value = true;
  audioStore.startLoading();
  try {
    const dto = await invoke<LibraryPageDto>('load_and_play_library_page', {
      page,
      pageSize: PAGE_SIZE,
      order: { [sortField.value]: sortDirection.value },
    });
    pageCursor.value = {
      currentPage: dto.currentPage,
      totalPages: dto.totalPages,
    };
    refreshQueue();
  } catch (e) {
    console.error('Failed to load page:', e);
    audioStore.stopLoading();
  } finally {
    isPageLoading.value = false;
  }
};

const handleTrackPlay = async (
  track: TrackDto,
  visibleTracks: TrackDto[],
  isFiltered: boolean,
) => {
  if (isFiltered) {
    await playVisibleTracks(track, visibleTracks);
    return;
  }

  audioStore.startLoading();
  try {
    await invoke('play_library_track', { trackId: track.id });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play track:', e);
    audioStore.stopLoading();
  }
};
</script>

<style module lang="scss">
.home {
  height: 100%;
  overflow-y: auto;
}

.error {
  padding: 10px 12px;
  color: var(--danger);
}

.row {
  display: flex;
  gap: 1px;
  overflow-x: auto;
  scroll-snap-type: x proximity;
  background: var(--line-soft);
  border-block: 1px solid var(--line);
}

.cell {
  flex: none;
  width: 220px;
  display: flex;
  scroll-snap-align: start;

  > * {
    flex: 1;
    min-width: 0;
  }
}

.allTile {
  align-items: center;
  justify-content: center;
  border: 0;
  background: var(--bg-table);
  color: var(--text);
  font-weight: 600;
  cursor: pointer;
}

.pagination {
  display: flex;
  align-items: center;
  gap: 6px;
}
</style>
