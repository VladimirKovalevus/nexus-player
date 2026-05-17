<template>
  <div class="cache-scroll" :class="$style.cache">
    <div>
      <div class="view-header">
        <h1 class="view-title">{{ $t('nav.cache') }}</h1>
        <span class="view-count">{{ $t('home.trackCount', { count: cachedTracks.length }) }}</span>
      </div>

      <MediaTileSection
        :title="$t('cacheView.albums')"
        :items="albumItems"
        :empty-title="$t('cacheView.noAlbums')"
        placeholder-icon="album"
        @item-click="handleAlbumClick"
        @item-play="handleAlbumPlay"
      />

      <TrackListSection
        :title="$t('cacheView.tracks')"
        :tracks="cachedTracks"
        :active-track-id="audioStore.currentTrack?.id ?? null"
        :empty-title="$t('cacheView.noTracks')"
        scroll-selector=".cache-scroll"
        @track-click="handleTrackClick"
        @track-play="handleTrackPlay"
        @track-refresh="fetchData"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, inject } from 'vue';
import { invoke } from '@/utils/providerAuth';
import { useI18n } from 'vue-i18n';
import MediaTileSection, { type MediaTileItem } from '@/components/common/MediaTileSection.vue';
import TrackListSection from '@/components/common/TrackListSection.vue';
import type { TrackDto, CachedAlbumDto } from '@/types/types';
import { useAudioStore } from '@/stores/audio';

const emit = defineEmits<{
  'navigate-playlist': [playlistId: number];
}>();

const { t } = useI18n();
const audioStore = useAudioStore();
const refreshQueue = inject<() => void>('refreshQueue', () => {});

const cachedTracks = ref<TrackDto[]>([]);
const albums = ref<CachedAlbumDto[]>([]);

const albumItems = computed<MediaTileItem[]>(() =>
  albums.value.map((album) => ({
    id: album.id,
    title: album.title,
    subtitle: `${album.artist || t('track.unknownArtist')} · ${album.cachedTrackCount}`,
    thumbnail: album.artworkUrl,
  })),
);

const fetchData = async () => {
  try {
    const [tracksResult, albumsResult] = await Promise.all([
      invoke<TrackDto[]>('get_cached_tracks', { order: { default: 'desc' } }),
      invoke<CachedAlbumDto[]>('get_cached_albums'),
    ]);
    cachedTracks.value = tracksResult;
    albums.value = albumsResult;
  } catch (e) {
    console.error('Failed to fetch cached library:', e);
  }
};

const playVisibleTracks = async (track: TrackDto, visibleTracks: TrackDto[]) => {
  audioStore.startLoading();
  try {
    const trackIds = visibleTracks.map((t) => t.id);
    await invoke('load_and_play_library_tracks', { trackIds, selectedId: track.id });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play track:', e);
    audioStore.stopLoading();
  }
};

const handleTrackClick = (track: TrackDto, visibleTracks: TrackDto[]) =>
  playVisibleTracks(track, visibleTracks);
const handleTrackPlay = (track: TrackDto, visibleTracks: TrackDto[]) =>
  playVisibleTracks(track, visibleTracks);

const handleAlbumClick = (item: MediaTileItem) => {
  emit('navigate-playlist', item.id);
};

const handleAlbumPlay = async (item: MediaTileItem) => {
  audioStore.startLoading();
  try {
    await invoke('load_playlist_and_play', { playlistId: item.id, startIndex: 0 });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play album:', e);
    audioStore.stopLoading();
  }
};

onMounted(() => fetchData());
</script>

<style module lang="scss">
.cache {
  height: 100%;
  overflow-y: auto;
}
</style>
