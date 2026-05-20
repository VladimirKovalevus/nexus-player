<script setup lang="ts">
import { ref, provide, onMounted } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { useI18n } from 'vue-i18n';
import { useNotificationStore } from './stores/notifications';
import AppLayout from './components/layout/AppLayout.vue';
import PlaybackQueue from './components/PlaybackQueue.vue';
import PlayerControl from './components/PlayerControl.vue';
import TabBar from './components/layout/TabBar.vue';
import PlaylistTree from './components/layout/PlaylistTree.vue';
import MobileNav from './components/layout/MobileNav.vue';
import HomeView from './views/HomeView.vue';
import PlaylistDetailView from './views/PlaylistDetailView.vue';
import HistoryView from './views/HistoryView.vue';
import CacheView from './views/CacheView.vue';
import DownloadsView from './views/DownloadsView.vue';
import LibraryView from './views/LibraryView.vue';
import RadioView from './views/RadioView.vue';
import SettingsView from './views/SettingsView.vue';
import StreamingSearch from './components/StreamingSearch.vue';
import GlobalNotifications from './components/common/GlobalNotifications.vue';
import VisualizerOverlay from './components/VisualizerOverlay.vue';
import CommandLine from './components/CommandLine.vue';
import PlaylistCreateModal from './components/PlaylistCreateModal.vue';
import ConfirmDialog from './components/common/ConfirmDialog.vue';

const playbackQueueRef = ref<InstanceType<typeof PlaybackQueue> | null>(null);
const playerControlRef = ref<InstanceType<typeof PlayerControl> | null>(null);
// Docked by default only where it leaves room for the table; see AppLayout's breakpoints.
const showQueue = ref(window.matchMedia('(min-width: 1101px)').matches);
const isCreatePlaylistOpen = ref(false);
const showVisualizer = ref(false);
const currentView = ref('home');
const viewBeforeSettings = ref('home');
const selectedPlaylistId = ref<number | null>(null);
const playlistHistory = ref<number[]>([]);

const notificationStore = useNotificationStore();
const { t } = useI18n();

onMounted(async () => {
  // Listen for download events from backend
  await listen<any>('download-started', (event) => {
    const { title, artist } = event.payload;
    notificationStore.addNotification(t('downloads.started', { artist, title }), 'info');
  });

  await listen<any>('download-finished', (event) => {
    const { success, error } = event.payload;
    if (success) {
      notificationStore.addNotification(t('downloads.finished'), 'success');
      refreshQueue(); // Refresh to show local source
    } else {
      notificationStore.addNotification(t('downloads.failed', { error }), 'error');
    }
  });
});

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
  // Settings is a detour: leaving it returns to the same view and open playlist.
  if (view === 'settings') {
    if (currentView.value !== 'settings') viewBeforeSettings.value = currentView.value;
    currentView.value = 'settings';
    return;
  }
  currentView.value = view;
  selectedPlaylistId.value = null;
  playlistHistory.value = [];
};

const navigateToPlaylist = (playlistId: number) => {
  if (selectedPlaylistId.value !== null) {
    playlistHistory.value.push(selectedPlaylistId.value);
  }
  selectedPlaylistId.value = playlistId;
};

// The tree is visible from every view, but playlists open inside home and cache.
const openPlaylistFromTree = (playlistId: number) => {
  if (currentView.value !== 'cache') currentView.value = 'home';
  playlistHistory.value = [];
  selectedPlaylistId.value = playlistId;
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
provide('changeView', changeView);
provide('toggleQueue', toggleQueue);
provide('openCreatePlaylistModal', () => playbackQueueRef.value?.openCreatePlaylistModal());
provide('clearQueue', () => playbackQueueRef.value?.clearQueue());
provide('openNewPlaylist', () => { isCreatePlaylistOpen.value = true; });
</script>

<template>
  <AppLayout :show-queue="showQueue" @close-queue="toggleQueue">
    <template #deck>
      <PlayerControl
        ref="playerControlRef"
        :queue-visible="showQueue"
        @toggle-queue="toggleQueue"
        @toggle-visualizer="showVisualizer = !showVisualizer"
      />
    </template>

    <template #tabs>
      <TabBar :active-view="currentView" @change-view="changeView" />
    </template>

    <template #tree>
      <PlaylistTree
        :selected-id="selectedPlaylistId"
        @select="openPlaylistFromTree"
        @create="isCreatePlaylistOpen = true"
      />
    </template>

    <template #mobileNav>
      <MobileNav :active-view="currentView" @change-view="changeView" />
    </template>

    <template #queue>
      <PlaybackQueue ref="playbackQueueRef" @close="toggleQueue" />
    </template>

    <PlaylistDetailView
      v-if="(currentView === 'home' || currentView === 'cache') && selectedPlaylistId !== null"
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
    <HistoryView v-else-if="currentView === 'history'" />
    <CacheView v-else-if="currentView === 'cache'" @navigate-playlist="navigateToPlaylist" />
    <DownloadsView v-else-if="currentView === 'downloads'" />
    <LibraryView v-else-if="currentView === 'library'" />
    <RadioView v-else-if="currentView === 'radio'" />
    <SettingsView v-else-if="currentView === 'settings'" @close="currentView = viewBeforeSettings" />
  </AppLayout>
  <GlobalNotifications />
  <VisualizerOverlay :is-open="showVisualizer" @close="showVisualizer = false" />
  <CommandLine />
  <PlaylistCreateModal :is-open="isCreatePlaylistOpen" @close="isCreatePlaylistOpen = false" @created="isCreatePlaylistOpen = false" />
  <ConfirmDialog />
</template>
