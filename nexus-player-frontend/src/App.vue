<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import TrackList from './components/TrackList.vue';
import PlaylistList from './components/PlaylistList.vue';
import PlayerControl from './components/PlayerControl.vue';

const trackListRef = ref<InstanceType<typeof TrackList> | null>(null);
const playlistListRef = ref<InstanceType<typeof PlaylistList> | null>(null);
const statusMessage = ref('');

// TODO(human): Implement the addTrackFromDialog function
// This function should:
// 1. Call 'select_audio_file' command to open file dialog
// 2. If a file is selected, extract filename and create track metadata
// 3. Call 'add_track_from_file' command with CreateTrackRequest
// 4. Reload the track list using trackListRef
// 5. Show success/error message in statusMessage
const addTrackFromDialog = async () => {
  const path = await invoke('select_audio_file');
  await invoke('add_track_from_file',{path:path});
  trackListRef.value?.reloadTracks()

};
</script>

<template>
  <main class="container">
    <h1>Nexus Audio Player</h1>

    <!-- Add Track Button and Status -->
    <div class="controls">
      <button class="add-track-btn" @click="addTrackFromDialog">
        Add Track
      </button>
      <p v-if="statusMessage" class="status-message">{{ statusMessage }}</p>
    </div>

    <!-- Track List Section -->
    <section class="section">
      <TrackList ref="trackListRef" />
    </section>

    <!-- Playlist List Section -->
    <section class="section">
      <PlaylistList ref="playlistListRef" />
    </section>

    <!-- Player Control -->
    <PlayerControl />
  </main>
</template>

<style scoped>
h1 {
  margin-bottom: 2rem;
  color: #0f0f0f;
}

.controls {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
  margin-bottom: 2rem;
}

.add-track-btn {
  padding: 0.8em 2em;
  font-size: 1.1rem;
  font-weight: 600;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  border: none;
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.3s ease;
  box-shadow: 0 4px 15px rgba(102, 126, 234, 0.4);
}

.add-track-btn:hover {
  transform: translateY(-2px);
  box-shadow: 0 6px 20px rgba(102, 126, 234, 0.6);
}

.add-track-btn:active {
  transform: translateY(0);
}

.status-message {
  font-size: 0.9rem;
  color: #666;
  font-style: italic;
}

.section {
  margin-bottom: 3rem;
  width: 100%;
}

.container {
  padding-bottom: 180px;
}

@media (prefers-color-scheme: dark) {
  h1 {
    color: #f6f6f6;
  }

  .status-message {
    color: #aaa;
  }
}
</style>
<style>
:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: #0f0f0f;
  background-color: #f6f6f6;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

.container {
  margin: 0;
  padding: 2rem;
  max-width: 1200px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: 0.75s;
}

.logo.tauri:hover {
  filter: drop-shadow(0 0 2em #24c8db);
}

.row {
  display: flex;
  justify-content: center;
}

a {
  font-weight: 500;
  color: #646cff;
  text-decoration: inherit;
}

a:hover {
  color: #535bf2;
}

h1 {
  text-align: center;
}

input,
button {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  color: #0f0f0f;
  background-color: #ffffff;
  transition: border-color 0.25s;
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
}

button {
  cursor: pointer;
}

button:hover {
  border-color: #396cd8;
}
button:active {
  border-color: #396cd8;
  background-color: #e8e8e8;
}

input,
button {
  outline: none;
}

#greet-input {
  margin-right: 5px;
}

@media (prefers-color-scheme: dark) {
  :root {
    color: #f6f6f6;
    background-color: #2f2f2f;
  }

  a:hover {
    color: #24c8db;
  }

  input,
  button {
    color: #ffffff;
    background-color: #0f0f0f98;
  }
  button:active {
    background-color: #0f0f0f69;
  }
}
</style>
