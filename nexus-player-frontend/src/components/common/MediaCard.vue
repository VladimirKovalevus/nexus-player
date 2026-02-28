<template>
  <div :class="[$style.mediaCard, $style[`type-${type}`], { [$style.dropdownActive]: dropdownOpen }]" :data-dropdown-open="dropdownOpen || undefined">
    <div :class="$style.content" @click="handleClick">
    <div :class="$style.thumbnail">
      <img
        v-if="entity.thumbnail"
        :src="entity.thumbnail"
        :alt="entity.title"
        :class="$style.image"
      />
      <div v-else :class="$style.placeholder">
        <span :class="$style.placeholderIcon">{{ placeholderIcon }}</span>
      </div>

      <div v-if="type === 'track'" :class="$style.playOverlay">
        <button :class="$style.playButton" @click.stop="handlePlay">▶</button>
      </div>
    </div>

    <div :class="$style.info">
      <div :class="$style.title" :title="entity.title">{{ entity.title }}</div>
      <div :class="$style.meta">
        <span :class="$style.artist">{{ entity.artist }}</span>

        <template v-if="type === 'track' && entity.duration !== undefined">
          <span :class="$style.separator">•</span>
          <span :class="$style.duration">{{ formatDuration(entity?.duration||0) }}</span>
        </template>

        <template
          v-if="
            (type === 'playlist' || type === 'album') &&
            entity.trackCount !== undefined
          "
        >
          <span :class="$style.separator">•</span>
          <span :class="$style.trackCount">{{ entity.trackCount }} tracks</span>
        </template>

        <template v-if=" entity.year">
          <span :class="$style.separator">•</span>
          <span :class="$style.year">{{ entity.year }}</span>
        </template>
      </div>
    </div>
    </div>

    <div v-if="true" :class="$style.actions" ref="dropdownRef">
      <button :class="$style.actionButton" @click.stop="toggleDropdown">⋯</button>

      <div v-if="dropdownOpen" :class="$style.dropdown" @click.stop>
        <div v-if="!showPlaylistSubmenu">
          <button
            v-for="option in menuOptions"
            :key="option.label"
            :class="$style.dropdownItem"
            @click="option.action"
          >
            <span :class="$style.itemIcon">{{ option.icon }}</span>
            <span :class="$style.itemLabel">{{ option.label }}</span>
            <span v-if="option.hasSubmenu" :class="$style.itemArrow">›</span>
          </button>
        </div>

        <div v-else :class="$style.submenu">
          <button :class="$style.dropdownItem" @click="showPlaylistSubmenu = false">
            <span :class="$style.itemIcon">‹</span>
            <span :class="$style.itemLabel">Назад</span>
          </button>
          <div :class="$style.divider"></div>
          <button
            v-for="pl in filteredPlaylists"
            :key="pl.id"
            :class="$style.dropdownItem"
            @click="handleAddToPlaylist(pl.id)"
          >
            <span :class="$style.itemIcon">📋</span>
            <span :class="$style.itemLabel">{{ pl.title }}</span>
          </button>
          <div v-if="playlists.length === 0" :class="$style.emptyPlaylist">
            Нет плейлистов
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { onClickOutside } from '@vueuse/core';
import type { TrackDto, PlaylistDto, ContainerMetaDto } from '@/types/types';
import { useAudioStore } from '@/stores/audio';
const store = useAudioStore()

interface Props {
  type: 'track' | 'playlist' | 'album';
  track?: TrackDto;
  playlist?: PlaylistDto;
  album?: ContainerMetaDto;
  showAction: boolean;
  context?: string;
  isLocal?: boolean;
  isInLibrary?: boolean;
  queueIndex?: number;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  click: [];
  play: [];
  refresh: [];
}>();

const dropdownOpen = ref(false);
const dropdownRef = ref<HTMLElement | null>(null);
const playlists = ref<Array<{ id: number; title: string }>>([]);
const showPlaylistSubmenu = ref(false);

const entity = computed(() => {

  if (props.type === 'track' && props.track) {
    return {
      id: props.track.id,
      title: props.track.title,
      artist: props.track.artist || 'Unknown Artist',
      thumbnail: props.track.artworkUrl,
      duration: props.track.durationSecs,
      trackCount: undefined,
      year: undefined,
    };
  } else if (props.type === 'playlist' && props.playlist) {
    return {
      id: props.playlist.id,
      title: props.playlist.title,
      artist: props.playlist.creator || 'Unknown Creator',
      thumbnail: props.playlist.thumbnail || undefined,
      duration: undefined,
      trackCount: props.playlist.trackCount,
      year: undefined,
    };
  } else if (props.type === 'album' && props.album) {
    return {
      id: props.album.id,
      title: props.album.title,
      artist: props.album.artist || 'Unknown Artist',
      thumbnail: props.album.thumbnail || undefined,
      duration: undefined,
      trackCount: props.album.trackCount,
      year: undefined,
    };
  }

  return {
    id: 0,
    title: 'Unknown',
    artist: 'Unknown',
    thumbnail: undefined,
    duration: undefined,
    trackCount: undefined,
    year: undefined,
  };
});

