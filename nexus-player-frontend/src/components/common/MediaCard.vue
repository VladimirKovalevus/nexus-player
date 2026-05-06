<template>
  <div :class="[$style.mediaCard, $style[`type-${type}`], { [$style.dropdownActive]: dropdownOpen, [$style.active]: props.active, [$style.unavailable]: props.track?.unavailable }]" :data-dropdown-open="dropdownOpen || undefined">
    <div :class="$style.content" @click="handleClick">
    <div :class="$style.thumbnail">
      <img
        v-if="entity.thumbnail"
        :src="entity.thumbnail"
        :alt="entity.title"
        :class="$style.image"
      />
      <div v-else :class="$style.placeholder">
        <AppIcon :name="placeholderIcon" :size="24" />
      </div>

      <div v-if="type === 'track'" :class="$style.playOverlay">
        <button :class="$style.playButton" @click.stop="handlePlay" :disabled="(store.isLoading && props.active) || props.track?.unavailable">
          <AppIcon v-if="store.isLoading && props.active" name="loader-dot" :size="16" class="mini-loader" />
          <AppIcon v-else-if="props.track?.unavailable" name="x-circle" :size="16" />
          <AppIcon v-else name="play" :size="18" />
        </button>
      </div>
    </div>

    <div :class="$style.info">
      <div :class="[props.active ? $style['title-active'] : $style.title]" :title="entity.title">
        <MarqueeText :active="props.active">{{ entity.title }}</MarqueeText>
      </div>
      <div :class="$style.meta">
        <div :class="$style.artist">
          <MarqueeText :active="props.active">{{ entity.artist }}</MarqueeText>
        </div>

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
      <button
        v-if="type === 'track'"
        :class="[$style.likeButton, { [$style.isLiked]: props.track?.liked }]"
        @click.stop="handleToggleLike"
        :title="props.track?.liked ? 'Удалить из любимых' : 'Добавить в любимые'"
      >
        <div v-if="props.track?.liked" :class="$style.heartFilled"></div>
        <div v-else :class="$style.heartOutline"></div>
      </button>
      <button v-if="showAction " ref="actionButtonRef" :class="$style.actionButton" @click.stop="toggleDropdown">
        <AppIcon name="more-horizontal" :size="18" />
      </button>
      <button v-if="props.containerMode" :class="$style.expandBtn" @click.stop="emit('toggleExpand')">
        <AppIcon :name="props.isExpanded ? 'chevron-up' : 'chevron-down'" :size="16" />
      </button>
    </div>
  </div>

  <Teleport to="body">
    <div v-if="dropdownOpen" ref="dropdownPortalRef" :class="$style.dropdown" :style="dropdownStyle" @click.stop @wheel.stop>
      <div v-if="!showPlaylistSubmenu">
        <button
          v-for="option in menuOptions"
          :key="option.label"
          :class="[$style.dropdownItem, { [$style.dangerItem]: option.isDanger }]"
          @click="option.action"
        >
          <span :class="$style.itemIcon"><AppIcon :name="option.icon" :size="16" /></span>
          <span :class="$style.itemLabel">{{ option.label }}</span>
          <AppIcon v-if="option.hasSubmenu" name="chevron-right" :size="14" :class="$style.itemArrow" />
        </button>
      </div>

      <div v-else :class="$style.submenu">
        <button :class="$style.dropdownItem" @click="showPlaylistSubmenu = false">
          <span :class="$style.itemIcon"><AppIcon name="chevron-left" :size="14" /></span>
          <span :class="$style.itemLabel">Назад</span>
        </button>
        <div :class="$style.divider"></div>
        <button
          v-for="pl in filteredPlaylists"
          :key="pl.id"
          :class="$style.dropdownItem"
          @click="handleAddToPlaylist(pl.id)"
        >
          <span :class="$style.itemIcon"><AppIcon name="playlist" :size="16" /></span>
          <span :class="$style.itemLabel">{{ pl.title }}</span>
        </button>
        <div v-if="playlists.length === 0" :class="$style.emptyPlaylist">
          Нет плейлистов
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, ref, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { TrackDto, PlaylistDto, ContainerMetaDto } from '@/types/types';
import { useAudioStore } from '@/stores/audio';
import MarqueeText from './MarqueeText.vue';
import AppIcon from './AppIcon.vue';
const store = useAudioStore()

