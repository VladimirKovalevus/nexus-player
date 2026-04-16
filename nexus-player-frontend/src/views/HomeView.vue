<template>
  <div :class="$style.home">
    <PlaylistCreateModal
      :is-open="isPlaylistModalOpen"
      @close="handleModalClose"
      @created="handlePlaylistCreated"
    />

    <!-- Плейлисты: фиксированная верхняя зона -->
    <section :class="$style.playlistSection">
      <div :class="$style.sectionHeader">
        <h2 :class="$style.sectionTitle">Плейлисты</h2>
        <a
          v-if="playlists.length > 0"
          href="#"
          :class="$style.showAll"
          @click.prevent="showAllPlaylists"
        >
          Показать всё →
        </a>
      </div>

      <div v-if="playlists.length === 0" :class="$style.emptyState">
        <div :class="$style.emptyIcon">📂</div>
        <div :class="$style.emptyTitle">Данных нет</div>
        <div :class="$style.emptyHint">Добавьте новый плейлист</div>
        <button :class="$style.addButton" @click="handleAddPlaylist">
          + Добавить плейлист
        </button>
      </div>

      <div v-else :class="$style.playlistList">
        <VirtualHorizontalList
          :items="playlists"
          :item-width="180"
          :height="220"
        >
          <template #default="{ item: playlist }">
            <MediaCardCompact
              type="playlist"
              :title="playlist.title"
              :artist="playlist.description || 'Unknown'"
              :thumbnail="playlist.thumbnail || undefined"
              :track-count="playlist.trackCount"
              :playlist-id="playlist.id"
              show-action
              @click="handlePlaylistClick(playlist)"
              @play="handlePlaylistPlay(playlist)"
              @delete="handlePlaylistDelete(playlist)"
            >
              <template #menu="{ close }">
                <button
                  :class="$style.menuItem"
                  @click.stop="
                    () => {
                      handlePlaylistCache(playlist);
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
                      handlePlaylistDelete(playlist);
                      close();
                    }
                  "
                >
                  Удалить
                </button>
              </template>
            </MediaCardCompact>
          </template>
        </VirtualHorizontalList>
      </div>
    </section>

    <!-- Треки: занимает всё оставшееся место, прокручивается отдельно -->
    <section :class="$style.trackSection">
      <div :class="$style.sectionHeader">
        <h2 :class="$style.sectionTitle">Треки</h2>
        <div :class="$style.headerControls">
          <div
            v-if="tracks.length > 0"
            :class="$style.filter"
            @click="toggleSort"
          >
            <span>{{ currentSortLabel }}</span>
            <span
              :class="[
                $style.arrow,
                sortDirection === 'desc' && $style.arrowDown,
              ]"
              >▲</span
            >
          </div>
        </div>
      </div>

      <div v-if="pageCursor.totalPages > 1" :class="$style.pagination">
        <button
          :class="$style.pageBtn"
          :disabled="pageCursor.currentPage === 0"
          @click="goToPage(pageCursor.currentPage - 1)"
        >‹</button>
        <span :class="$style.pageInfo">
          {{ pageCursor.currentPage + 1 }} / {{ pageCursor.totalPages }}
        </span>
        <button
          :class="$style.pageBtn"
          :disabled="pageCursor.currentPage >= pageCursor.totalPages - 1"
          @click="goToPage(pageCursor.currentPage + 1)"
        >›</button>
      </div>

      <div v-if="tracks.length === 0" :class="$style.emptyState">
        <div :class="$style.emptyIcon">🎵</div>
        <div :class="$style.emptyTitle">Данных нет</div>
        <div :class="$style.emptyHint">Добавьте новый трек</div>
        <button :class="$style.addButton" @click="handleAddTrack">
          + Добавить трек
        </button>
      </div>

      <VirtualList
        v-else
        :class="$style.scroller"
        :items="tracks"
        :item-height="96"
        :overscan="8"
      >
        <template #default="{ item: track }">
          <MediaCard
            v-memo="[
              track.id,
              track.liked,
              audioStore.currentTrack?.id === track.id,
            ]"
            type="track"
            show-action
            :track="track"
            :active="audioStore.currentTrack?.id === track.id"
            @click="handleTrackClick(track)"
            @play="handleTrackPlay(track)"
            @refresh="fetchTracks"
          />
        </template>
      </VirtualList>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, inject, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCard from '@/components/common/MediaCard.vue';
