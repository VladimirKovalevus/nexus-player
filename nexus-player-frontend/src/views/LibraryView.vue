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

    <label :class="$style.search">
      <span>🔍</span>
      <input v-model="query" type="search" placeholder="Поиск по названию, исполнителю или альбому" />
      <button v-if="query" title="Очистить поиск" @click="query = ''">×</button>
    </label>

    <section v-if="!isAndroid && sources.length" :class="$style.sources">
      <div v-for="source in sources" :key="source.id" :class="$style.source">
        <span>{{ source.path }}</span>
        <button v-if="!source.isDefault" title="Удалить источник" @click="removeSource(source.id)">×</button>
      </div>
    </section>

    <div v-if="error" :class="$style.error">{{ error }}</div>
    <div v-else-if="!loading && filteredTracks.length === 0" :class="$style.empty">
      <span>🎵</span>
      <strong>{{ tracks.length ? 'Ничего не найдено' : 'Аудиофайлы не найдены' }}</strong>
      <p>{{ tracks.length ? 'Попробуйте изменить поисковый запрос.' : 'Нажмите «Обновить», чтобы выполнить поиск.' }}</p>
    </div>

    <VirtualList v-else :class="$style.list" :items="filteredTracks" :item-height="96" :overscan="8">
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
  await invoke('add_music_source');
  await load(); await scan();
}

async function removeSource(id: number) {
  await invoke('remove_music_source', { sourceId: id });
  await load(); await scan();
}

async function play(track: TrackDto) {
  if (track.unavailable) { error.value = 'Файл сейчас недоступен'; return; }
  audioStore.startLoading();
  try { await invoke('play_library_track', { trackId: track.id }); refreshQueue(); }
  catch (e) { error.value = `Не удалось воспроизвести файл: ${e}`; audioStore.stopLoading(); }
}

onMounted(async () => { await load(); await scan(); });
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
.library { height: 100%; display: flex; flex-direction: column; overflow: hidden; padding: $spacing-xl; gap: $spacing-md; }
.header { display: flex; justify-content: space-between; align-items: flex-start; gap: $spacing-md; h2 { margin: 0; } p { color: $color-text-secondary; margin: $spacing-xs 0 0; } }
.actions { display: flex; gap: $spacing-sm; button { white-space: nowrap; } }
.search { display: flex; align-items: center; gap: $spacing-sm; padding: $spacing-sm $spacing-md; background: $color-bg-player; border: 1px solid $color-border; input { flex: 1; min-width: 0; border: 0; outline: 0; background: transparent; color: $color-text-primary; font: inherit; } button { border: 0; background: transparent; color: $color-text-secondary; font-size: $font-size-xl; } }
.sources { display: flex; flex-wrap: wrap; gap: $spacing-sm; }
.source { display: flex; align-items: center; gap: $spacing-sm; padding: $spacing-xs $spacing-sm; border: 1px solid $color-border; background: $color-bg-player; max-width: 100%; span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } button { border: 0; background: transparent; color: $color-text-secondary; } }
.list { flex: 1; min-height: 0; }
.empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; color: $color-text-secondary; span { font-size: 48px; } }
.error { padding: $spacing-md; color: #ff6b6b; border: 1px solid currentColor; }
@media (max-width: $mobile-breakpoint) { .library { padding: calc(#{$spacing-md} + env(safe-area-inset-top)) $spacing-md $spacing-md; } .header { flex-direction: column; } .actions { width: 100%; button { flex: 1; } } }
</style>
