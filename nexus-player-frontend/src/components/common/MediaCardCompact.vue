<template>
  <div
    :class="[$style.mediaCardCompact, $style[`type-${type}`], { [$style.active]: props.active }]"
    @click="handleClick"
  >
    <div :class="$style.thumbnail">
      <img
        v-if="thumbnail"
        :src="thumbnail"
        :alt="title"
        :class="$style.image"
        decoding="async"
        loading="lazy"
      />
      <div v-else :class="$style.placeholder">
        <span :class="$style.placeholderIcon">{{ placeholderIcon }}</span>
      </div>

      <div :class="$style.playOverlay">
        <button :class="$style.playButton" @click.stop="handlePlay">
          <img src="/icon/play.svg" alt="Play">
        </button>
      </div>


      <button
        v-if="showAction"
        ref="actionButtonRef"
        style="position:absolute !important"
        :class="$style.menuButton"
        @click.stop="toggleMenu"
      >
        <span :class="$style.dots">⋮</span>
      </button>
    </div>

    <Teleport to="body">
      <div v-if="menuOpen" ref="dropdownRef" :class="$style.dropdownMenu" :style="dropdownStyle" @click.stop>
        <slot name="menu" :close="() => menuOpen = false">
          <template v-if="!showPlaylistSubmenu">
            <button :class="$style.menuItem" @click.stop="handleShowPlaylistSubmenu">
              Добавить в плейлист ›
            </button>
            <button :class="[$style.menuItem, $style.danger]" @click.stop="handleDelete">
              Удалить
            </button>
          </template>
          <template v-else>
            <button :class="$style.menuItem" @click.stop="showPlaylistSubmenu = false">
              ‹ Назад
            </button>
            <div :class="$style.divider"></div>
            <button
              v-for="pl in filteredPlaylists"
              :key="pl.id"
              :class="$style.menuItem"
              @click.stop="handleAddToPlaylist(pl.id)"
            >
              {{ pl.title }}
            </button>
            <div v-if="filteredPlaylists.length === 0" :class="$style.emptyHint">
              Нет плейлистов
            </div>
          </template>
        </slot>
      </div>
    </Teleport>

    <div :class="$style.info">
      <div :class="$style.title" :title="title">{{ title }}</div>
      <div :class="$style.artist" :title="artist">{{ artist }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  type: 'track' | 'playlist' | 'album';
  title: string;
  artist: string;
  thumbnail?: string;
  trackCount?: number;
  playlistId?: number;
  showAction?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  thumbnail: undefined,
  trackCount: undefined,
  playlistId: undefined,
  showAction: false,
});

const emit = defineEmits<{
  click: [];
  play: [];
  delete: [];
  action: [actionId: string];
}>();

const menuOpen = ref(false);
const showPlaylistSubmenu = ref(false);
const allPlaylists = ref<Array<{ id: number; title: string }>>([]);

const actionButtonRef = ref<HTMLElement | null>(null);
const dropdownRef = ref<HTMLElement | null>(null);
const dropdownPos = ref({ top: 0, left: 0 });

const filteredPlaylists = computed(() => {
  if (props.playlistId !== undefined) {
    return allPlaylists.value.filter(p => p.id !== props.playlistId);
  }
  return allPlaylists.value;
});

const dropdownStyle = computed(() => ({
  top: `${dropdownPos.value.top}px`,
  left: `${dropdownPos.value.left}px`,
}));

const toggleMenu = async () => {
  menuOpen.value = !menuOpen.value;
  showPlaylistSubmenu.value = false;
  
  if (menuOpen.value) {
    if (actionButtonRef.value) {
      const rect = actionButtonRef.value.getBoundingClientRect();
      dropdownPos.value = {
        top: rect.bottom + 4,
        left: rect.left
      };
    }
    
    if (allPlaylists.value.length === 0) {
      try {
        allPlaylists.value = await invoke<Array<{ id: number; title: string }>>('get_all_playlists');
      } catch (e) {
        console.error('Failed to fetch playlists:', e);
      }
    }
    document.addEventListener('mousedown', handleDocumentClick);
  } else {
    document.removeEventListener('mousedown', handleDocumentClick);
  }
};

const handleDocumentClick = (e: MouseEvent) => {
  const target = e.target as Node;
  const isInsideActionButton = actionButtonRef.value?.contains(target);
  const isInsideDropdown = dropdownRef.value?.contains(target);
  
  if (menuOpen.value && !isInsideActionButton && !isInsideDropdown) {
    menuOpen.value = false;
    document.removeEventListener('mousedown', handleDocumentClick);
  }
};

const handleShowPlaylistSubmenu = () => {
  showPlaylistSubmenu.value = true;
};