interface MenuOption {
  label: string;
  icon: string;
  action: () => void | Promise<void>;
  hasSubmenu?: boolean;
}

const menuOptions = computed<MenuOption[]>(() => {
  const options: MenuOption[] = [];

  if (props.type === 'track') {
    options.push({
      label: 'Добавить в плейлист',
      icon: '📋',
      action: handleShowPlaylistSubmenu,
      hasSubmenu: true,
    });
  } else if (props.type === 'playlist') {
    options.push({
      label: 'Добавить в плейлист',
      icon: '📋',
      action: handleShowPlaylistSubmenu,
      hasSubmenu: true,
    });
    return options;
  }

  if (props.type !== 'track') return options;

  if (props.context === 'queue') {
    options.push({
      label: 'Удалить из очереди',
      icon: '🗑️',
      action: handleRemoveFromQueue,
    });
  } else {
    options.push({
      label: 'Добавить в очередь',
      icon: '➕',
      action: handleAddToQueue,
    });
  }

  if (!props.isLocal) {
    options.push({
      label: 'Скачать',
      icon: '⬇️',
      action: handleDownload,
    });
  }

  options.push({
    label: 'Сохранить в файл',
    icon: '💾',
    action: handleSaveToFile,
  });

  if (props.context === 'queue' && !props.isInLibrary) {
    options.push({
      label: 'Добавить в библиотеку',
      icon: '➕',
      action: handleAddToLibrary,
    });
  }

  return options;
});

const filteredPlaylists = computed(() => {
  if (props.type === 'playlist' && props.playlist) {
    return playlists.value.filter(p => p.id !== props.playlist!.id);
  }
  return playlists.value;
});

const placeholderIcon = computed(() => {
  switch (props.type) {
    case 'track':
      return '🎵';
    case 'playlist':
      return '📂';
    case 'album':
      return '💿';
    default:
      return '🎵';
  }
});

