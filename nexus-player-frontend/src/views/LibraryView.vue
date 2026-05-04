<template>
  <div :class="$style.library">
    <header :class="$style.header">
      <div>
        <h2>Локальная музыка</h2>
        <p v-if="status">{{ status }}</p>
      </div>
      <div :class="$style.actions">
        <button v-if="!isAndroid" @click="addSource">+ Добавить папку</button>
        <button :disabled="scanning" @click="scan">{{ scanning ? 'Поиск…' : 'Обновить' }}</button>
      </div>
    </header>

    <div :class="$style.searchRow">
      <label :class="$style.search">
        <span>🔍</span>
        <input v-model="query" type="search" placeholder="Поиск по названию, исполнителю или альбому" />
        <button v-if="query" title="Очистить поиск" @click="query = ''">×</button>
      </label>
      <div :class="$style.sortControls">
        <select v-model="sortField" aria-label="Сортировка локальной музыки">
          <option value="catalog">По порядку</option>
          <option value="title">Название</option>
          <option value="artist">Исполнитель</option>
        </select>
        <button
          :disabled="sortField === 'catalog'"
          :title="sortDirection === 'asc' ? 'По возрастанию' : 'По убыванию'"
          @click="sortDirection = sortDirection === 'asc' ? 'desc' : 'asc'"
        >{{ sortDirection === 'asc' ? '↑' : '↓' }}</button>
      </div>
    </div>

    <section v-if="!isAndroid && sources.length" :class="$style.sources">
      <div v-for="source in sources" :key="source.id" :class="$style.source">
        <span>{{ source.path }}</span>
        <button v-if="!source.isDefault" title="Удалить источник" @click="removeSource(source.id)">×</button>
      </div>
    </section>

    <div v-if="error" :class="$style.error">{{ error }}</div>
    <div v-else-if="!loading && visibleTracks.length === 0" :class="$style.empty">
      <span>🎵</span>
      <strong>{{ tracks.length ? 'Ничего не найдено' : 'Аудиофайлы не найдены' }}</strong>
      <p>{{ tracks.length ? 'Попробуйте изменить поисковый запрос.' : 'Нажмите «Обновить», чтобы выполнить поиск.' }}</p>
    </div>

    <VirtualList v-else :class="$style.list" :items="visibleTracks" :item-height="96" :overscan="8">
      <template #default="{ item: track }">
        <MediaCard
          type="track"
          is-local
          :track="track"
          :active="audioStore.currentTrack?.id === track.id"
          @click="play(track)"
          @play="play(track)"
          @refresh="load"
        />
      </template>
    </VirtualList>
  </div>
</template>

<script setup lang="ts">
import { computed, inject, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import MediaCard from '@/components/common/MediaCard.vue';
import VirtualList from '@/components/common/VirtualList.vue';
import { useAudioStore } from '@/stores/audio';
import type { AndroidMediaEntry, LocalScanResult, MusicSourceDto, TrackDto } from '@/types/types';

declare global {
  interface Window { _NexusLibrary?: { hasPermission(): boolean; requestPermission(): void; scanAudio(): string } }
}

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
  status.value = `Найдено: ${result.found}, добавлено: ${result.added}, обновлено: ${result.updated}, ошибок: ${result.errors}`;
}

async function scan() {
  scanning.value = true; error.value = '';
  try {
    let result: LocalScanResult;
    if (isAndroid) {
      const bridge = window._NexusLibrary;
      if (!bridge) throw new Error('MediaStore bridge недоступен');
      if (!bridge.hasPermission()) {
        bridge.requestPermission();
        status.value = 'Разрешите доступ к музыке и нажмите «Обновить» ещё раз.';
        return;
      }
      const entries = JSON.parse(bridge.scanAudio()) as AndroidMediaEntry[];
      result = await invoke('import_android_media', { entries });
    } else {
      result = await invoke('scan_local_library');
    }
    describe(result); await load();
  } catch (e) { error.value = `Не удалось обновить библиотеку: ${e}`; }
  finally { scanning.value = false; }
}

async function addSource() {
  const source = await invoke('add_music_source');
  await load();
  if (source) status.value = 'Папка добавлена. Нажмите «Обновить», чтобы найти в ней музыку.';
}

async function removeSource(id: number) {
  await invoke('remove_music_source', { sourceId: id });
  await load();
  status.value = 'Папка удалена из источников. Нажмите «Обновить», чтобы проверить каталог.';
}

async function play(track: TrackDto) {
  if (track.unavailable) { error.value = 'Файл сейчас недоступен'; return; }
  audioStore.startLoading();
  try {
    const trackIds = visibleTracks.value.filter((item) => !item.unavailable).map((item) => item.id);
    await invoke('load_and_play_local_tracks', { trackIds, selectedId: track.id });
    refreshQueue();
  }
  catch (e) { error.value = `Не удалось воспроизвести файл: ${e}`; audioStore.stopLoading(); }
}

onMounted(async () => {
  await load();
  if (tracks.value.length === 0 && !error.value) await scan();
});
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
.library { height: 100%; display: flex; flex-direction: column; overflow: hidden; padding: $spacing-xl; gap: $spacing-md; }
.header { display: flex; justify-content: space-between; align-items: flex-start; gap: $spacing-md; h2 { margin: 0; } p { color: $color-text-secondary; margin: $spacing-xs 0 0; } }
.actions { display: flex; gap: $spacing-sm; button { white-space: nowrap; } }
.searchRow { display: flex; gap: $spacing-sm; }
.search { flex: 1; display: flex; align-items: center; gap: $spacing-sm; padding: $spacing-sm $spacing-md; background: $color-bg-player; border: 1px solid $color-border; input { flex: 1; min-width: 0; border: 0; outline: 0; background: transparent; color: $color-text-primary; font: inherit; } button { border: 0; background: transparent; color: $color-text-secondary; font-size: $font-size-xl; } }
.sortControls { display: flex; gap: $spacing-xs; select, button { background: $color-bg-player; color: $color-text-primary; border: 1px solid $color-border; padding: $spacing-sm; font: inherit; } button { min-width: 42px; } }
.sources { display: flex; flex-wrap: wrap; gap: $spacing-sm; }
.source { display: flex; align-items: center; gap: $spacing-sm; padding: $spacing-xs $spacing-sm; border: 1px solid $color-border; background: $color-bg-player; max-width: 100%; span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } button { border: 0; background: transparent; color: $color-text-secondary; } }
.list { flex: 1; min-height: 0; }
.empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; color: $color-text-secondary; span { font-size: 48px; } }
.error { padding: $spacing-md; color: #ff6b6b; border: 1px solid currentColor; }
@media (max-width: $mobile-breakpoint) { .library { padding: calc(#{$spacing-md} + env(safe-area-inset-top)) $spacing-md $spacing-md; } .header { flex-direction: column; } .actions { width: 100%; button { flex: 1; } } .searchRow { flex-direction: column; } .sortControls { select { flex: 1; } } }
</style>