interface Props {
  type: 'track' | 'playlist' | 'album';
  track?: TrackDto&{thumbnail:string};
  playlist?: PlaylistDto;
  album?: ContainerMetaDto;
  showAction?: boolean;
  context?: string;
  isLocal?: boolean;
  isInLibrary?: boolean;
  queueIndex?: number;
  containerMode?: boolean;
  isExpanded?: boolean;
  active?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  showAction: true,
  active: false
});

const emit = defineEmits<{
  click: [];
  play: [];
  refresh: [];
  toggleExpand: [];
  containerCreatePlaylist: [];
  containerAddToPlaylist: [playlistId: number];
  containerRemove: [];
  remove: [];
}>();

const dropdownOpen = ref(false);
const dropdownRef = ref<HTMLElement | null>(null);
const actionButtonRef = ref<HTMLElement | null>(null);
const dropdownPortalRef = ref<HTMLElement | null>(null);
const dropdownPos = ref({ top: 0, right: 0 });
const playlists = ref<Array<{ id: number; title: string }>>([]);
const showPlaylistSubmenu = ref(false);

const dropdownStyle = computed(() => ({
  top: `${dropdownPos.value.top}px`,
  right: `${dropdownPos.value.right}px`,
}));

const entity = computed(() => {

  if (props.type === 'track' && props.track) {
    return {
      id: props.track.id,
      title: props.track.title,
      artist: props.track.artist || 'Unknown Artist',
      thumbnail: props.track.artworkUrl||props.track.thumbnail,
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
  isDanger?: boolean;
}

const menuOptions = computed<MenuOption[]>(() => {
  const options: MenuOption[] = [];

  if (props.containerMode) {
    return [
      {
        label: 'Сохранить в плейлист',
        icon: 'playlist',
        action: handleShowPlaylistSubmenu,
        hasSubmenu: true,
      },
      {
        label: 'Сформировать плейлист',
        icon: 'music-note',
        action: () => {
          closeDropdown();
          emit('containerCreatePlaylist');
        },
      },
      {
        label: 'Удалить из очереди',
        icon: 'trash',
        isDanger: true,
        action: () => {
          closeDropdown();
          emit('containerRemove');
        },
      },
    ];
  }

  if (props.type === 'track') {
    options.push({
      label: 'Добавить в плейлист',
      icon: 'playlist',
      action: handleShowPlaylistSubmenu,
      hasSubmenu: true,
    });

    if (props.context === 'playlist') {
      options.push({
        label: 'Удалить из плейлиста',
        icon: 'trash',
        isDanger: true,
        action: handleRemoveFromPlaylist,
      });
    }
  } else if (props.type === 'playlist') {
    options.push({
      label: 'Добавить в плейлист',
      icon: 'playlist',
      action: handleShowPlaylistSubmenu,
      hasSubmenu: true,
    });
    return options;
  }

  if (props.type !== 'track') return options;

  if (props.context === 'queue') {
    options.push({
      label: 'Удалить из очереди',
      icon: 'trash',
      action: handleRemoveFromQueue,
    });
  } else {
    options.push({
      label: 'Добавить в очередь',
      icon: 'plus',
      action: handleAddToQueue,
    });
  }

  if (!props.isLocal) {
    if (isCached.value) {
      options.push({
        label: 'Удалить',
        icon: 'trash',
        isDanger: true,
        action: handleDeleteCache,
      });
    } else {
      options.push({
        label: 'Скачать',
        icon: 'download-tray',
        action: handleDownload,
      });
    }
  }

  // options.push({
  //   label: 'Сохранить в файл',
  //   icon: '💾',
  //   action: handleSaveToFile,
  // });

  if (props.context === 'queue' && !props.isInLibrary) {
    options.push({
      label: 'Добавить в библиотеку',
      icon: 'plus',
      action: handleAddToLibrary,
    });
  }

  return options;
});

const isCached = computed(() => props.track?.state?.type === 'cached');

const filteredPlaylists = computed(() => {
  if (props.type === 'playlist' && props.playlist) {
    return playlists.value.filter(p => p.id !== props.playlist!.id);
  }
  return playlists.value;
});

const placeholderIcon = computed(() => {
  switch (props.type) {
    case 'track':
      return 'music-note';
    case 'playlist':
      return 'folder';
    case 'album':
      return 'album';
    default:
      return 'music-note';
  }
});

const formatDuration = (seconds: number): string => {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

const handleDocumentClick = (e: MouseEvent) => {
  const target = e.target as Node;
  const insideActions = dropdownRef.value?.contains(target) ?? false;
  const insidePortal = dropdownPortalRef.value?.contains(target) ?? false;
  if (!insideActions && !insidePortal) {
    closeDropdown();
  }
};

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
    await nextTick();
    if (actionButtonRef.value) {
      const rect = actionButtonRef.value.getBoundingClientRect();
      const viewportHeight = window.innerHeight;
      
      // Initial position (below)
      let top = rect.bottom + 4;
      let right = window.innerWidth - rect.right;
      
      // Check if dropdown goes off screen
      if (dropdownPortalRef.value) {
        const dropdownHeight = dropdownPortalRef.value.offsetHeight;
        if (top + dropdownHeight > viewportHeight) {
          // Open upwards if not enough space below
          top = rect.top - dropdownHeight - 4;
        }
      }
      
      dropdownPos.value = { top, right };
    }
    if (playlists.value.length === 0) {
      await fetchPlaylists();
    }
    document.addEventListener('mousedown', handleDocumentClick);
  } else {
    document.removeEventListener('mousedown', handleDocumentClick);
  }
};

const closeDropdown = () => {
  dropdownOpen.value = false;
  showPlaylistSubmenu.value = false;
  document.removeEventListener('mousedown', handleDocumentClick);
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

const handleRemoveFromPlaylist = () => {
  emit('remove');
  closeDropdown();
};

const handleAddToPlaylist = async (playlistId: number) => {
  if (props.containerMode) {
    emit('containerAddToPlaylist', playlistId);
    closeDropdown();
    return;
  }
  try {
    if (props.type === 'track' && props.track) {
      const t = props.track as any;
      const track = {
        id: t.id ?? 0,
        title: t.title,
        artist: t.artist ?? null,
        album: t.album ?? null,
        durationSecs: t.durationSecs ?? null,
        artworkUrl: t.artworkUrl ?? t.thumbnail ?? null,
        playCount: t.playCount ?? 0,
        liked: t.liked ?? false,
        state: t.state ?? { type: 'streaming' },
        source: t.source ?? null,
      };
      await invoke('add_track_to_playlist', { playlistId, track });
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
  try {
    const t = props.track as any;
    const track = {
      id: t.id ?? 0,
      title: t.title,
      artist: t.artist ?? null,
      album: t.album ?? null,
      durationSecs: t.durationSecs ?? null,
      artworkUrl: t.artworkUrl ?? t.thumbnail ?? null,
      playCount: t.playCount ?? 0,
      liked: t.liked ?? false,
      state: t.state ?? { type: 'streaming' },
      source: t.source ?? null,
    };
    await invoke('save_track_to_file', { track });
    console.log('Download process started for:', entity.value.id);
  } catch (e) {
    console.error('Failed to start download process:', e);
  }
  closeDropdown();
};

const handleDeleteCache = async () => {
  if (!props.track) return;

  try {
    await invoke('delete_cached_track', { trackId: entity.value.id });
    props.track.state = { type: 'streaming' };
    emit('refresh');
  } catch (e) {
    console.error('Failed to delete cached track:', e);
  }
  closeDropdown();
};

const handleSaveToFile = async () => {
  try {
    await invoke('save_track_to_file', { track: entity.value });
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

const handleToggleLike = async () => {
console.log({
...props.track,
thumbnail: props?.track?.thumbnail || props.track?.artworkUrl, 
    },"AAALO")
  if (props.type !== 'track' || !props.track) return;
  
  try {
    props.track.liked =  !props.track.liked

    await invoke('toggle_liked', { track: {
...props.track,
artworkUrl: props.track.thumbnail || props.track?.artworkUrl, 
    } });
    emit('refresh');
  } catch (e) {
    console.error('Failed to toggle like:', e);
  }
};
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.mediaCard {
  display: flex;
  gap: $spacing-md;
  padding:  $spacing-md;
  background-color: var(--media-card-bg);
  color: var(--media-card-color);
  transition: all $transition-fast;
  position: relative;

  &:hover {
    background-color: var(--media-card-hover-bg);
    color: var(--media-card-hover-color);

    .playOverlay {
      opacity: 1;
    }

    .title, .title-active, .artist, .duration, .trackCount, .year, .separator {
      // color: var(--media-card-hover-color);
      color: var(--media-card-hover-color);
    }
  }

  &.dropdownActive {
    z-index: 1000;
  }

  &.active {
    background-color: var(--media-card-active-bg);
    color: var(--media-card-active-color);

    .title {
      color: var(--media-card-active-color);
    }
  }

  &.unavailable {
    opacity: 0.45;

    .content {
      pointer-events: none;
    }
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
  background: black;}

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
  width: 40px;
  height: 40px;
  padding: 0 !important;
  border-radius: 50%;
  clip-path: none !important;
  background-color: transparent !important;
  position: relative;
  isolation: isolate;
  color: white;
  border: none;
  @include flex-center;
  font-size: $font-size-base;
  cursor: pointer;
  transition: all $transition-fast;

  &::before {
    content: '';
    position: absolute;
    inset: 0;
    background-color: var(--color-accent-primary);
    border-radius: 50%;
    z-index: -1;
    transition: all $transition-fast;
  }

  &:hover {
    transform: scale(1.1);
    &::before {
      background-color: var(--media-card-hover-color);
    }
    img {
      // Иконка остается прежней, но на новом фоне она будет контрастна
      filter: none; 
    }
  }

  &:disabled {
    cursor: wait;
    opacity: 0.8;
  }
}

.unavailableIcon {
  font-size: 16px;
  line-height: 1;
}

.mini-loader {
  animation: pulse 1s infinite;
  font-size: 1.5rem;
}

@keyframes pulse {
  0% { opacity: 0.4; }
  50% { opacity: 1; }
  100% { opacity: 0.4; }
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
  @include text-ellipsis;
}
.title-active {
  font-size: $font-size-base;
  font-weight: $font-weight-semibold;
  @include text-ellipsis;
}

.meta {
  display: flex;
  align-items: center;
  gap: $spacing-xs;
  font-size: $font-size-sm;
  @include text-ellipsis;
  min-width: 0;
}

.artist {
  @include text-ellipsis;
  display: block;
  flex: 1;
  min-width: 0;
  .active & {
    color: var(--media-card-active-color);
  }
}

.separator {
  color: var(--color-text-muted);
  flex-shrink: 0;
}

.duration,
.trackCount,
.year {
  white-space: nowrap;
  flex-shrink: 0;
}

.actions {
  gap:8px;
  display: flex;
  align-items: center;
  position: relative;
}

.actionButton {
  width: 32px !important;
  height: 32px !important;
  padding: 0 !important;
  border-radius: 50%;
  clip-path: none !important;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  font-size: $font-size-xl;
  line-height: 1 !important;
  padding-bottom: var(--ellipsis-offset-bottom) !important;
  cursor: pointer;
  @include flex-center;
  align-items: var(--ellipsis-v-align, center) !important;
  transition: all $transition-fast;

  &:hover {
    background-color: var(--media-card-action-hover-bg);
    color: var(--media-card-action-hover-color);
  }
}

.likeButton {
  width: 32px;
  height: 32px;
  padding:0 !important;
  border-radius: 50%;
  clip-path: none !important;
  background: transparent;
  border: none;
  border-style:none !important;
  cursor: pointer;
  @include flex-center;
  transition: all $transition-fast;
  color: var(--color-text-muted);
  opacity: 0.5;

  &:hover {
    background-color: var(--color-bg-primary);
    transform: scale(1.1);
    opacity: 1;
    color: var(--color-text-secondary);
  }

  &.isLiked {
    color: #ef4444;
    opacity: 1;

    &:hover {
        color: #f87171;
    }
  }
}

.heartFilled,
.heartOutline {
  width: 32px;
  height: 32px;
  background-color: currentColor;
  -webkit-mask-repeat: no-repeat;
  mask-repeat: no-repeat;
  -webkit-mask-position: center;
  mask-position: center;
  -webkit-mask-size: contain;
  mask-size: contain;
}

.heartFilled {
  -webkit-mask-image: url('/icons/heart-filled.svg');
  mask-image: url('/icons/heart-filled.svg');
}

.heartOutline {
  -webkit-mask-image: url('/icons/heart-outline.svg');
  mask-image: url('/icons/heart-outline.svg');
}


.dropdown {
  position: fixed;
  background-color: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  min-width: 200px;
  z-index: 10000;
  overflow-y: auto;
  max-height: 400px;
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

.dangerItem {
  color: #e74c3c;

  &:hover {
    background-color: rgba(231, 76, 60, 0.1);
  }
}

.expandBtn {
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: $spacing-xs $spacing-md $spacing-xs $spacing-xs;
  font-size: $font-size-sm;
  flex-shrink: 0;
  @include flex-center;
  width: 32px;
  height: 32px;

  &:hover {
    color: var(--color-text-primary);
  }
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
