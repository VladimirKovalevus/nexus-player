<template>
  <div :class="$style.cache">
    <MediaCardSection
      title="Альбомы"
      :items="albumItems"
      card-type="album"
      empty-icon="album"
      empty-title="Нет закэшированных альбомов"
      @item-click="handleAlbumClick"
      @item-play="handleAlbumPlay"
    />

    <TrackListSection
      title="Треки"
      :tracks="cachedTracks"
      :active-track-id="audioStore.currentTrack?.id ?? null"
      empty-title="Нет закэшированных треков"
      @track-click="handleTrackClick"
      @track-play="handleTrackPlay"
      @track-refresh="fetchData"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, inject } from 'vue';
import { invoke } from '@/utils/providerAuth';
import MediaCardSection, { type MediaCardItem } from '@/components/common/MediaCardSection.vue';
import TrackListSection from '@/components/common/TrackListSection.vue';
import type { TrackDto, CachedAlbumDto } from '@/types/types';
import { useAudioStore } from '@/stores/audio';

const emit = defineEmits<{
  'navigate-playlist': [playlistId: number];
}>();

const audioStore = useAudioStore();
const refreshQueue = inject<() => void>('refreshQueue', () => {});

const cachedTracks = ref<TrackDto[]>([]);
const albums = ref<CachedAlbumDto[]>([]);

const albumItems = computed<MediaCardItem[]>(() =>
  albums.value.map((album) => ({
    id: album.id,
    title: album.title,
    subtitle: album.artist || 'Неизвестный исполнитель',
    thumbnail: album.artworkUrl,
    trackCount: album.cachedTrackCount,
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

const handleAlbumClick = (item: MediaCardItem) => {
  emit('navigate-playlist', item.id);
};

const handleAlbumPlay = async (item: MediaCardItem) => {
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
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
</style>
