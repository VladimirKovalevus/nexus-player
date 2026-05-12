<template>
  <div :class="$style.home">
    <PlaylistCreateModal
      :is-open="isPlaylistModalOpen"
      @close="handleModalClose"
      @created="handlePlaylistCreated"
    />

    <MediaCardSection
      title="Плейлисты"
      :items="playlistItems"
      card-type="playlist"
      show-action
      @item-click="handlePlaylistClick"
      @item-play="handlePlaylistPlay"
    >
      <template #header-extra>
        <a
          v-if="playlists.length > 0"
          href="#"
          :class="$style.showAll"
          @click.prevent="showAllPlaylists"
        >
          Показать всё <AppIcon name="arrow-right" :size="14" />
        </a>
      </template>

      <template #empty-extra>
        <div :class="$style.emptyHint">Добавьте новый плейлист</div>
        <button :class="$style.addButton" @click="handleAddPlaylist">
          + Добавить плейлист
        </button>
      </template>

      <template #menu="{ close, item }">
        <button
          :class="$style.menuItem"
          @click.stop="
            () => {
              handlePlaylistCache(item);
              close();
            }
          "
        >
          Кэшировать плейлист
        </button>
        <button
          :class="[$style.menuItem, $style.danger]"
          @click.stop="
            () => {
              handlePlaylistDelete(item);
              close();
            }
          "
        >
          Удалить
        </button>
      </template>
    </MediaCardSection>

    <TrackListSection
      title="Треки"
      :tracks="tracks"
      :active-track-id="audioStore.currentTrack?.id ?? null"
      show-action
      @track-click="handleTrackClick"
      @track-play="handleTrackPlay"
      @track-refresh="fetchTracks"
    >
      <template #header-extra>
        <div
          v-if="tracks.length > 0"
          :class="$style.filter"
          @click="toggleSort"
        >
          <span>{{ currentSortLabel }}</span>
          <AppIcon
            name="chevron-up"
            :size="10"
            :class="[
              $style.arrow,
              sortDirection === 'desc' && $style.arrowDown,
            ]"
          />
        </div>
      </template>

      <template #above-list>
        <div v-if="pageCursor.totalPages > 1" :class="$style.pagination">
          <button
            :class="$style.pageBtn"
            :disabled="pageCursor.currentPage === 0"
            @click="goToPage(pageCursor.currentPage - 1)"
          >
            <AppIcon name="chevron-left" :size="14" />
          </button>
          <span :class="$style.pageInfo">
            {{ pageCursor.currentPage + 1 }} / {{ pageCursor.totalPages }}
          </span>
          <button
            :class="$style.pageBtn"
            :disabled="pageCursor.currentPage >= pageCursor.totalPages - 1"
            @click="goToPage(pageCursor.currentPage + 1)"
          >
            <AppIcon name="chevron-right" :size="14" />
          </button>
        </div>
      </template>

      <template #empty-extra>
        <div :class="$style.emptyHint">Добавьте новый трек</div>
        <button :class="$style.addButton" @click="handleAddTrack">
          + Добавить трек
        </button>
      </template>
    </TrackListSection>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, inject, computed, watch } from 'vue';
import { invoke } from '@/utils/providerAuth';
import MediaCardSection, { type MediaCardItem } from '@/components/common/MediaCardSection.vue';
import TrackListSection from '@/components/common/TrackListSection.vue';
import PlaylistCreateModal from '@/components/PlaylistCreateModal.vue';
import AppIcon from '@/components/common/AppIcon.vue';
import type {
  PlaylistDto,
  TrackDto,
  PlaylistWithItemsDto,
  LibraryPageDto,
} from '@/types/types';
import { useNotificationStore } from '@/stores/notifications';
import { useAudioStore } from '@/stores/audio';

const audioStore = useAudioStore();
const emit = defineEmits<{
  'navigate-playlist': [playlistId: number];
}>();

const refreshQueue = inject<() => void>('refreshQueue', () => {});
const notifications = useNotificationStore();

const playlists = ref<PlaylistDto[]>([]);
const tracks = ref<TrackDto[]>([]);
const loading = ref(false);

const playlistItems = computed<MediaCardItem[]>(() =>
  playlists.value.map((playlist) => ({
    id: playlist.id,
    title: playlist.title,
    subtitle: playlist.description || 'Unknown',
    thumbnail: playlist.thumbnail,
    trackCount: playlist.trackCount,
  })),
);

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
const isPlaylistModalOpen = ref(false);

const sortField = ref<'default' | 'title' | 'artist' | 'added_at'>('default');
const sortDirection = ref<'asc' | 'desc'>('asc');

const PAGE_SIZE = 50;
const pageCursor = ref({ currentPage: 0, totalPages: 0 });
const isPageLoading = ref(false);

const sortLabels = {
  default: 'По умолчанию',
  title: 'Название',
  artist: 'Артист',
  added_at: 'Дата',
};

const currentSortLabel = computed(() => sortLabels[sortField.value]);

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

