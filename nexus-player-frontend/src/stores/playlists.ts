import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@/utils/providerAuth';
import { PLAYLISTS_CHANGED_EVENT } from '@/utils/playlistEvents';
import type { PlaylistDto } from '@/types/types';

export const usePlaylistsStore = defineStore('playlists', () => {
  const playlists = ref<PlaylistDto[]>([]);

  const fetch = async () => {
    try {
      playlists.value = await invoke<PlaylistDto[]>('get_all_playlists');
    } catch (e) {
      console.error('Failed to fetch playlists:', e);
    }
  };

  window.addEventListener(PLAYLISTS_CHANGED_EVENT, fetch);

  return { playlists, fetch };
});
