<template>
  <div :class="[$style.mediaCard, $style[`type-${type}`]]" @click="handleClick">
    <div :class="$style.thumbnail">
      <img
        v-if="thumbnail"
        :src="thumbnail"
        :alt="title"
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
      <div :class="$style.title" :title="title">{{ title }}</div>
      <div :class="$style.meta">
        <span :class="$style.artist">{{ artist }}</span>

        <template v-if="type === 'track' && duration !== undefined">
          <span :class="$style.separator">•</span>
          <span :class="$style.duration">{{ formatDuration(duration) }}</span>
        </template>

        <template
          v-if="
            (type === 'playlist' || type === 'album') &&
            trackCount !== undefined
          "
        >
          <span :class="$style.separator">•</span>
          <span :class="$style.trackCount">{{ trackCount }} tracks</span>
        </template>

        <template v-if="type === 'album' && year">
          <span :class="$style.separator">•</span>
          <span :class="$style.year">{{ year }}</span>
        </template>
      </div>
    </div>

    <div v-if="showActions" :class="$style.actions">
      <button :class="$style.actionButton" @click.stop="handleMore">⋯</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

interface Props {
  type: 'track' | 'playlist' | 'album';
  title: string;
  artist: string;
  thumbnail?: string;
  duration?: number; // in seconds, for tracks
  trackCount?: number; // for playlists and albums
  year?: number; // for albums
  showActions?: boolean; // show more actions button
}

const props = withDefaults(defineProps<Props>(), {
  thumbnail: undefined,
  duration: undefined,
  trackCount: undefined,
  year: undefined,
  showActions: true,
});

const emit = defineEmits<{
  click: [];
  play: [];
  more: [];
}>();

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

const handleClick = () => {
  emit('click');
};

const handlePlay = () => {
  emit('play');
};

const handleMore = () => {
  emit('more');
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
  cursor: pointer;
  transition: all $transition-fast;
  position: relative;

  &:hover {
    background-color: var(--color-bg-hover);
    transform: translateY(-2px);

    .playOverlay {
      opacity: 1;
    }
  }

  &:active {
    transform: translateY(0);
  }
}

// Type-specific borders
.type-track {
  border-left: 3px solid #4caf50;
}

.type-playlist {
  border-left: 3px solid #2196f3;
}

.type-album {
  border-left: 3px solid #ff9800;
}

// Thumbnail
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

// Play Overlay
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

// Info Section
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

// Actions
.actions {
  display: flex;
  align-items: center;
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
</style>
