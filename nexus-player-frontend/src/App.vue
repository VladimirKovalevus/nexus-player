<script setup lang="ts">
import { ref, provide } from 'vue';
import AppLayout from './components/layout/AppLayout.vue';
import PlaybackQueue from './components/PlaybackQueue.vue';
import PlayerControl from './components/PlayerControl.vue';
import TopBar from './components/layout/TopBar.vue';
import HomeView from './views/HomeView.vue';

const playbackQueueRef = ref<InstanceType<typeof PlaybackQueue> | null>(null);
const playerControlRef = ref<InstanceType<typeof PlayerControl> | null>(null);
const showQueue = ref(true);

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

provide('refreshQueue', refreshQueue);
</script>

<template>
  <AppLayout :show-queue="showQueue">
    <template #topBar>
      <TopBar @refetch="handleSearchRefetch" />
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

    <HomeView />
  </AppLayout>
</template>
<style lang="scss" scoped></style>
