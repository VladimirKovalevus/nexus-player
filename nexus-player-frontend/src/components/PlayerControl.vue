<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface AudioState {
  state: 'playing' | 'paused' | 'stopped' | 'buffering';
  positionSecs: number;
  durationSecs: number;
  volume: number;
  currentTrack: {
    index: number;
    trackId: number;
    title: string;
    source: string;
  } | null;
}

interface QueueState {
  repeatMode: 'off' | 'all' | 'one';
  shuffleEnabled: boolean;
}

const audioState = ref<AudioState>({
  state: 'stopped',
  positionSecs: 0,
  durationSecs: 0,
  volume: 1.0,
  currentTrack: null
});

const queueState = ref<QueueState>({
  repeatMode: 'off',
  shuffleEnabled: false
});

const seeking = ref(false);
let updateInterval: number | null = null;

const fetchAudioState = async () => {
  try {
    audioState.value = await invoke('get_audio_state');
  } catch (e) {
    console.error('Failed to fetch audio state:', e);
  }
};

const fetchQueueState = async () => {
  try {
    const state = await invoke('get_queue_state');
    queueState.value.repeatMode = state.repeatMode;
    queueState.value.shuffleEnabled = state.shuffleEnabled;
  } catch (e) {
    console.error('Failed to fetch queue state:', e);
  }
};

const togglePlayPause = async () => {
  try {
    if (audioState.value.state === 'playing') {
      await invoke('pause');
    } else if (audioState.value.state === 'paused') {
      await invoke('resume');
    }
    await fetchAudioState();
  } catch (e) {
    console.error('Failed to toggle play/pause:', e);
  }
};

const playNext = async () => {
  try {
    await invoke('play_next');
    await fetchAudioState();
  } catch (e) {
    console.error('Failed to play next:', e);
  }
};

const playPrevious = async () => {
  try {
    await invoke('play_previous');
    await fetchAudioState();
  } catch (e) {
    console.error('Failed to play previous:', e);
  }
};

const onSeekStart = () => {
  seeking.value = true;
};

const onSeekEnd = async (event: Event) => {
  const target = event.target as HTMLInputElement;
  const position = parseFloat(target.value);

  try {
    await invoke('seek', { positionSecs: position });
    seeking.value = false;
    await fetchAudioState();
  } catch (e) {
    console.error('Failed to seek:', e);
    seeking.value = false;
  }
};

const cycleRepeatMode = async () => {
  const modes: Array<'off' | 'all' | 'one'> = ['off', 'all', 'one'];
  const currentIndex = modes.indexOf(queueState.value.repeatMode);
  const nextMode = modes[(currentIndex + 1) % modes.length];

  try {
    await invoke('set_repeat_mode', { mode: nextMode });
    queueState.value.repeatMode = nextMode;
  } catch (e) {
    console.error('Failed to set repeat mode:', e);
  }
};

const toggleShuffleMode = async () => {
  try {
    await invoke('toggle_shuffle');
    queueState.value.shuffleEnabled = !queueState.value.shuffleEnabled;
  } catch (e) {
    console.error('Failed to toggle shuffle:', e);
  }
};

const getRepeatIcon = (): string => {
  switch (queueState.value.repeatMode) {
    case 'off': return '🔁';
    case 'all': return '🔁';
    case 'one': return '🔂';
    default: return '🔁';
  }
};

