<template>
  <div :class="$style.cache">
    <section :class="$style.albumSection">
      <div :class="$style.sectionHeader">
        <h2 :class="$style.sectionTitle">Альбомы</h2>
        <a
          v-if="selectedAlbum"
          href="#"
          :class="$style.showAll"
          @click.prevent="selectedAlbum = null"
        >
          Сбросить фильтр
        </a>
      </div>

      <div v-if="albums.length === 0" :class="$style.emptyState">
        <div :class="$style.emptyIcon"><AppIcon name="album" :size="64" /></div>
        <div :class="$style.emptyTitle">Нет закэшированных альбомов</div>
      </div>

      <div v-else :class="$style.albumList">
        <VirtualHorizontalList :items="albums" :item-width="180" :height="220">
          <template #default="{ item: album }">
            <MediaCardCompact
              type="album"
              :title="album.title"
              :artist="album.artist || 'Неизвестный исполнитель'"
              :thumbnail="album.artworkUrl || undefined"
              :track-count="album.cachedTrackCount"
              :active="selectedAlbum === album.title"
              @click="toggleAlbum(album.title)"
              @play="handleAlbumPlay(album)"
            />
          </template>
        </VirtualHorizontalList>
      </div>
    </section>

    <section :class="$style.trackSection">
      <div :class="$style.sectionHeader">
        <h2 :class="$style.sectionTitle">
          Треки{{ selectedAlbum ? `: ${selectedAlbum}` : '' }}
        </h2>
      </div>

      <div v-if="displayedTracks.length === 0" :class="$style.emptyState">
        <div :class="$style.emptyIcon"><AppIcon name="music-note" :size="64" /></div>
        <div :class="$style.emptyTitle">Нет закэшированных треков</div>
      </div>

      <VirtualList
        v-else
        :class="$style.scroller"
        :items="displayedTracks"
        :item-height="96"
        :overscan="8"
      >
        <template #default="{ item: track }">
          <MediaCard
            type="track"
            :track="track"
            :active="audioStore.currentTrack?.id === track.id"
            @click="handleTrackClick(track)"
            @play="handleTrackPlay(track)"
          />
        </template>
      </VirtualList>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, inject } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCard from '@/components/common/MediaCard.vue';
import MediaCardCompact from '@/components/common/MediaCardCompact.vue';
import VirtualList from '@/components/common/VirtualList.vue';
import VirtualHorizontalList from '@/components/common/VirtualHorizontalList.vue';
import AppIcon from '@/components/common/AppIcon.vue';
import type { TrackDto, CachedAlbumDto } from '@/types/types';
import { useAudioStore } from '@/stores/audio';

const audioStore = useAudioStore();
const refreshQueue = inject<() => void>('refreshQueue', () => {});

const cachedTracks = ref<TrackDto[]>([]);
const albums = ref<CachedAlbumDto[]>([]);
const selectedAlbum = ref<string | null>(null);

const displayedTracks = computed(() => {
  if (!selectedAlbum.value) return cachedTracks.value;
  return cachedTracks.value.filter((t) => t.album === selectedAlbum.value);
});

const toggleAlbum = (title: string) => {
  selectedAlbum.value = selectedAlbum.value === title ? null : title;
};

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

const playTrack = async (track: TrackDto) => {
  audioStore.startLoading();
  try {
    await invoke('play_library_track', { trackId: track.id });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play track:', e);
    audioStore.stopLoading();
  }
};

const handleTrackClick = (track: TrackDto) => playTrack(track);
const handleTrackPlay = (track: TrackDto) => playTrack(track);

const handleAlbumPlay = async (album: CachedAlbumDto) => {
  const first = cachedTracks.value.find((t) => t.album === album.title);
  if (!first) return;
  await playTrack(first);
};

onMounted(() => fetchData());
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.cache {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.albumSection {
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

.albumList {
  margin: 0 (-$spacing-xl);
  padding: 0 $spacing-xl;
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
  opacity: 0.4;
  filter: grayscale(1);
}

.emptyTitle {
  font-size: $font-size-xl;
  font-weight: $font-weight-semibold;
  color: var(--color-text-primary);
}
</style>