const formatDuration = (seconds: number): string => {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

let stopClickOutside: (() => void) | null = null;

const handleClick = () => {
  emit('click');
};

const handlePlay = () => {
  emit('play');
};

const toggleDropdown = async () => {
  dropdownOpen.value = !dropdownOpen.value;
  showPlaylistSubmenu.value = false;

  if (dropdownOpen.value) {
    if (playlists.value.length === 0) {
      await fetchPlaylists();
    }
    // Start listening only when open
    if (!stopClickOutside) {
        stopClickOutside = onClickOutside(dropdownRef, () => {
          closeDropdown();
        });
    }
  } else {
    // Stop listening when closed
    if (stopClickOutside) {
        stopClickOutside();
        stopClickOutside = null;
    }
  }
};

const closeDropdown = () => {
  dropdownOpen.value = false;
  showPlaylistSubmenu.value = false;
  if (stopClickOutside) {
    stopClickOutside();
    stopClickOutside = null;
  }
};

const fetchPlaylists = async () => {
  try {
    const allPlaylists = await invoke<Array<{ id: number; title: string }>>('get_all_playlists');
    playlists.value = allPlaylists;
  } catch (e) {
    console.error('Failed to fetch playlists:', e);
  }
};

const handleShowPlaylistSubmenu = () => {
  showPlaylistSubmenu.value = true;
};

const handleAddToPlaylist = async (playlistId: number) => {
  try {
    if (props.type === 'track' && props.track) {
      await invoke('add_track_to_playlist', {
        playlistId,
        track: props.track,
      });
    } else if (props.type === 'playlist' && props.playlist) {
      if (props.playlist.id === playlistId) {
        console.warn('Cannot add playlist to itself');
        return;
      }
      await invoke('add_playlist_to_playlist', {
        parentPlaylistId: playlistId,
        childPlaylistId: props.playlist.id,
      });
    }
    closeDropdown();
  } catch (e) {
    console.error('Failed to add to playlist:', e);
  }
};

const handleAddToQueue = async () => {
  if (props.type !== 'track') return;

  try {
    await invoke('add_to_queue', { trackId: entity.value.id });
    emit('refresh');
    closeDropdown();
  } catch (e) {
    console.error('Failed to add track to queue:', e);
  }
};

const handleRemoveFromQueue = async () => {
  if (props.queueIndex === undefined) return;

  try {
    await invoke('remove_from_queue', { index: props.queueIndex });
    emit('refresh');
    closeDropdown();
  } catch (e) {
    console.error('Failed to remove from queue:', e);
  }
};

const handleDownload = async () => {
  console.log('Download track:', entity.value.id);
  closeDropdown();
};

const handleSaveToFile = async () => {
  try {
    await invoke('save_track_to_file', { trackId: entity.value.id });
    console.log('Track saved to file:', entity.value.id);
  } catch (e) {
    console.error('Failed to save track to file:', e);
  }
  closeDropdown();
};

const handleAddToLibrary = async () => {
  console.log('Add to library:', entity.value.id);
  closeDropdown();
};
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.mediaCard {
  display: flex;
  gap: $spacing-md;
  padding: $spacing-md;
  background-color: var(--color-bg-secondary);
  border-radius: $radius-md;
  transition: all $transition-fast;
  position: relative;

  &:hover {
    background-color: var(--color-bg-hover);

    .playOverlay {
      opacity: 1;
    }
  }

  &.dropdownActive {
    z-index: 1000;
  }
}

.content {
  display: flex;
  gap: $spacing-md;
  flex: 1;
  min-width: 0;
  cursor: pointer;
}

.type-track {
  border-left: 3px solid #4caf50;
}

.type-playlist {
  border-left: 3px solid #2196f3;
}

.type-album {
  border-left: 3px solid #ff9800;
}

.thumbnail {
  position: relative;
  width: 56px;
  height: 56px;
  flex-shrink: 0;
  border-radius: $radius-sm;
  overflow: hidden;
  background-color: var(--color-bg-primary);
}

.image {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.placeholder {
  width: 100%;
  height: 100%;
  @include flex-center;
  background: linear-gradient(
    135deg,
    var(--color-accent-primary) 0%,
    var(--color-accent-purple) 100%
  );
}

.placeholderIcon {
  font-size: $font-size-2xl;
}

.playOverlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  @include flex-center;
  background-color: rgba(0, 0, 0, 0.6);
  opacity: 0;
  transition: opacity $transition-fast;
}

.playButton {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background-color: var(--color-accent-primary);
  color: white;
  border: none;
  @include flex-center;
  font-size: $font-size-base;
  cursor: pointer;
  transition: all $transition-fast;

  &:hover {
    background-color: var(--color-accent-hover);
    transform: scale(1.1);
  }
}

.info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: $spacing-xs;
}

.title {
  font-size: $font-size-base;
  font-weight: $font-weight-semibold;
  color: var(--color-text-primary);
  @include text-ellipsis;
}

.meta {
  display: flex;
  align-items: center;
  gap: $spacing-xs;
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
  @include text-ellipsis;
}

.artist {
  @include text-ellipsis;
}

.separator {
  color: var(--color-text-muted);
}

.duration,
.trackCount,
.year {
  white-space: nowrap;
  color: var(--color-text-muted);
}

.actions {
  display: flex;
  align-items: center;
  position: relative;
}

.actionButton {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  font-size: $font-size-xl;
  cursor: pointer;
  @include flex-center;
  transition: all $transition-fast;

  &:hover {
    background-color: var(--color-bg-primary);
    color: var(--color-text-primary);
  }
}

.dropdown {
  position: absolute;
  top: 100%;
  right: 0;
  margin-top: $spacing-xs;
  background-color: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  min-width: 200px;
  z-index: 1000;
  overflow: hidden;
}

.dropdownItem {
  width: 100%;
  display: flex;
  align-items: center;
  gap: $spacing-sm;
  padding: $spacing-sm $spacing-md;
  background: transparent;
  border: none;
  color: var(--color-text-primary);
  font-size: $font-size-sm;
  text-align: left;
  cursor: pointer;
  transition: background-color $transition-fast;

  &:hover {
    background-color: var(--color-bg-hover);
  }
}

.itemIcon {
  font-size: $font-size-base;
  flex-shrink: 0;
}

.itemLabel {
  flex: 1;
}

.itemArrow {
  font-size: $font-size-lg;
  color: var(--color-text-muted);
}

.divider {
  height: 1px;
  background-color: var(--color-border);
  margin: $spacing-xs 0;
}

.submenu {
  animation: slideIn 0.15s ease-out;
}

.emptyPlaylist {
  padding: $spacing-lg;
  text-align: center;
  color: var(--color-text-muted);
  font-size: $font-size-sm;
}

@keyframes slideIn {
  from {
    opacity: 0;
    transform: translateX(-10px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}
</style>
