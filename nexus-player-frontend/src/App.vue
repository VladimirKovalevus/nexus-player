<script setup lang="ts">
import { ref, provide } from 'vue';
import AppLayout from './components/layout/AppLayout.vue';
import PlaybackQueue from './components/PlaybackQueue.vue';
import PlayerControl from './components/PlayerControl.vue';
import TopBar from './components/layout/TopBar.vue';
import Sidebar from './components/layout/Sidebar.vue';
import HomeView from './views/HomeView.vue';
import VkView from './views/VkView.vue';
import StreamingSearch from './components/StreamingSearch.vue';

const playbackQueueRef = ref<InstanceType<typeof PlaybackQueue> | null>(null);
const playerControlRef = ref<InstanceType<typeof PlayerControl> | null>(null);
const showQueue = ref(true);
const currentView = ref('home');

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
};

provide('refreshQueue', refreshQueue);
</script>

<template>
  <AppLayout :show-queue="showQueue">
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

    <template #rightSidebar>
      <PlaybackQueue ref="playbackQueueRef" />
    </template>

    <HomeView v-if="currentView === 'home'" />
    <StreamingSearch v-else-if="currentView === 'search'" @refetch="handleSearchRefetch" />
    <VkView v-else-if="currentView === 'vk'" />
    <div v-else :style="{ padding: '40px', textAlign: 'center' }">
      <h2>Раздел в разработке</h2>
      <p>Вы перешли в раздел: {{ currentView }}</p>
    </div>
  </AppLayout>
</template>
<style lang="scss" scoped></style>
