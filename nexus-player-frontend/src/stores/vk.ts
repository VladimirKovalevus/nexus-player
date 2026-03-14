import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { TrackDto, PlaylistDto } from '@/types/types';

export const useVkStore = defineStore('vk', () => {
  const tracks = ref<TrackDto[]>([]);
  const playlists = ref<PlaylistDto[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const initialized = ref(false);

  async function fetchContent(force = false) {
    if (initialized.value && !force) {
      return;
    }

    loading.value = true;
    error.value = null;
    
    try {
      const result = await invoke<[TrackDto[], PlaylistDto[]]>('get_vk_profile');
      const [t, p] = result;
      tracks.value = t;
      playlists.value = p;
      initialized.value = true;
    } catch (e) {
      console.error('Failed to fetch VK profile:', e);
      error.value = `Ошибка: ${e}`;
    } finally {
      loading.value = false;
    }
  }

  function clear() {
    tracks.value = [];
    playlists.value = [];
    initialized.value = false;
  }

  return {
    tracks,
    playlists,
    loading,
    error,
    initialized,
    fetchContent,
    clear
  };
});
