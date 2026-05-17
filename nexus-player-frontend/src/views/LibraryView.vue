<template>
  <div :class="$style.library">
    <header class="view-header">
      <h1 class="view-title">{{ $t('library.title') }}</h1>
      <span class="view-count">{{ $t('home.trackCount', { count: tracks.length }) }}</span>
      <span class="view-spacer" />
      <button v-if="!isAndroid" class="deck-btn" @click="addSource">+ {{ $t('library.addFolder') }}</button>
      <button class="deck-btn" :disabled="scanning" @click="scan">
        {{ scanning ? $t('library.scanning') : $t('common.refresh') }}
      </button>
    </header>

    <p v-if="status" :class="$style.status">{{ status }}</p>

    <div :class="$style.searchRow">
      <label class="filter-box" :class="$style.search">
        <AppIcon name="search" :size="14" />
        <input v-model="query" type="search" :placeholder="$t('library.searchPlaceholder')" />
        <button v-if="query" type="button" :aria-label="$t('common.clear')" @click="query = ''">
          <AppIcon name="close" :size="13" />
        </button>
      </label>
      <select v-model="sortField" :class="$style.sort" :aria-label="$t('library.sortLabel')">
        <option value="catalog">{{ $t('library.sort.catalog') }}</option>
        <option value="title">{{ $t('library.sort.title') }}</option>
        <option value="artist">{{ $t('library.sort.artist') }}</option>
      </select>
      <button
        class="deck-btn"
        :class="$style.direction"
        :disabled="sortField === 'catalog'"
        :title="sortDirection === 'asc' ? $t('library.ascending') : $t('library.descending')"
        :aria-label="sortDirection === 'asc' ? $t('library.ascending') : $t('library.descending')"
        @click="sortDirection = sortDirection === 'asc' ? 'desc' : 'asc'"
      >
        <AppIcon :name="sortDirection === 'asc' ? 'arrow-up' : 'arrow-down'" :size="13" />
      </button>
    </div>

    <section v-if="!isAndroid && sources.length" :class="$style.sources">
      <div v-for="source in sources" :key="source.id" :class="$style.source">
        <span :title="source.path">{{ source.path }}</span>
        <button
          v-if="!source.isDefault"
          :title="$t('library.removeSource')"
          :aria-label="$t('library.removeSource')"
          @click="removeSource(source.id)"
        >
          <AppIcon name="close" :size="12" />
        </button>
      </div>
    </section>

    <div v-if="error" :class="$style.error">{{ error }}</div>
    <div v-else-if="!loading && visibleTracks.length === 0" :class="$style.empty">
      <AppIcon name="music-note" :size="36" />
      <strong>{{ tracks.length ? $t('common.nothingFound') : $t('library.notFound') }}</strong>
      <p>{{ tracks.length ? $t('library.tryAnotherQuery') : $t('library.pressRefresh') }}</p>
    </div>

    <div v-else class="track-table" :class="$style.table">
      <TrackTableHeader />
      <VirtualList :class="$style.list" :items="visibleTracks" :item-height="rowHeights.table.value" :overscan="8">
        <template #default="{ item: track, index }">
          <MediaRow
            type="track"
            is-local
            :track="track"
            :index="index"
            :active="audioStore.currentTrack?.id === track.id"
            @click="play(track)"
            @play="play(track)"
            @refresh="load"
          />
        </template>
      </VirtualList>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, inject, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@/utils/providerAuth';
import MediaRow from '@/components/common/MediaRow.vue';
import TrackTableHeader from '@/components/common/TrackTableHeader.vue';
import VirtualList from '@/components/common/VirtualList.vue';
import AppIcon from '@/components/common/AppIcon.vue';
import { useAudioStore } from '@/stores/audio';
import { useRowHeights } from '@/composables/useRowHeights';
import type { AndroidMediaEntry, LocalScanResult, MusicSourceDto, TrackDto } from '@/types/types';

declare global {
  interface Window { _NexusLibrary?: { hasPermission(): boolean; requestPermission(): void; scanAudio(): string } }
}

