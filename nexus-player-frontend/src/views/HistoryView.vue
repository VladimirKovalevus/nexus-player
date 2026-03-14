<template>
  <div :class="$style.historyView" class="history-scroll-container">
    <header :class="$style.header">
      <h1 :class="$style.title">История воспроизведения</h1>
      <button :class="$style.refreshButton" @click="fetchHistory" :disabled="loading">
        {{ loading ? 'Загрузка...' : 'Обновить' }}
      </button>
    </header>

    <div v-if="error" :class="$style.error">{{ error }}</div>

    <div v-if="historyEntries.length > 0" :class="$style.list">
      <VirtualList
        :items="historyEntries"
        :item-height="80"
        container-selector=".history-scroll-container"
      >
        <template #default="{ item: entry }">
          <div
            :class="$style.entry"
            @click="handleEntryClick(entry)"
          >
            <div :class="$style.entryMain">
              <img
                v-if="entry.artworkUrl"
                :src="entry.artworkUrl"
                :class="$style.artwork"
                alt="Artwork"
                decoding="async"
                loading="lazy"
              />
              <div v-else :class="$style.artworkPlaceholder">🎵</div>

              <div :class="$style.info">
                <div :class="$style.trackTitle">{{ entry.title }}</div>
                <div :class="$style.trackArtist">{{ entry.artist || 'Неизвестный исполнитель' }}</div>
              </div>
            </div>

            <div :class="$style.meta">
              <div :class="$style.system">{{ entry.source.system }}</div>
              <div :class="$style.playedAt">{{ formatTime(entry.playedAt) }}</div>
            </div>

          </div>
        </template>
      </VirtualList>
    </div>

    <div v-if="!loading && historyEntries.length === 0" :class="$style.empty">
      История воспроизведения пуста.
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, inject } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import VirtualList from '@/components/common/VirtualList.vue';
import type { PlayHistoryEntryDto } from '@/types/types';
import { useNotificationStore } from '@/stores/notifications';

const refreshQueue = inject<() => void>('refreshQueue', () => {});
const notifications = useNotificationStore();

const historyEntries = ref<PlayHistoryEntryDto[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

const fetchHistory = async () => {
  loading.value = true;
  error.value = null;
  try {
    const result = await invoke<PlayHistoryEntryDto[]>('get_play_history', { limit: 50 });
    historyEntries.value = result;
  } catch (e) {
    console.error('Failed to fetch history:', e);
    error.value = `Ошибка: ${e}`;
  } finally {
    loading.value = false;
  }
};

const handleEntryClick = async (entry: PlayHistoryEntryDto) => {
  const searchResult = {
    type: 'track',
    id: entry.id,
    title: entry.title,
    artist: entry.artist || '',
    thumbnail: entry.artworkUrl,
    source: entry.source,
    state: { type: 'streaming' },
    liked: false,
  };

  try {
    await invoke('handle_streaming_track', { searchResult, strategy: 'cache', searchType: 'track' });
    refreshQueue();
    notifications.addNotification(`Добавлено: ${entry.title}`, 'success');
  } catch (e) {
    console.error('Failed to add to queue:', e);
    notifications.addNotification('Не удалось добавить в очередь', 'error');
  }
};

const formatTime = (timestamp: number) => {
  const date = new Date(timestamp * 1000);
  return date.toLocaleString();
};

onMounted(fetchHistory);
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.historyView {
  padding: $spacing-xl;
  height: 100%;
  overflow-y: auto;
}

.header {
  @include flex-between;
  margin-bottom: $spacing-2xl;
}

.title {
  font-size: $font-size-3xl;
  font-weight: $font-weight-bold;
  color: var(--color-text-primary);
}

.refreshButton {
  padding: $spacing-sm $spacing-lg;
  background-color: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: $radius-md;
  color: var(--color-text-primary);
  cursor: pointer;
  
  &:hover {
    background-color: var(--color-bg-hover);
  }
}

.list {
  // VirtualList manages layout internally
}

.entry {
  @include flex-between;
  padding: 0 $spacing-md 0 $spacing-md;
  background-color: var(--color-bg-primary);
  cursor: pointer;
  transition: background-color $transition-fast;

  &:hover {
    background-color: var(--color-bg-secondary);
  }
}

.entryMain {
  @include flex-start;
  gap: $spacing-md;
  flex: 1;
  min-width: 0;
}

.artwork {
  width: 48px;
  height: 48px;
  border-radius: $radius-sm;
  object-fit: cover;
}

.artworkPlaceholder {
  width: 48px;
  height: 48px;
  border-radius: $radius-sm;
  background-color: var(--color-bg-tertiary);
  @include flex-center;
  font-size: $font-size-xl;
}

.info {
  flex: 1;
  min-width: 0;
}

.trackTitle {
  font-weight: $font-weight-semibold;
  color: var(--color-text-primary);
  @include text-ellipsis;
}

.trackArtist {
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
  @include text-ellipsis;
}

.meta {
  text-align: right;
  margin-left: $spacing-md;
}

.system {
  font-size: $font-size-xs;
  color: var(--color-accent-primary);
  text-transform: uppercase;
  margin-bottom: 2px;
}

.playedAt {
  font-size: $font-size-xs;
  color: var(--color-text-secondary);
}


.error {
  color: #ef4444;
  padding: $spacing-md;
  background-color: rgba(239, 68, 68, 0.1);
  border-radius: $radius-md;
  margin-bottom: $spacing-lg;
}

.empty {
  text-align: center;
  padding: $spacing-3xl;
  color: var(--color-text-secondary);
}
</style>
