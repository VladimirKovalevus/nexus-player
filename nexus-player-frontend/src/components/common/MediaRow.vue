<template>
  <div
    :class="[
      $style.row,
      layout === 'compact' ? $style.compact : 'track-grid',
      {
        [$style.active]: active,
        [$style.unavailable]: track?.unavailable,
      },
    ]"
    @click="handleClick"
  >
    <span v-if="layout === 'table'" class="track-col-index" :class="$style.index">
      <AppIcon v-if="draggable" name="drag" :size="14" :class="$style.grip" />
      <template v-else-if="index !== undefined">{{ String(index + 1).padStart(3, '0') }}</template>
    </span>

    <div :class="$style.art">
      <AppImage :src="entity.thumbnail" :alt="entity.title" :class="$style.image">
        <template #placeholder>
          <span :class="$style.placeholder"><AppIcon :name="placeholderIcon" :size="14" /></span>
        </template>
      </AppImage>
      <button
        v-if="type === 'track'"
        :class="$style.play"
        :title="$t('player.play')"
        :aria-label="$t('player.play')"
        :disabled="(store.isLoading && active) || track?.unavailable"
        @click.stop="handlePlay"
      >
        <AppIcon v-if="store.isLoading && active" name="loader-dot" :size="14" />
        <AppIcon v-else-if="track?.unavailable" name="x-circle" :size="14" />
        <AppIcon v-else name="play" :size="14" />
      </button>
    </div>

    <div :class="$style.main">
      <div :class="$style.title" :title="entity.title">{{ entity.title }}</div>
      <div :class="[$style.artist, { 'track-row-artist': layout === 'table' }]">{{ entity.artist }}</div>
    </div>

    <template v-if="layout === 'table'">
      <span class="track-col-artist" :class="$style.cell" :title="entity.artist">{{ entity.artist }}</span>
      <span class="track-col-album" :class="$style.cell">{{ track?.album ?? '' }}</span>
      <span class="track-col-source"><span v-if="sourceLabel" class="source-tag">{{ sourceLabel }}</span></span>
    </template>

    <span :class="$style.time">
      <template v-if="type === 'track'">{{ entity.duration != null ? formatDuration(entity.duration) : '' }}</template>
      <template v-else-if="entity.trackCount !== undefined">{{ entity.trackCount }}</template>
    </span>

    <button
      v-if="type === 'track'"
      :class="[$style.action, { [$style.liked]: track?.liked }]"
      :title="track?.liked ? $t('media.unlike') : $t('media.like')"
      :aria-label="track?.liked ? $t('media.unlike') : $t('media.like')"
      @click.stop="handleToggleLike"
    >
      <AppIcon :name="track?.liked ? 'heart-filled' : 'heart-outline'" :size="14" />
    </button>
    <button
      v-else-if="containerMode || expandable"
      :class="$style.action"
      :title="$t(isExpanded ? 'media.collapse' : 'media.expand')"
      :aria-label="$t(isExpanded ? 'media.collapse' : 'media.expand')"
      @click.stop="emit('toggleExpand')"
    >
      <AppIcon :name="isExpanded ? 'chevron-up' : 'chevron-down'" :size="14" />
    </button>
    <span v-else />

    <ActionMenu
      v-if="showAction && menuOptions.length > 0"
      ref="menuRef"
      :label="$t('common.more')"
      :trigger-class="$style.action"
      @close="showPlaylistSubmenu = false"
    >
      <template #default>
        <PlaylistPicker
          v-if="showPlaylistSubmenu"
          show-back
          :exclude-id="pickerExcludeId"
          @back="showPlaylistSubmenu = false"
          @pick="handleAddToPlaylist"
        />
        <template v-else>
          <MenuItem
            v-for="option in menuOptions"
            :key="option.label"
            :icon="option.icon"
            :danger="option.isDanger"
            :submenu="option.hasSubmenu"
            @click="option.action"
          >
            {{ option.label }}
          </MenuItem>
        </template>
      </template>
    </ActionMenu>
    <span v-else />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@/utils/providerAuth';
import type { TrackDto, PlaylistDto, ContainerMetaDto } from '@/types/types';
import { useAudioStore } from '@/stores/audio';
import { usePlaylistActions } from '@/composables/usePlaylistActions';
import ActionMenu from './ActionMenu.vue';
import MenuItem from './MenuItem.vue';
import PlaylistPicker from './PlaylistPicker.vue';
import AppIcon from './AppIcon.vue';
import AppImage from './AppImage.vue';
const store = useAudioStore();
const { t } = useI18n();
const { addToPlaylist } = usePlaylistActions();

