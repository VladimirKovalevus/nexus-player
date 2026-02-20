<template>
  <footer :class="$style.playerBar">
    <div :class="$style.trackInfo">
      <div :class="$style.trackCover">
        <img v-if="currentTrack?.artworkUrl" :src="currentTrack.artworkUrl" :alt="currentTrack.title" />
        <span v-else>🎵</span>
      </div>
      <div :class="$style.trackDetails">
        <div :class="$style.trackName">{{ currentTrack?.title || 'Нет воспроизведения' }}</div>
        <div :class="$style.trackArtist">{{ currentTrack?.artist || '—' }}</div>
      </div>
      <button :class="$style.likeButton">♡</button>
    </div>

    <div :class="$style.playerControls">
      <div :class="$style.controls">
        <button :class="$style.controlButton" @click="playPrevious">⏮</button>
        <button :class="[$style.controlButton, $style.playButton]" @click="togglePlay" :disabled="isLoading">
          <div v-if="isLoading" :class="$style.spinner"></div>
          <template v-else>{{ isPlaying ? '⏸' : '▶' }}</template>
        </button>
        <button :class="$style.controlButton" @click="playNext">⏭</button>
      </div>
      <div :class="$style.progress">
        <span :class="$style.time">{{ formatTime(position) }}</span>
        <WaveformBar />
        <span :class="$style.time">{{ formatTime(duration) }}</span>
      </div>
    </div>

    <div :class="$style.rightControls">
      <button :class="$style.controlButton">🔊</button>
      <div :class="$style.volumeBar">
        <div :class="$style.volumeFilled" :style="{ width: '70%' }"></div>
      </div>
      <button :class="$style.controlButton">⚙️</button>
    </div>
  </footer>
</template>

<script setup lang="ts">
import { useAudioStore } from '@/stores/audio';
import WaveformBar from '@/components/common/WaveformBar.vue';
import { invoke } from '@tauri-apps/api/core';
import { computed } from 'vue';

const audioStore = useAudioStore();

const isPlaying = computed(() => audioStore.isPlaying);
const isLoading = computed(() => audioStore.isLoading);
const currentTrack = computed(() => audioStore.currentTrack);
const position = computed(() => audioStore.position);
const duration = computed(() => audioStore.duration);

const formatTime = (seconds: number) => {
  if (isNaN(seconds)) return '0:00';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

const togglePlay = async () => {
  if (isPlaying.value) {
    await invoke('pause');
  } else {
    await invoke('resume');
  }
};

const playNext = () => invoke('play_next');
const playPrevious = () => invoke('play_previous');
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.playerBar {
  height: $player-height;
  background-color: $color-bg-player;
  border-top: 1px solid $color-border;
  padding: 0 $spacing-lg;
  display: grid;
  grid-template-columns: 1fr 2fr 1fr;
  gap: $spacing-lg;
  align-items: center;
}

// Left: Track Info
.trackInfo {
  @include flex-start;
  gap: $spacing-md;
}

.trackCover {
  width: 56px;
  height: 56px;
  background-color: $color-bg-secondary;
  border-radius: $radius-md;
  @include flex-center;
  font-size: $font-size-xl;
}

.trackDetails {
  flex: 1;
  min-width: 0;
}

.trackName {
  color: $color-text-primary;
  font-weight: $font-weight-medium;
  font-size: $font-size-base;
  @include text-ellipsis;
}

.trackArtist {
  color: $color-text-secondary;
  font-size: $font-size-sm;
  @include text-ellipsis;
}

.likeButton {
  background: transparent;
  border: none;
  color: $color-text-secondary;
  font-size: $font-size-xl;
  cursor: pointer;
  transition: color $transition-fast;

  &:hover {
    color: $color-text-primary;
  }
}

// Center: Player Controls
.playerControls {
  display: flex;
  flex-direction: column;
  gap: $spacing-sm;
  align-items: center;
}

.controls {
  @include flex-center;
  gap: $spacing-md;
}

.controlButton {
  background: transparent;
  border: none;
  color: $color-text-primary;
  font-size: $font-size-lg;
  cursor: pointer;
  transition: all $transition-fast;
  width: 32px;
  height: 32px;
  @include flex-center;
  border-radius: 50%;

  &:hover {
    background-color: rgba(255, 255, 255, 0.1);
  }
}

.playButton {
  width: 40px;
  height: 40px;
  background-color: $color-accent-primary;
  font-size: $font-size-xl;

  &:hover {
    background-color: $color-accent-hover;
  }
}

.progress {
  width: 100%;
  @include flex-center;
  gap: $spacing-md;
}

.time {
  color: $color-text-secondary;
  font-size: $font-size-xs;
  min-width: 40px;
  text-align: center;
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-radius: 50%;
  border-top-color: #fff;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

// Right: Volume & Settings
.rightControls {
  @include flex-center;
  gap: $spacing-sm;
  justify-self: end;
}

.volumeBar {
  width: 100px;
  height: 4px;
  background-color: $color-bg-secondary;
  border-radius: 2px;
  position: relative;
  cursor: pointer;

  &:hover {
    height: 6px;
  }
}

.volumeFilled {
  height: 100%;
  background-color: $color-accent-primary;
  border-radius: 2px;
  transition: width $transition-fast;
}
</style>
