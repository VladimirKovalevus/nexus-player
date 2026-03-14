<template>
  <div :class="$style.vkView">
    <header :class="$style.header">
      <h1 :class="$style.title">VK Музыка</h1>
      <button :class="$style.refreshButton" @click="vkStore.fetchContent(true)" :disabled="vkStore.loading">
        {{ vkStore.loading ? 'Загрузка...' : 'Обновить' }}
      </button>
    </header>

    <div v-if="vkStore.error" :class="$style.error">{{ vkStore.error }}</div>

    <section v-if="vkStore.playlists.length > 0" :class="$style.section">
      <h2 :class="$style.sectionTitle">Ваши плейлисты</h2>
      <div :class="$style.grid">
        <MediaCardCompact
          v-for="playlist in vkStore.playlists"
          :key="playlist.id"
          type="playlist"
          :title="playlist.title"
          :artist="playlist.creator || 'VK User'"
          :thumbnail="playlist.thumbnail || undefined"
          :track-count="playlist.trackCount"
          show-action
          @click="handlePlaylistClick(playlist)"
          @play="handlePlaylistPlay(playlist)"
        >
          <template #menu="{ close }">
            <button :class="$style.menuItem" @click.stop="() => { console.log('Template: click on create playlist'); handleCreatePlaylist(playlist); close(); }">
              Сформировать плейлист
            </button>
          </template>
        </MediaCardCompact>
      </div>
    </section>

    <section v-if="vkStore.tracks.length > 0" :class="$style.section">
      <h2 :class="$style.sectionTitle">Ваши аудиозаписи</h2>
      <div :class="$style.list">
        <MediaCard
          v-for="track in vkStore.tracks"
          :key="track.id"
          type="track"
          :track="track"
          show-action
          @click="handleTrackClick(track)"
          @play="handleTrackPlay(track)"
          @refresh="vkStore.fetchContent(true)"
        />
      </div>
    </section>

    <div v-if="!vkStore.loading && vkStore.tracks.length === 0 && vkStore.playlists.length === 0" :class="$style.empty">
      Ничего не найдено. Убедитесь, что вы залогинены в настройках.
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, inject } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCard from '@/components/common/MediaCard.vue';
import MediaCardCompact from '@/components/common/MediaCardCompact.vue';
import type { TrackDto, PlaylistDto } from '@/types/types';
import { useVkStore } from '@/stores/vk';
import { useNotificationStore } from '@/stores/notifications';

const vkStore = useVkStore();
const notifications = useNotificationStore();
const refreshQueue = inject<() => void>('refreshQueue', () => {});

const handleTrackClick = async (track: TrackDto) => {
  if (track.source) {
    // Construct SearchResultDto (Track variant)
    const searchResult = {
      type: 'track',
      id: track.id,
      title: track.title,
      artist: track.artist || '',
      durationSecs: track.durationSecs,
      thumbnail: track.artworkUrl,
      source: track.source,
      state: track.state,
      liked: track.liked || false,
    };

    await invoke('handle_streaming_track', {
      searchResult,
      strategy: 'cache',
      searchType: 'track',
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
        searchType: 'playlist',
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

const handleCreatePlaylist = async (playlist: PlaylistDto) => {
  console.log('handleCreatePlaylist triggered for:', playlist.title);
  if (playlist.source) {
    console.log('Playlist source found:', playlist.source);
    try {
      notifications.addNotification(`Импорт плейлиста "${playlist.title}"...`, 'info');
      await invoke('create_playlist_from_container', {
        provider: playlist.source.system,
        contentType: 'Playlist',
        id: playlist.source.url,
        title: playlist.title,
        description: playlist.description || '',
        artworkUrl: playlist.thumbnail,
      });
      notifications.addNotification(`Плейлист "${playlist.title}" успешно импортирован`, 'success');
    } catch (e) {
      console.error('Failed to create playlist from VK:', e);
      notifications.addNotification(`Ошибка импорта: ${e}`, 'error');
    }
  } else {
    console.warn('Playlist has no source data:', playlist);
    notifications.addNotification('Не удалось получить данные источника плейлиста', 'error');
  }
};

onMounted(() => {
  vkStore.fetchContent();
});
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
}
</style>
