<script setup lang="ts">
import { errorMessage } from '@/utils/errorMessage';
import { ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@/utils/providerAuth';
import AppModal from '@/components/common/AppModal.vue';

import type { PlaylistDto, TrackDto } from '@/types/types';

interface Props {
  isOpen: boolean;
}

const props = defineProps<Props>();
const { t } = useI18n();
const emit = defineEmits<{
  close: [];
  created: [];
}>();

const title = ref('');
const description = ref('');
const allTracks = ref<TrackDto[]>([]);
const allAlbums = ref<PlaylistDto[]>([]);
const selectionView = ref<'tracks' | 'albums'>('tracks');
const selectedAlbumIds = ref<Set<number>>(new Set());
const selectedTrackIds = ref<Set<number>>(new Set());
const loading = ref(false);
const error = ref<string | null>(null);

const fetchLibrary = async () => {
  try {
    const [tracks, albums] = await Promise.all([
      invoke<TrackDto[]>('get_all_tracks'),
      invoke<PlaylistDto[]>('get_all_playlists'),
    ]);
    allTracks.value = tracks;
    allAlbums.value = albums;
  } catch (e) {
    console.error('Failed to fetch tracks:', e);
    error.value = t('common.error', { error: errorMessage(e) });
  }
};

// Loaded on open: this modal stays mounted, so tracks added later would be missing otherwise.
watch(
  () => props.isOpen,
  (isOpen) => {
    if (isOpen) fetchLibrary();
  },
  { immediate: true },
);

const toggleTrackSelection = (trackId: number) => {
  if (selectedTrackIds.value.has(trackId)) {
    selectedTrackIds.value.delete(trackId);
  } else {
    selectedTrackIds.value.add(trackId);
  }
};

const toggleAlbumSelection = (albumId: number) => {
  if (selectedAlbumIds.value.has(albumId)) {
    selectedAlbumIds.value.delete(albumId);
  } else {
    selectedAlbumIds.value.add(albumId);
  }
};

const handleSave = async () => {
  if (loading.value || !title.value.trim()) return;

  loading.value = true;
  error.value = null;

  try {
    const tracksToSave = allTracks.value.filter(t => selectedTrackIds.value.has(t.id));

    const playlistId = await invoke<number>('create_playlist_with_tracks', {
      request: {
        title: title.value.trim(),
        description: description.value.trim() || null,
        tracks: tracksToSave,
      },
    });

    try {
      for (const albumId of selectedAlbumIds.value) {
        await invoke('add_playlist_to_playlist', {
          parentPlaylistId: playlistId,
          childPlaylistId: albumId,
        });
      }
    } catch (e) {
      // Remove the incomplete playlist so saving again does not create duplicates.
      await invoke('delete_playlist', { playlistId });
      throw e;
    }

    emit('created');
    resetForm();
    emit('close');
  } catch (e) {
    console.error('Failed to create playlist:', e);
    error.value = t('common.error', { error: errorMessage(e) });
  } finally {
    loading.value = false;
  }
};

const resetForm = () => {
  title.value = '';
  description.value = '';
  selectedTrackIds.value.clear();
  selectedAlbumIds.value.clear();
  selectionView.value = 'tracks';
  error.value = null;
};

const handleClose = () => {
  if (loading.value) return;
  resetForm();
  emit('close');
};

const formatDuration = (seconds: number | null): string => {
  if (!seconds) return '0:00';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};
</script>

<template>
  <AppModal :open="isOpen" wide :title="$t('playlistForm.create')" @close="handleClose">
    <p v-if="error" :class="$style.error">{{ error }}</p>

    <div class="deck-field">
      <label class="section-label" for="new-playlist-title">{{ $t('playlistForm.title') }}</label>
      <input
        id="new-playlist-title"
        v-model="title"
        class="deck-input"
        type="text"
        :placeholder="$t('playlistForm.titlePlaceholder')"
        :disabled="loading"
      />
    </div>

    <div class="deck-field">
      <label class="section-label" for="new-playlist-description">{{ $t('playlistForm.description') }}</label>
      <textarea
        id="new-playlist-description"
        v-model="description"
        class="deck-input"
        :placeholder="$t('playlistForm.descriptionPlaceholder')"
        :disabled="loading"
        rows="2"
      />
    </div>

    <div class="deck-field">
      <div :class="$style.selectionTabs">
        <button type="button" class="deck-btn" :aria-pressed="selectionView === 'tracks'" @click="selectionView = 'tracks'">
          {{ $t('playlistForm.tracks', { count: selectedTrackIds.size }) }}
        </button>
        <button type="button" class="deck-btn" :aria-pressed="selectionView === 'albums'" @click="selectionView = 'albums'">
          {{ $t('playlistForm.albums', { count: selectedAlbumIds.size }) }}
        </button>
      </div>
      <div v-if="selectionView === 'tracks'" :class="$style.trackList">
        <p v-if="allTracks.length === 0" :class="$style.empty">{{ $t('home.noTracks') }}</p>
        <label v-for="track in allTracks" :key="track.id" :class="$style.trackItem">
          <input
            type="checkbox"
            :checked="selectedTrackIds.has(track.id)"
            :disabled="loading"
            @change="toggleTrackSelection(track.id)"
          />
          <span :class="$style.trackInfo">
            <span :class="$style.trackTitle">{{ track.title }}</span>
            <span :class="$style.trackArtist">{{ track.artist || $t('track.unknownArtist') }}</span>
          </span>
          <span v-if="track.durationSecs" :class="$style.trackTime">{{ formatDuration(track.durationSecs) }}</span>
        </label>
      </div>
      <div v-else :class="$style.trackList">
        <p v-if="allAlbums.length === 0" :class="$style.empty">{{ $t('home.noPlaylists') }}</p>
        <label v-for="album in allAlbums" :key="album.id" :class="$style.trackItem">
          <input
            type="checkbox"
            :checked="selectedAlbumIds.has(album.id)"
            :disabled="loading"
            @change="toggleAlbumSelection(album.id)"
          />
          <span :class="$style.trackInfo">
            <span :class="$style.trackTitle">{{ album.title }}</span>
            <span :class="$style.trackArtist">{{ album.creator || $t('track.unknownArtist') }}</span>
          </span>
          <span :class="$style.trackTime">{{ $t('home.trackCount', { count: album.trackCount }) }}</span>
        </label>
      </div>
    </div>

    <template #footer>
      <button class="deck-btn" :disabled="loading" @click="handleClose">{{ $t('common.cancel') }}</button>
      <button class="deck-btn deck-btn--primary" :disabled="loading || !title.trim()" @click="handleSave">
        {{ loading ? $t('playlistForm.saving') : $t('common.save') }}
      </button>
    </template>
  </AppModal>
</template>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.error {
  color: var(--danger);
  overflow-wrap: anywhere;
}

.selectionTabs {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;

  [aria-pressed='true'] {
    color: var(--accent);
    border-color: var(--accent);
  }
}

.empty {
  padding: 10px;
  color: var(--muted);
}

.trackList {
  max-height: 260px;
  overflow-y: auto;
  background: var(--bg-table);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
}

.trackItem {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  align-items: center;
  column-gap: 9px;
  min-height: var(--row-height-compact);
  padding: 4px 10px;
  border-bottom: 1px solid var(--line-soft);
  cursor: pointer;

  &:last-child {
    border-bottom: 0;
  }

  &:hover {
    background: var(--bg-selected);
  }

  input {
    accent-color: var(--accent);
  }
}

.trackInfo {
  display: flex;
  flex-direction: column;
  min-width: 0;
  line-height: 1.2;
}

.trackTitle {
  font-weight: 500;
  @include text-ellipsis;
}

.trackArtist {
  font-size: var(--font-size-xs);
  color: var(--muted);
  @include text-ellipsis;
}

.trackTime {
  font: 500 var(--font-size-sm) $font-family-mono;
  color: var(--muted);
}
</style>
