<template>
  <div :class="$style.historyView" class="history-scroll-container">
    <div>
      <header class="view-header">
        <h1 class="view-title">{{ $t('nav.history') }}</h1>
        <span class="view-spacer" />
        <button class="deck-btn" :disabled="loading" @click="() => fetchHistory(page)">
          {{ loading ? $t('common.loading') : $t('common.refresh') }}
        </button>
        <button class="deck-btn deck-btn--danger" :disabled="loading || historyEntries.length === 0" @click="handleClearHistory">
          {{ $t('history.clear') }}
        </button>
      </header>

      <div v-if="error" :class="$style.error">{{ error }}</div>

      <VirtualList
        v-if="historyEntries.length > 0"
        :items="historyEntries"
        :item-height="rowHeights.table.value"
        container-selector=".history-scroll-container"
      >
        <template #default="{ item: entry }">
          <div :class="$style.entry" @click="handleEntryClick(entry)">
            <AppImage :src="entry.artworkUrl" :class="$style.artwork" :alt="entry.title">
              <template #placeholder>
                <span :class="$style.artworkPlaceholder"><AppIcon name="music-note" :size="14" /></span>
              </template>
            </AppImage>
            <div :class="$style.info">
              <div :class="$style.trackTitle" :title="entry.title">{{ entry.title }}</div>
              <div :class="$style.trackArtist">{{ entry.artist || $t('track.unknownArtist') }}</div>
            </div>
            <span :class="$style.source"><span class="source-tag">{{ historySourceText(entry) }}</span></span>
            <span :class="$style.playedAt">{{ formatTime(entry.playedAt) }}</span>
          </div>
        </template>
      </VirtualList>

      <div v-if="!loading && historyEntries.length === 0" :class="$style.empty">
        {{ $t('history.empty') }}
      </div>

      <div v-if="historyEntries.length > 0 || page > 0" :class="$style.pagination">
        <button class="deck-btn" :disabled="loading || page === 0" @click="goToPage(page - 1)">
          {{ $t('common.back') }}
        </button>
        <span class="view-count">{{ $t('history.page', { page: page + 1 }) }}</span>
        <button class="deck-btn" :disabled="loading || historyEntries.length < pageSize" @click="goToPage(page + 1)">
          {{ $t('common.forward') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, inject } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@/utils/providerAuth';
import VirtualList from '@/components/common/VirtualList.vue';
import AppIcon from '@/components/common/AppIcon.vue';
import AppImage from '@/components/common/AppImage.vue';
import type { PlayHistoryEntryDto } from '@/types/types';
import { useNotificationStore } from '@/stores/notifications';
import { sourceLabel } from '@/utils/sourceLabel';
import { useRowHeights } from '@/composables/useRowHeights';

const { t, locale } = useI18n();
const historySourceText = (entry: PlayHistoryEntryDto) => sourceLabel(entry.source.system, t);
const rowHeights = useRowHeights();
const refreshQueue = inject<() => void>('refreshQueue', () => {});
const notifications = useNotificationStore();

const historyEntries = ref<PlayHistoryEntryDto[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const page = ref(0);
const pageSize = 50;

const fetchHistory = async (targetPage: number) => {
  loading.value = true;
  error.value = null;
  try {
    const result = await invoke<PlayHistoryEntryDto[]>('get_play_history', {
      limit: pageSize,
      offset: targetPage * pageSize,
    });
    historyEntries.value = result;
    page.value = targetPage;
  } catch (e) {
    console.error('Failed to fetch history:', e);
    error.value = t('common.error', { error: String(e) });
  } finally {
    loading.value = false;
  }
};

const goToPage = (targetPage: number) => {
  if (targetPage < 0) return;
  fetchHistory(targetPage);
};

const handleClearHistory = async () => {
  loading.value = true;
  error.value = null;
  try {
    await invoke('clear_play_history');
    historyEntries.value = [];
    page.value = 0;
    notifications.addNotification(t('history.cleared'), 'success');
  } catch (e) {
    console.error('Failed to clear history:', e);
    error.value = t('common.error', { error: String(e) });
    notifications.addNotification(t('history.clearFailed'), 'error');
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
    notifications.addNotification(t('history.added', { title: entry.title }), 'success');
  } catch (e) {
    console.error('Failed to add to queue:', e);
    notifications.addNotification(t('history.addFailed'), 'error');
  }
};

const formatTime = (timestamp: number) => {
  const date = new Date(timestamp * 1000);
  return date.toLocaleString(locale.value);
};

onMounted(() => fetchHistory(0));
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.historyView {
  height: 100%;
  overflow-y: auto;
}

.error {
  padding: 10px 12px;
  color: var(--danger);
}

.entry {
  display: grid;
  grid-template-columns: 30px minmax(0, 1fr) 96px 150px;
  align-items: center;
  gap: 8px;
  height: var(--row-height);
  padding: 0 12px;
  border-bottom: 1px solid var(--line-soft);
  cursor: pointer;

  &:hover {
    background: var(--bg-selected);
  }
}

.artwork,
.artworkPlaceholder {
  display: block;
  width: 30px;
  height: 30px;
  object-fit: cover;
}

.artworkPlaceholder {
  @include flex-center;
  background: var(--key);
  color: var(--muted);
}

.info {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}

.trackTitle {
  flex: 1 1 60%;
  font-weight: 500;
  @include text-ellipsis;
}

.trackArtist {
  flex: 1 1 40%;
  font-size: var(--font-size-sm);
  color: var(--muted);
  @include text-ellipsis;
}

.playedAt {
  font: 500 var(--font-size-xs) $font-family-mono;
  color: var(--muted);
  text-align: right;
  white-space: nowrap;
}

.empty {
  padding: 36px 12px;
  text-align: center;
  color: var(--muted);
}

.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 10px 12px;
}

@media (max-width: 800px) {
  .entry {
    grid-template-columns: 30px minmax(0, 1fr) 130px;
  }

  .source {
    display: none;
  }
}

@media (max-width: $mobile-breakpoint) {
  .entry {
    grid-template-columns: 40px minmax(0, 1fr) auto;
  }

  .artwork,
  .artworkPlaceholder {
    width: 40px;
    height: 40px;
  }

  .info {
    display: block;
    line-height: 1.2;
  }

  .trackArtist {
    font-size: var(--font-size-xs);
  }

  .playedAt {
    white-space: normal;
    max-width: 84px;
  }
}
</style>
