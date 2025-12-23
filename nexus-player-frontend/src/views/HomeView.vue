<template>
  <div :class="$style.home">
    <PlaylistCreateModal
      :is-open="isPlaylistModalOpen"
      @close="handleModalClose"
      @created="handlePlaylistCreated"
    />

    <section :class="$style.section">
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

      <div v-else :class="$style.trackGrid">
        <MediaCardCompact
          v-for="playlist in playlists"
          :key="playlist.id"
          type="playlist"
          :title="playlist.title"
          :artist="playlist.creator"
          :thumbnail="playlist.thumbnail"
          :track-count="playlist.trackCount"
          @click="handlePlaylistClick(playlist)"
          @play="handlePlaylistPlay(playlist)"
        />
      </div>
    </section>

    <section :class="$style.section">
      <div :class="$style.sectionHeader">
        <h2 :class="$style.sectionTitle">Треки</h2>
        <div v-if="tracks.length > 0" :class="$style.filter">
          <span>По умолчанию</span>
          <span>▼</span>
        </div>
      </div>

      <div v-if="tracks.length === 0" :class="$style.emptyState">
        <div :class="$style.emptyIcon">🎵</div>
        <div :class="$style.emptyTitle">Данных нет</div>
        <div :class="$style.emptyHint">Добавьте новый трек</div>
        <button :class="$style.addButton" @click="handleAddTrack">
          + Добавить трек
        </button>
      </div>

      <div v-else :class="$style.trackList">
        <MediaCard
          v-for="track in tracks"
          :key="track.id"
          type="track"
          :title="track.title"
          :artist="track.artist"
          :thumbnail="track.thumbnail"
          :duration="track.duration"
          @click="handleTrackClick(track)"
          @play="handleTrackPlay(track)"
          @more="handleTrackMore(track)"
        />
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, inject } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCard from '@/components/common/MediaCard.vue';
import MediaCardCompact from '@/components/common/MediaCardCompact.vue';
import PlaylistCreateModal from '@/components/PlaylistCreateModal.vue';

const refreshQueue = inject<() => void>('refreshQueue', () => {});

interface Playlist {
  id: number;
  title: string;
  creator: string;
  trackCount: number;
  thumbnail?: string;
}

interface Track {
  id: number;
  title: string;
  artist: string;
  duration: number;
  thumbnail?: string;
}

interface TrackDto {
  id: number;
  title: string;
  artist: string | null;
  album: string | null;
  duration_secs: number | null;
  artwork_url: string | null;
  play_count: number;
}

interface PlaylistDto {
  id: number;
  title: string;
  description: string | null;
  creator: string | null;
  track_count: number;
  created_at: number;
  updated_at: number;
}

// State
const playlists = ref<Playlist[]>([]);
const tracks = ref<Track[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const isPlaylistModalOpen = ref(false);

// Fetch data from backend
const fetchPlaylists = async () => {
  try {
    const playlistDtos = await invoke<PlaylistDto[]>('get_all_playlists');

    playlists.value = playlistDtos.map((dto) => ({
      id: dto.id,
      title: dto.title,
      creator: dto.creator || 'Unknown',
      trackCount: dto.track_count,
      thumbnail: undefined,
    }));
  } catch (e) {
    console.error('Failed to fetch playlists:', e);
    error.value = `Failed to load playlists: ${e}`;
  }
};

const fetchTracks = async () => {
  try {
    const trackDtos = await invoke<TrackDto[]>('get_all_tracks');

    tracks.value = trackDtos.map((dto) => ({
      id: dto.id,
      title: dto.title,
      artist: dto.artist || 'Unknown Artist',
      duration: dto.duration_secs || 0,
      thumbnail: dto.artwork_url || undefined,
    }));
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

// Load data on mount
onMounted(() => {
  loadData();
});

// Handlers for playlists
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
  // TODO: Navigate to playlists page
};

const handlePlaylistClick = (playlist: Playlist) => {
  console.log('Playlist clicked:', playlist);
  // TODO: Navigate to playlist detail page
};

const handlePlaylistPlay = async (playlist: Playlist) => {
  console.log('Play playlist clicked:', playlist);
  try {
    await invoke('load_playlist_to_queue', { playlistId: playlist.id });
    await invoke('play_at_index', { index: 0 });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play playlist:', e);
  }
};

// Handlers for tracks
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

const handleTrackClick = async (track: Track) => {
  console.log('Track clicked:', track);
  try {
    await invoke('add_to_queue', { trackId: track.id });
    refreshQueue();
  } catch (e) {
    console.error('Failed to add track to queue:', e);
  }
};

const handleTrackPlay = async (track: Track) => {
  console.log('Play track clicked:', track);
  try {
    await invoke('clear_queue');
    const index = await invoke<number>('add_to_queue', { trackId: track.id });
    await invoke('play_at_index', { index });
    refreshQueue();
  } catch (e) {
    console.error('Failed to play track:', e);
  }
};

const handleTrackMore = (track: Track) => {
  console.log('Track more clicked:', track);
  // TODO: Show context menu
};
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.home {
  padding: $spacing-xl;
  overflow-y: auto;
  height: 100%;
}

.section {
  margin-bottom: $spacing-2xl;
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

.filter {
  @include flex-center;
  gap: $spacing-sm;
  color: var(--color-text-secondary);
  font-size: $font-size-sm;
  cursor: pointer;
  padding: $spacing-xs $spacing-sm;
  border-radius: $radius-sm;
  transition: all $transition-fast;

  &:hover {
    color: var(--color-text-primary);
    background-color: var(--color-bg-hover);
  }
}

.trackGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: $spacing-lg;
}

.trackList {
  display: flex;
  flex-direction: column;
  gap: $spacing-sm;
}

// Empty State
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
  background: linear-gradient(
    135deg,
    var(--color-accent-primary) 0%,
    var(--color-accent-purple) 100%
  );
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
</style>
