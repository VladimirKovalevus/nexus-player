<template>
  <div :class="$style.vkView">
    <header :class="$style.header">
      <h1 :class="$style.title">VK Музыка</h1>
      <button :class="$style.refreshButton" @click="fetchContent" :disabled="loading">
        {{ loading ? 'Загрузка...' : 'Обновить' }}
      </button>
    </header>

    <div v-if="error" :class="$style.error">{{ error }}</div>

    <section v-if="playlists.length > 0" :class="$style.section">
      <h2 :class="$style.sectionTitle">Ваши плейлисты</h2>
      <div :class="$style.grid">
        <MediaCardCompact
          v-for="playlist in playlists"
          :key="playlist.id"
          type="playlist"
          :title="playlist.title"
          :artist="playlist.creator || 'VK User'"
          :thumbnail="playlist.thumbnail || undefined"
          :track-count="playlist.trackCount"
          @click="handlePlaylistClick(playlist)"
          @play="handlePlaylistPlay(playlist)"
        />
      </div>
    </section>

    <section v-if="tracks.length > 0" :class="$style.section">
      <h2 :class="$style.sectionTitle">Ваши аудиозаписи</h2>
      <div :class="$style.list">
        <MediaCard
          v-for="track in tracks"
          :key="track.id"
          type="track"
          :track="track"
          show-action
          @click="handleTrackClick(track)"
          @play="handleTrackPlay(track)"
          @refresh="fetchContent"
        />
      </div>
    </section>

    <div v-if="!loading && tracks.length === 0 && playlists.length === 0" :class="$style.empty">
      Ничего не найдено. Убедитесь, что вы залогинены в настройках.
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, inject } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCard from '@/components/common/MediaCard.vue';
import MediaCardCompact from '@/components/common/MediaCardCompact.vue';
import type { TrackDto, PlaylistDto } from '@/types/types';

const refreshQueue = inject<() => void>('refreshQueue', () => {});

const tracks = ref<TrackDto[]>([]);
const playlists = ref<PlaylistDto[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

const fetchContent = async () => {
  loading.value = true;
  error.value = null;
  console.log('Fetching VK profile content...');
  try {
    const result = await invoke<[TrackDto[], PlaylistDto[]]>('get_vk_profile');
    console.log('Received VK profile data:', result);
    const [t, p] = result;
    tracks.value = t;
    playlists.value = p;
  } catch (e) {
    console.error('Failed to fetch VK profile:', e);
    error.value = `Ошибка: ${e}`;
  } finally {
    loading.value = false;
  }
};

const handleTrackClick = async (track: TrackDto) => {
  if (track.source) {
    // Construct SearchResultDto (Track variant)
    const searchResult = {
      type: 'track',
      title: track.title,
      artist: track.artist || '',
      durationSecs: track.durationSecs,
      thumbnail: track.artworkUrl,
      source: track.source,
      state: track.state,
    };

    await invoke('handle_streaming_track', {
      searchResult,
      strategy: 'cache',
    });
    refreshQueue();
  }
};

const handleTrackPlay = async (track: TrackDto) => {
  await invoke('clear_queue');
  await handleTrackClick(track);
  await invoke('play_at_index', { index: 0 });
  refreshQueue();
};

const handlePlaylistClick = async (playlist: PlaylistDto) => {
  console.log('Playlist click', playlist);
  if (playlist.source) {
    // Construct SearchResultDto for handle_streaming_track
    const searchResult = {
      type: 'playlist',
      id: playlist.source.url,
      title: playlist.title,
      creator: playlist.creator || '',
      trackCount: playlist.trackCount,
      thumbnail: playlist.thumbnail || undefined,
      url: `https://vk.com/audio?z=audio_playlist${playlist.source.url}`,
      system: playlist.source.system,
      tracks: [],
    };

    try {
      await invoke('handle_streaming_track', {
        searchResult,
        strategy: 'cache',
      });
      refreshQueue();
    } catch (e) {
      console.error('Failed to expand playlist:', e);
    }
  }
};

const handlePlaylistPlay = async (playlist: PlaylistDto) => {
  await handlePlaylistClick(playlist);
  await invoke('play_at_index', { index: 0 });
  refreshQueue();
};

onMounted(fetchContent);
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.vkView {
  padding: $spacing-xl;
  height: 100%;
  overflow-y: auto;
}

.header {
  @include flex-between;
  margin-bottom: $spacing-2xl;
}

.title {
  font-size: $font-size-3xl;
  font-weight: $font-weight-bold;
  color: var(--color-text-primary);
}

.refreshButton {
  padding: $spacing-sm $spacing-lg;
  background-color: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  color: var(--color-text-primary);
  cursor: pointer;
  
  &:hover {
    background-color: var(--color-bg-hover);
  }
}

.section {
  margin-bottom: $spacing-3xl;
}

.sectionTitle {
  font-size: $font-size-xl;
  margin-bottom: $spacing-lg;
  color: var(--color-text-primary);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: $spacing-lg;
}

.list {
  display: flex;
  flex-direction: column;
  gap: $spacing-sm;
}

.error {
  color: #ef4444;
  padding: $spacing-md;
  background-color: rgba(239, 68, 68, 0.1);
  border-radius: $radius-md;
  margin-bottom: $spacing-lg;
}

.empty {
  text-align: center;
  padding: $spacing-3xl;
  color: var(--color-text-secondary);
}
</style>