interface Props {
  type: 'track' | 'playlist' | 'album';
  /** `compact` is the two-line row of the narrow queue panel. */
  layout?: 'table' | 'compact';
  /** Row number; the drag grip replaces it when `draggable` is set. */
  index?: number;
  draggable?: boolean;
  /** Shows the chevron without the queue and search container menu. */
  expandable?: boolean;
  track?: TrackDto & { thumbnail?: string };
  playlist?: PlaylistDto;
  album?: ContainerMetaDto;
  showAction?: boolean;
  context?: string;
  isLocal?: boolean;
  queueIndex?: number;
  containerMode?: boolean;
  isExpanded?: boolean;
  active?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  layout: 'table',
  showAction: true,
  active: false,
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
  open: [];
}>();

const showPlaylistSubmenu = ref(false);

const entity = computed(() => {

  if (props.type === 'track' && props.track) {
    return {
      id: props.track.id,
      title: props.track.title,
      artist: props.track.artist || t('track.unknownArtist'),
      thumbnail: props.track.artworkUrl||props.track.thumbnail,
      duration: props.track.durationSecs,
      trackCount: undefined,
      year: undefined,
    };
  } else if (props.type === 'playlist' && props.playlist) {
    return {
      id: props.playlist.id,
      title: props.playlist.title,
      artist: props.playlist.creator || t('track.unknownCreator'),
      thumbnail: props.playlist.thumbnail || undefined,
      duration: undefined,
      trackCount: props.playlist.trackCount,
      year: undefined,
    };
  } else if (props.type === 'album' && props.album) {
    return {
      id: props.album.id,
      title: props.album.title,
      artist: props.album.artist || t('track.unknownArtist'),
      thumbnail: props.album.thumbnail || undefined,
      duration: undefined,
      trackCount: props.album.trackCount,
      year: undefined,
    };
  }

  return {
    id: 0,
    title: t('track.unknown'),
    artist: t('track.unknown'),
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
  if (props.containerMode) {
    return [
      { label: t('media.saveToPlaylist'), icon: 'playlist', action: handleShowPlaylistSubmenu, hasSubmenu: true },
      {
        label: t('queue.createPlaylist'),
        icon: 'music-note',
        action: () => {
          closeMenu();
          emit('containerCreatePlaylist');
        },
      },
      {
        label: t(props.context === 'queue' ? 'media.removeFromQueue' : 'media.removeFromResults'),
        icon: 'trash',
        isDanger: true,
        action: () => {
          closeMenu();
          emit('containerRemove');
        },
      },
    ];
  }

  const addToPlaylist: MenuOption = {
    label: t('media.addToPlaylist'),
    icon: 'playlist',
    action: handleShowPlaylistSubmenu,
    hasSubmenu: true,
  };

  if (props.type === 'playlist') {
    if (props.context !== 'playlist') return [addToPlaylist];
    return [
      {
        label: t('media.open'),
        icon: 'folder',
        action: () => {
          closeMenu();
          emit('open');
        },
      },
      addToPlaylist,
      { label: t('media.removeFromPlaylist'), icon: 'trash', isDanger: true, action: handleRemoveFromPlaylist },
    ];
  }

  if (props.type !== 'track') return [];

  const options: MenuOption[] = [addToPlaylist];

  if (props.context === 'playlist') {
    options.push({ label: t('media.removeFromPlaylist'), icon: 'trash', isDanger: true, action: handleRemoveFromPlaylist });
  }

  if (props.context === 'queue') {
    options.push({ label: t('media.removeFromQueue'), icon: 'trash', action: handleRemoveFromQueue });
  } else {
    options.push({ label: t('media.addToQueue'), icon: 'plus', action: handleAddToQueue });
  }

  if (!props.isLocal) {
    if (isCached.value) {
      options.push({ label: t('media.deleteFromCache'), icon: 'trash', isDanger: true, action: handleDeleteCache });
    } else {
      options.push({ label: t('media.download'), icon: 'download-tray', action: handleDownload });
    }
  }

  return options;
});

const isCached = computed(() => props.track?.state?.type === 'cached');

const pickerExcludeId = computed(() => (props.type === 'playlist' ? props.playlist?.id : undefined));

const sourceLabel = computed(() => {
  if (props.type === 'track') {
    const system = props.track?.source?.system;
    if (system) return system;
    return t(isCached.value ? 'track.source.cached' : 'track.source.local');
  }
  return (props.album as { system?: string } | undefined)?.system ?? props.playlist?.source?.system ?? '';
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

const handleClick = () => {
  emit('click');
};

const handlePlay = () => {
  emit('play');
};

const menuRef = ref<InstanceType<typeof ActionMenu> | null>(null);

const closeMenu = () => {
  menuRef.value?.close();
  showPlaylistSubmenu.value = false;
};

const handleShowPlaylistSubmenu = () => {
  showPlaylistSubmenu.value = true;
};

const handleRemoveFromPlaylist = () => {
  emit('remove');
  closeMenu();
};

const handleAddToPlaylist = async (playlistId: number) => {
  if (props.containerMode) {
    emit('containerAddToPlaylist', playlistId);
    closeMenu();
    return;
  }
  closeMenu();
  if (props.type === 'track' && props.track) {
    const source = props.track as any;
    const track = {
      id: source.id ?? 0,
      title: source.title,
      artist: source.artist ?? null,
      album: source.album ?? null,
      durationSecs: source.durationSecs ?? null,
      artworkUrl: source.artworkUrl ?? source.thumbnail ?? null,
      playCount: source.playCount ?? 0,
      liked: source.liked ?? false,
      state: source.state ?? { type: 'streaming' },
      source: source.source ?? null,
    };
    await addToPlaylist(playlistId, track.title, 'add_track_to_playlist', { playlistId, track });
  } else if (props.type === 'playlist' && props.playlist) {
    await addToPlaylist(playlistId, props.playlist.title, 'add_playlist_to_playlist', {
      parentPlaylistId: playlistId,
      childPlaylistId: props.playlist.id,
    });
  }
};

const handleAddToQueue = async () => {
  if (props.type !== 'track') return;

  try {
    await invoke('add_to_queue', { trackId: entity.value.id });
    emit('refresh');
    closeMenu();
  } catch (e) {
    console.error('Failed to add track to queue:', e);
  }
};

const handleRemoveFromQueue = async () => {
  if (props.queueIndex === undefined) return;

  try {
    await invoke('remove_from_queue', { index: props.queueIndex });
    emit('refresh');
    closeMenu();
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
  } catch (e) {
    console.error('Failed to start download process:', e);
  }
  closeMenu();
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
  closeMenu();
};

const handleToggleLike = async () => {
  if (props.type !== 'track' || !props.track) return;

  try {
    props.track.liked =  !props.track.liked

    await invoke('toggle_liked', { track: {
...props.track,
// Same precedence as `entity` and handleAddToPlaylist: library DTOs carry
// artworkUrl, streaming results carry thumbnail.
artworkUrl: props.track.artworkUrl ?? props.track.thumbnail ?? null,
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

.row {
  height: var(--row-height);
  border-bottom: 1px solid var(--line-soft);
  cursor: pointer;

  &:hover {
    background: var(--bg-selected);
  }
}

.compact {
  display: grid;
  grid-template-columns: 30px minmax(0, 1fr) auto 26px 26px;
  align-items: center;
  column-gap: 8px;
  height: var(--row-height-compact);
  padding: 0 4px 0 10px;
}

.active {
  background: var(--bg-selected);
  color: var(--accent);

  .title {
    font-weight: 600;
  }
}

.unavailable {
  opacity: 0.5;
}

.index {
  font: 500 var(--font-size-sm) $font-family-mono;
  color: var(--muted);
  text-align: right;
}

.grip {
  cursor: grab;
}

.art {
  position: relative;
  width: 30px;
  height: 30px;
  overflow: hidden;
  background: var(--key);

  &:hover .play,
  .play:focus-visible {
    opacity: 1;
  }
}

.image {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.placeholder {
  @include flex-center;
  width: 100%;
  height: 100%;
  color: var(--muted);
}

.play {
  @include flex-center;
  position: absolute;
  inset: 0;
  background: var(--overlay);
  color: var(--screen-text);
  opacity: 0;
}

.main {
  min-width: 0;
  line-height: 1.2;
}

.title {
  font-weight: 500;
  @include text-ellipsis;
}

.artist {
  font-size: var(--font-size-xs);
  color: var(--muted);
  @include text-ellipsis;
}

.active .artist,
.active .cell,
.active .time,
.active .index {
  color: inherit;
}

.cell {
  color: var(--muted);
  @include text-ellipsis;
}

.time {
  font: 500 var(--font-size-sm) $font-family-mono;
  color: var(--muted);
  text-align: right;
  white-space: nowrap;
}

.action {
  @include flex-center;
  width: 26px;
  height: 26px;
  color: var(--muted);

  &:hover {
    color: var(--text);
  }
}

.liked {
  color: var(--accent);
}

@media (max-width: $mobile-breakpoint) {
  .art {
    width: 40px;
    height: 40px;
  }

  .compact {
    grid-template-columns: 40px minmax(0, 1fr) auto 34px 34px;
  }

  .action {
    width: 34px;
    height: 44px;
  }
}
</style>