const fetchPlaylists = async () => {
  try {
    const playlistDtos = await invoke<PlaylistDto[]>('get_all_playlists');
    playlists.value = playlistDtos;
  } catch (e) {
    console.error('Failed to fetch playlists:', e);
    error.value = `Failed to load playlists: ${e}`;
  }
};

const fetchTracks = async () => {
  try {
    const order = { [sortField.value]: sortDirection.value };
    const trackDtos = await invoke<TrackDto[]>('get_all_tracks', { order });
    tracks.value = trackDtos;
  } catch (e) {
    console.error('Failed to fetch tracks:', e);
    error.value = `Failed to load tracks: ${e}`;
  }
};

const loadData = async () => {
  loading.value = true;
  error.value = null;

  await Promise.all([fetchPlaylists(), fetchTracks()]);

  loading.value = false;
};

onMounted(() => {
  loadData();
});

const handleAddPlaylist = () => {
  isPlaylistModalOpen.value = true;
};

const handlePlaylistCreated = async () => {
  await fetchPlaylists();
};

const handleModalClose = () => {
  isPlaylistModalOpen.value = false;
};

const showAllPlaylists = () => {
  console.log('Show all playlists clicked');
};

const handlePlaylistClick = (item: MediaCardItem) => {
  emit('navigate-playlist', item.id);
};

const handlePlaylistPlay = async (item: MediaCardItem) => {
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

const handlePlaylistCache = async (item: MediaCardItem) => {
  notifications.addNotification(
    `Начинаю кэширование плейлиста "${item.title}"`,
    'info',
  );
  try {
    const pwi = await invoke<PlaylistWithItemsDto | null>(
      'get_playlist_with_tracks',
      { playlistId: item.id },
    );
    if (pwi && pwi.items) {
      let successCount = 0;
      let failCount = 0;

      for (const pwiItem of pwi.items) {
        if (pwiItem.itemType === 'track') {
          try {
            await invoke('save_track_to_file', { track: pwiItem.track });
            successCount++;
          } catch (e) {
            console.error(`Failed to cache track ${pwiItem.track.title}:`, e);
            failCount++;
          }
        }
      }

      if (failCount === 0) {
        notifications.addNotification(
          `Плейлист "${item.title}" поставлен на загрузку (${successCount} треков)`,
          'success',
        );
      } else {
        notifications.addNotification(
          `Загрузка "${item.title}": ${successCount} ок, ${failCount} ошибок`,
          'warning',
        );
      }
    }
  } catch (e) {
    console.error('Failed to cache playlist:', e);
    notifications.addNotification(
      `Ошибка при получении данных плейлиста: ${e}`,
      'error',
    );
  }
};

const handlePlaylistDelete = async (item: MediaCardItem) => {
  try {
    await invoke('delete_playlist', { playlistId: item.id });
    await fetchPlaylists();
  } catch (e) {
    console.error('Failed to delete playlist:', e);
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
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.home {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.showAll {
  color: var(--color-text-secondary);
  text-decoration: none;
  font-size: $font-size-sm;
  transition: color $transition-fast;

  &:hover {
    color: var(--color-text-primary);
  }
}

.filter {
  @include flex-center;
  gap: $spacing-sm;
  color: var(--color-text-secondary);
  font-size: $font-size-sm;
  cursor: pointer;
  padding: $spacing-xs $spacing-sm;
  border-radius: $radius-sm;
  transition: all $transition-fast;
  user-select: none;

  &:hover {
    color: var(--color-text-primary);
    background-color: var(--color-bg-hover);
  }
}

.pagination {
  @include flex-center;
  gap: $spacing-sm;
  padding: $spacing-xs 0 $spacing-sm;
}

.pageBtn {
  width: 28px;
  height: 28px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-secondary);
  color: var(--color-text-primary);
  border-radius: $radius-sm;
  cursor: pointer;
  font-size: $font-size-base;
  @include flex-center;
  transition: all $transition-fast;

  &:hover:not(:disabled) {
    background: var(--color-bg-hover);
  }

  &:disabled {
    opacity: 0.35;
    cursor: default;
  }
}

.pageInfo {
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
  min-width: 60px;
  text-align: center;
}

.arrow {
  display: inline-block;
  font-size: 10px;
  transition: transform $transition-fast;
}

.arrowDown {
  transform: rotate(180deg);
}

.emptyHint {
  font-size: $font-size-base;
  color: var(--color-text-secondary);
  margin-bottom: $spacing-sm;
}

.addButton {
  padding: $spacing-md $spacing-xl;
  font-size: $font-size-base;
  font-weight: $font-weight-semibold;
  background: black;
  color: white;
  border: none;
  border-radius: $radius-lg;
  cursor: pointer;
  transition: all $transition-fast;
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 16px rgba(102, 126, 234, 0.4);
  }

  &:active {
    transform: translateY(0);
  }
}

.menuItem {
  display: block;
  width: 100%;
  padding: $spacing-sm $spacing-md;
  border: none;
  background: none;
  text-align: left;
  cursor: pointer;
  font-size: $font-size-sm;
  color: var(--color-text-primary);
  transition: background-color $transition-fast;

  &:hover {
    background-color: var(--color-bg-hover);
  }

  &.danger {
    color: #e74c3c;
  }
}
</style>