import MediaCardCompact from '@/components/common/MediaCardCompact.vue';
import PlaylistCreateModal from '@/components/PlaylistCreateModal.vue';
import VirtualList from '@/components/common/VirtualList.vue';
import VirtualHorizontalList from '@/components/common/VirtualHorizontalList.vue';
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

watch(() => audioStore.lastUnavailableTrackId, (trackId) => {
  if (trackId === null) return;
  const track = tracks.value.find(t => t.id === trackId);
  if (track) track.unavailable = true;
});
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
    console.log(playlistDtos, 'PLAYLISTS');
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
    console.log(trackDtos, 'DTOS');
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

const handlePlaylistClick = (playlist: PlaylistDto) => {
  emit('navigate-playlist', playlist.id);
};

const handlePlaylistPlay = async (playlist: PlaylistDto) => {
  console.log('Play playlist clicked:', playlist);
  audioStore.startLoading();
  try {
    await invoke('load_playlist_and_play', { playlistId: playlist.id, startIndex: 0 });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play playlist:', e);
    audioStore.stopLoading();
  }
};

const handlePlaylistCache = async (playlist: PlaylistDto) => {
  console.log('Caching playlist:', playlist.title);
  notifications.addNotification(
    `Начинаю кэширование плейлиста "${playlist.title}"`,
    'info',
  );
  try {
    const pwi = await invoke<PlaylistWithItemsDto | null>(
      'get_playlist_with_tracks',
      { playlistId: playlist.id },
    );
    if (pwi && pwi.items) {
      let successCount = 0;
      let failCount = 0;

      for (const item of pwi.items) {
        if (item.itemType === 'track') {
          try {
            await invoke('save_track_to_file', { track: item.track });
            successCount++;
          } catch (e) {
            console.error(`Failed to cache track ${item.track.title}:`, e);
            failCount++;
          }
        }
      }

      if (failCount === 0) {
        notifications.addNotification(
          `Плейлист "${playlist.title}" поставлен на загрузку (${successCount} треков)`,
          'success',
        );
      } else {
        notifications.addNotification(
          `Загрузка "${playlist.title}": ${successCount} ок, ${failCount} ошибок`,
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

const handlePlaylistDelete = async (playlist: PlaylistDto) => {
  try {
    await invoke('delete_playlist', { playlistId: playlist.id });
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

const handleTrackClick = async (track: TrackDto) => {
  isPageLoading.value = true;
  audioStore.startLoading();
  try {
    const dto = await invoke<LibraryPageDto>('load_and_play_library_page_at_track', {
      trackId: track.id,
      pageSize: PAGE_SIZE,
      order: { [sortField.value]: sortDirection.value },
    });
    pageCursor.value = { currentPage: dto.currentPage, totalPages: dto.totalPages };
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
    pageCursor.value = { currentPage: dto.currentPage, totalPages: dto.totalPages };
    refreshQueue();
  } catch (e) {
    console.error('Failed to load page:', e);
    audioStore.stopLoading();
  } finally {
    isPageLoading.value = false;
  }
};

const handleTrackPlay = async (track: TrackDto) => {
  console.log('Play track clicked:', track);
  audioStore.startLoading();
  try {
    await invoke('play_library_track', { trackId: track.id });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play track:', e);
    audioStore.stopLoading();
  }
};

const handleTrackMore = (track: TrackDto) => {
  console.log('Track more clicked:', track);
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

.playlistSection {
  flex-shrink: 0;
  padding: $spacing-xl $spacing-xl 0;
  margin-bottom: $spacing-2xl;
}

.trackSection {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0 $spacing-xl;
}

.scroller {
  flex: 1;
  min-height: 0;
}

.sectionHeader {
  @include flex-between;
  margin-bottom: $spacing-lg;
}

.sectionTitle {
  font-size: $font-size-2xl;
  font-weight: $font-weight-bold;
  color: var(--color-text-primary);
  margin: 0;
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

.headerControls {
  @include flex-center;
  gap: $spacing-sm;
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

.playlistList {
  margin: 0 (-$spacing-xl);
  padding: 0 $spacing-xl;
}

.trackGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: $spacing-lg;
}

.emptyState {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: $spacing-2xl * 2;
  background-color: var(--color-bg-secondary);
  border-radius: $radius-lg;
  border: 2px dashed var(--color-border);
  gap: $spacing-lg;
}

.emptyIcon {
  font-size: 64px;
  opacity: 0.4;
  filter: grayscale(1);
}

.emptyTitle {
  font-size: $font-size-xl;
  font-weight: $font-weight-semibold;
  color: var(--color-text-primary);
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
