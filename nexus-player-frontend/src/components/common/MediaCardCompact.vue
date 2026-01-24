<template>
  <div
    :class="[$style.mediaCardCompact, $style[`type-${type}`]]"
    @click="handleClick"
  >
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

      <div :class="$style.playOverlay">
        <button :class="$style.playButton" @click.stop="handlePlay">▶</button>
      </div>

      <div v-if="type !== 'track'" :class="$style.typeBadge">
        {{ typeBadgeText }}
      </div>

      <button
        v-if="type === 'playlist'"
        :class="$style.menuButton"
        @click.stop="toggleMenu"
      >
        <span :class="$style.dots">⋮</span>
      </button>

      <div v-if="menuOpen" :class="$style.dropdownMenu" @click.stop>
        <button :class="$style.menuItem" @click.stop="handleDelete">
          Delete
        </button>
      </div>
    </div>

    <div :class="$style.info">
      <div :class="$style.title" :title="title">{{ title }}</div>
      <div :class="$style.artist" :title="artist">{{ artist }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';

interface Props {
  type: 'track' | 'playlist' | 'album';
  title: string;
  artist: string;
  thumbnail?: string;
  trackCount?: number;
}

const props = withDefaults(defineProps<Props>(), {
  thumbnail: undefined,
  trackCount: undefined,
});

const emit = defineEmits<{
  click: [];
  play: [];
  delete: [];
}>();

const menuOpen = ref(false);

const toggleMenu = () => {
  menuOpen.value = !menuOpen.value;
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
  background-color: transparent;

  &:hover {
    background-color: var(--color-bg-secondary);
    transform: translateY(-4px);

    .playOverlay {
      opacity: 1;
    }

    .thumbnail {
      box-shadow: 0 8px 16px rgba(0, 0, 0, 0.3);
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
  background: linear-gradient(
    135deg,
    var(--color-accent-primary) 0%,
    var(--color-accent-purple) 100%
  );
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
  border-radius: 50%;
  background-color: var(--color-accent-primary);
  color: white;
  border: none;
  @include flex-center;
  font-size: $font-size-lg;
  cursor: pointer;
  transition: all $transition-fast;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);

  &:hover {
    background-color: var(--color-accent-hover);
    transform: scale(1.15);
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
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background-color: rgba(0, 0, 0, 0.7);
  color: white;
  border: none;
  @include flex-center;
  font-size: $font-size-lg;
  cursor: pointer;
  opacity: 0;
  transition: opacity $transition-fast, background-color $transition-fast;
  backdrop-filter: blur(4px);

  &:hover {
    background-color: rgba(0, 0, 0, 0.9);
  }
}

.mediaCardCompact:hover .menuButton {
  opacity: 1;
}

.dots {
  font-weight: bold;
  line-height: 1;
}

.dropdownMenu {
  position: absolute;
  top: calc(#{$spacing-sm} + 32px);
  left: $spacing-sm;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  z-index: 100;
  min-width: 100px;
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
  color: #e74c3c;
  transition: background-color $transition-fast;

  &:hover {
    background-color: var(--color-bg-secondary);
  }
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
  color: var(--color-text-primary);
  @include text-ellipsis;
}

.artist {
  font-size: $font-size-xs;
  color: var(--color-text-secondary);
  @include text-ellipsis;
}
</style>