const handleAddToPlaylist = async (targetPlaylistId: number) => {
  if (props.playlistId === undefined) return;
  try {
    await invoke('add_playlist_to_playlist', {
      parentPlaylistId: targetPlaylistId,
      childPlaylistId: props.playlistId,
    });
    menuOpen.value = false;
    showPlaylistSubmenu.value = false;
  } catch (e) {
    console.error('Failed to add playlist to playlist:', e);
  }
};

const handleDelete = () => {
  menuOpen.value = false;
  emit('delete');
};

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

const typeBadgeText = computed(() => {
  if (props.type === 'playlist' && props.trackCount) {
    return `${props.trackCount} tracks`;
  }
  return props.type.toUpperCase();
});

const handleClick = () => {
  emit('click');
};

const handlePlay = () => {
  emit('play');
};
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.mediaCardCompact {
  display: flex;
  flex-direction: column;
  gap: $spacing-sm;
  cursor: pointer;
  transition: all $transition-fast;
  border-radius: $radius-md;
  padding: $spacing-sm;
  background-color: var(--media-card-bg, transparent);
  color: var(--media-card-color, inherit);

  &:hover {
    background-color: var(--media-card-hover-bg);
    color: var(--media-card-hover-color);
    transform: translateY(-4px);

    .playOverlay {
      opacity: 1;
    }

    .thumbnail {
      box-shadow: 0 8px 16px rgba(0, 0, 0, 0.3);
    }

    .title, .artist {
      color: var(--media-card-hover-color);
    }
  }

  &:active {
    transform: translateY(-2px);
  }
}

// Thumbnail
.thumbnail {
  position: relative;
  width: 100%;
  aspect-ratio: 1 / 1;
  border-radius: $radius-md;
  overflow: hidden;
  background-color: var(--color-bg-primary);
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.2);
  transition: box-shadow $transition-fast;
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
  background: black;
}

.placeholderIcon {
  font-size: 48px;
}

// Play Overlay
.playOverlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  @include flex-center;
  @include backdrop-blur(8px, 0.7);
  opacity: 0;
  transition: opacity $transition-fast;
}

.playButton {
  width: 48px;
  height: 48px;
  padding: 0px !important;
  border-radius: 50%;
  background-color: transparent !important;
  position: relative;
  isolation: isolate;
  color: white;
  border: none;
  @include flex-center;
  font-size: $font-size-lg;
  cursor: pointer;
  transition: all $transition-fast;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);

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
    transform: scale(1.15);
    &::before {
      background-color: var(--media-card-hover-color);
    }
    img {
      filter: none;
    }
  }
}

// Type Badge
.typeBadge {
  position: absolute;
  top: $spacing-sm;
  right: $spacing-sm;
  padding: $spacing-xs $spacing-sm;
  background-color: rgba(0, 0, 0, 0.8);
  color: white;
  font-size: $font-size-xs;
  font-weight: $font-weight-semibold;
  text-transform: uppercase;
  border-radius: $radius-sm;
  backdrop-filter: blur(4px);
}

// Menu Button
.menuButton {
  position: absolute;
  top: $spacing-sm;
  left: $spacing-sm;
  padding: 0 !important;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background-color: rgba(0, 0, 0, 0.7);
  color: white;
  border: none;
  @include flex-center;
  align-items: var(--ellipsis-v-align, center) !important;
  font-size: $font-size-lg;
  line-height: 1 !important;
  padding-bottom: var(--ellipsis-offset-bottom) !important;
  cursor: pointer;
  opacity: 0;
  transition: opacity $transition-fast, background-color $transition-fast, color $transition-fast;
  backdrop-filter: blur(4px);

  &:hover {
    background-color: var(--media-card-action-hover-bg, rgba(0, 0, 0, 0.9));
    color: var(--media-card-action-hover-color, white);
  }
}

.mediaCardCompact:hover .menuButton {
  opacity: 1;
}

.dots {
  font-weight: bold;
  font-size: 16px;
}

.dropdownMenu {
  position: fixed;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  z-index: 10000;
  min-width: 150px;
  overflow: hidden;
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
    background-color: var(--color-bg-secondary);
  }

  &.danger {
    color: #e74c3c;
  }
}

.divider {
  height: 1px;
  background-color: var(--color-border);
  margin: 2px 0;
}

.emptyHint {
  padding: $spacing-sm $spacing-md;
  font-size: $font-size-xs;
  color: var(--color-text-muted);
  text-align: center;
}

// Info Section
.info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.title {
  font-size: $font-size-sm;
  font-weight: $font-weight-semibold;
  @include text-ellipsis;
}

.artist {
  font-size: $font-size-xs;
  @include text-ellipsis;
}
</style>
