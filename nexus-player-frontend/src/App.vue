<script setup lang="ts">
import { ref, provide } from 'vue';
import AppLayout from './components/layout/AppLayout.vue';
import PlaybackQueue from './components/PlaybackQueue.vue';
import PlayerControl from './components/PlayerControl.vue';
import TopBar from './components/layout/TopBar.vue';
import Sidebar from './components/layout/Sidebar.vue';
import MobileNav from './components/layout/MobileNav.vue';
import HomeView from './views/HomeView.vue';
import PlaylistDetailView from './views/PlaylistDetailView.vue';
import VkView from './views/VkView.vue';
import StreamingSearch from './components/StreamingSearch.vue';

const playbackQueueRef = ref<InstanceType<typeof PlaybackQueue> | null>(null);
const playerControlRef = ref<InstanceType<typeof PlayerControl> | null>(null);
const showQueue = ref(true);
const currentView = ref('home');
const selectedPlaylistId = ref<number | null>(null);
const playlistHistory = ref<number[]>([]);

const refreshQueue = () => {
  playbackQueueRef.value?.fetchQueue();
  playerControlRef.value?.fetchAudioState();
};

const handleSearchRefetch = () => {
  refreshQueue();
};

const toggleQueue = () => {
  showQueue.value = !showQueue.value;
};

const changeView = (view: string) => {
  currentView.value = view;
  selectedPlaylistId.value = null;
  playlistHistory.value = [];
};

const navigateToPlaylist = (playlistId: number) => {
  if (selectedPlaylistId.value !== null) {
    playlistHistory.value.push(selectedPlaylistId.value);
  }
  selectedPlaylistId.value = playlistId;
  currentView.value = 'home';
};

const navigateBackFromPlaylist = () => {
  if (playlistHistory.value.length > 0) {
    selectedPlaylistId.value = playlistHistory.value.pop()!;
  } else {
    selectedPlaylistId.value = null;
  }
};

provide('refreshQueue', refreshQueue);
provide('navigateToPlaylist', navigateToPlaylist);
</script>

<template>
  <AppLayout :show-queue="showQueue" @close-queue="toggleQueue">
    <template #topBar>
      <TopBar @refetch="handleSearchRefetch" />
    </template>

    <template #leftSidebar>
      <Sidebar :active-view="currentView" @change-view="changeView" />
    </template>

    <template #bottomPlayer>
      <PlayerControl
        ref="playerControlRef"
        :queue-visible="showQueue"
        @toggle-queue="toggleQueue"
      />
    </template>

    <template #mobileNav>
      <MobileNav :active-view="currentView" @change-view="changeView" />
    </template>

    <template #rightSidebar>
      <PlaybackQueue ref="playbackQueueRef" />
    </template>

    <PlaylistDetailView
      v-if="currentView === 'home' && selectedPlaylistId !== null"
      :key="selectedPlaylistId"
      :playlist-id="selectedPlaylistId"
      @navigate="navigateToPlaylist"
      @back="navigateBackFromPlaylist"
    />
    <HomeView
      v-else-if="currentView === 'home'"
      @navigate-playlist="navigateToPlaylist"
    />
    <StreamingSearch v-else-if="currentView === 'search'" @refetch="handleSearchRefetch" />
    <VkView v-else-if="currentView === 'vk'" />
    <div v-else :style="{ padding: '40px', textAlign: 'center' }">
      <h2>Раздел в разработке</h2>
      <p>Вы перешли в раздел: {{ currentView }}</p>
    </div>
  </AppLayout>
</template>
<style lang="scss" scoped></style>