const formatTime = (seconds: number): string => {
  if (!seconds || !isFinite(seconds)) return '0:00';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

onMounted(() => {
  fetchAudioState();
  fetchQueueState();
  updateInterval = window.setInterval(fetchAudioState, 1000);
});

onUnmounted(() => {
  if (updateInterval !== null) {
    clearInterval(updateInterval);
  }
});
</script>

<template>
  <div class="player-control">
    <div class="track-info">
      <div v-if="audioState.currentTrack" class="current-track">
        {{ audioState.currentTrack.title }}
      </div>
      <div v-else class="no-track">No track playing</div>
    </div>

    <div class="controls">
      <button
        class="control-btn small-btn"
        @click="toggleShuffleMode"
        :class="{ active: queueState.shuffleEnabled }"
        title="Shuffle"
      >
        🔀
      </button>
      <button class="control-btn" @click="playPrevious" title="Previous">
        ⏮
      </button>
      <button class="control-btn play-btn" @click="togglePlayPause" title="Play/Pause">
        {{ audioState.state === 'playing' ? '⏸' : '▶' }}
      </button>
      <button class="control-btn" @click="playNext" title="Next">
        ⏭
      </button>
      <button
        class="control-btn small-btn"
        @click="cycleRepeatMode"
        :class="{ active: queueState.repeatMode !== 'off' }"
        :title="`Repeat: ${queueState.repeatMode}`"
      >
        {{ getRepeatIcon() }}
      </button>
    </div>

    <div class="playback-bar">
      <span class="time">{{ formatTime(audioState.positionSecs) }}</span>
      <input
        type="range"
        class="seek-bar"
        :value="audioState.positionSecs"
        :max="audioState.durationSecs || 0"
        step="0.1"
        @mousedown="onSeekStart"
        @touchstart="onSeekStart"
        @change="onSeekEnd"
      />
      <span class="time">{{ formatTime(audioState.durationSecs) }}</span>
    </div>
  </div>
</template>

<style scoped>
.player-control {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  background-color: #ffffff;
  border-top: 2px solid #e0e0e0;
  padding: 1rem 2rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  box-shadow: 0 -4px 12px rgba(0, 0, 0, 0.1);
}

.track-info {
  text-align: center;
}

.current-track {
  font-weight: 600;
  font-size: 1rem;
  color: #0f0f0f;
}

.no-track {
  font-size: 0.9rem;
  color: #999;
}

.controls {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 1rem;
}

.control-btn {
  background: none;
  border: 2px solid #396cd8;
  border-radius: 50%;
  width: 48px;
  height: 48px;
  font-size: 1.2rem;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  color: #396cd8;
}

.control-btn:hover {
  background-color: #396cd8;
  color: white;
  transform: scale(1.05);
}

.play-btn {
  width: 56px;
  height: 56px;
  font-size: 1.4rem;
  border-width: 3px;
}

.small-btn {
  width: 40px;
  height: 40px;
  font-size: 1rem;
}

.control-btn.active {
  background-color: #396cd8;
  color: white;
}

.playback-bar {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.time {
  font-family: 'Courier New', monospace;
  font-size: 0.875rem;
  color: #666;
  min-width: 40px;
  text-align: center;
}

.seek-bar {
  flex: 1;
  height: 6px;
  -webkit-appearance: none;
  appearance: none;
  background: #e0e0e0;
  border-radius: 3px;
  outline: none;
  cursor: pointer;
}

.seek-bar::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 16px;
  height: 16px;
  background: #396cd8;
  border-radius: 50%;
  cursor: pointer;
  transition: all 0.2s;
}

.seek-bar::-webkit-slider-thumb:hover {
  transform: scale(1.2);
  background: #2856b8;
}

.seek-bar::-moz-range-thumb {
  width: 16px;
  height: 16px;
  background: #396cd8;
  border-radius: 50%;
  cursor: pointer;
  border: none;
  transition: all 0.2s;
}

.seek-bar::-moz-range-thumb:hover {
  transform: scale(1.2);
  background: #2856b8;
}

@media (prefers-color-scheme: dark) {
  .player-control {
    background-color: #1a1a1a;
    border-top-color: #333;
  }

  .current-track {
    color: #f6f6f6;
  }

  .no-track {
    color: #666;
  }

  .control-btn {
    border-color: #24c8db;
    color: #24c8db;
  }

  .control-btn:hover {
    background-color: #24c8db;
    color: #0f0f0f;
  }

  .control-btn.active {
    background-color: #24c8db;
    color: #0f0f0f;
  }

  .time {
    color: #aaa;
  }

  .seek-bar {
    background: #333;
  }

  .seek-bar::-webkit-slider-thumb {
    background: #24c8db;
  }

  .seek-bar::-webkit-slider-thumb:hover {
    background: #1ca8c8;
  }

  .seek-bar::-moz-range-thumb {
    background: #24c8db;
  }

  .seek-bar::-moz-range-thumb:hover {
    background: #1ca8c8;
  }
}
</style>