const { t } = useI18n();
const rowHeights = useRowHeights();
const tracks = ref<TrackDto[]>([]);
const sources = ref<MusicSourceDto[]>([]);
const loading = ref(true);
const scanning = ref(false);
const error = ref('');
const status = ref('');
const query = ref('');
const sortField = ref<'catalog' | 'title' | 'artist'>('catalog');
const sortDirection = ref<'asc' | 'desc'>('asc');
const audioStore = useAudioStore();
const refreshQueue = inject<() => void>('refreshQueue', () => {});
const isAndroid = /Android/i.test(navigator.userAgent);
const filteredTracks = computed(() => {
  const value = query.value.trim().toLocaleLowerCase();
  if (!value) return tracks.value;
  return tracks.value.filter((track) =>
    [track.title, track.artist, track.album]
      .some((field) => field?.toLocaleLowerCase().includes(value)),
  );
});
const visibleTracks = computed(() => {
  if (sortField.value === 'catalog') return filteredTracks.value;
  const field = sortField.value;
  return [...filteredTracks.value].sort((left, right) => {
    const leftValue = field === 'title' ? left.title : left.artist;
    const rightValue = field === 'title' ? right.title : right.artist;
    if (!leftValue && !rightValue) return 0;
    if (!leftValue) return 1;
    if (!rightValue) return -1;
    const comparison = leftValue.localeCompare(rightValue, undefined, {
      sensitivity: 'base',
      numeric: true,
    });
    return sortDirection.value === 'asc' ? comparison : -comparison;
  });
});

async function load() {
  try {
    tracks.value = await invoke<TrackDto[]>('get_local_library');
    if (!isAndroid) sources.value = await invoke<MusicSourceDto[]>('get_music_sources');
  } catch (e) { error.value = String(e); }
  finally { loading.value = false; }
}

function describe(result: LocalScanResult) {
  status.value = t('library.scanResult', { found: result.found, added: result.added, updated: result.updated, errors: result.errors });
}

async function scan() {
  scanning.value = true; error.value = '';
  try {
    let result: LocalScanResult;
    if (isAndroid) {
      const bridge = window._NexusLibrary;
      if (!bridge) throw new Error(t('library.bridgeMissing'));
      if (!bridge.hasPermission()) {
        bridge.requestPermission();
        status.value = t('library.grantAccess');
        return;
      }
      const entries = JSON.parse(bridge.scanAudio()) as AndroidMediaEntry[];
      result = await invoke('import_android_media', { entries });
    } else {
      result = await invoke('scan_local_library');
    }
    describe(result); await load();
  } catch (e) { error.value = t('library.refreshFailed', { error: String(e) }); }
  finally { scanning.value = false; }
}

async function addSource() {
  const source = await invoke('add_music_source');
  await load();
  if (source) status.value = t('library.folderAdded');
}

async function removeSource(id: number) {
  await invoke('remove_music_source', { sourceId: id });
  await load();
  status.value = t('library.folderRemoved');
}

async function play(track: TrackDto) {
  if (track.unavailable) { error.value = t('library.fileUnavailable'); return; }
  audioStore.startLoading();
  try {
    const trackIds = visibleTracks.value.filter((item) => !item.unavailable).map((item) => item.id);
    await invoke('load_and_play_local_tracks', { trackIds, selectedId: track.id });
    refreshQueue();
  }
  catch (e) { error.value = t('library.playFailed', { error: String(e) }); audioStore.stopLoading(); }
}

onMounted(async () => {
  await load();
  if (tracks.value.length === 0 && !error.value) await scan();
});
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.library {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.status {
  padding: 8px 12px 0;
  font-size: var(--font-size-sm);
  color: var(--muted);
}

.searchRow {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 12px 8px;
}

.search {
  flex: 1;
  min-width: 0;
  margin: 0;
}

.sort {
  height: 30px;
  padding: 0 6px;
  background: var(--key);
  color: var(--text);
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-sm);
}

.direction {
  height: 30px;
}

.sources {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  padding: 0 12px 8px;
}

.source {
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  padding: 2px 6px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  font: 500 var(--font-size-xs) $font-family-mono;
  color: var(--muted);

  span {
    @include text-ellipsis;
  }

  button {
    display: flex;
  }
}

.table {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.list {
  flex: 1;
  min-height: 0;
}

.empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 24px 12px;
  text-align: center;
  color: var(--muted);

  strong {
    color: var(--text);
  }
}

.error {
  margin: 0 12px;
  padding: 8px 10px;
  color: var(--danger);
  border: 1px solid currentColor;
  border-radius: var(--radius-sm);
}

@media (max-width: $mobile-breakpoint) {
  .searchRow {
    flex-wrap: wrap;
  }

  .search {
    flex-basis: 100%;
  }

  .sort,
  .direction {
    height: 38px;
  }

  .sort {
    flex: 1;
  }
}
</style>
