import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { i18n } from '@/i18n';
import { invoke } from '@/utils/providerAuth';
import type { RadioMode } from '@/types/types';
import { useAudioStore } from './audio';
import { useNotificationStore } from './notifications';

const MODE_KEY = 'radio-mode';

export const useRadioStore = defineStore('radio', () => {
  const audioStore = useAudioStore();
  const notificationStore = useNotificationStore();

  const stored = localStorage.getItem(MODE_KEY);
  // The user's mode preference. The *active* session is read from the backend
  // (audioStore.radioMode) so it stays truthful across navigation and restarts.
  const selectedMode = ref<RadioMode>(stored === 'playlists' ? 'playlists' : 'tracks');
  const isStarting = ref(false);

  const activeMode = computed(() => audioStore.radioMode);
  const isActive = computed(() => audioStore.radioMode !== null);

  const setMode = (mode: RadioMode) => {
    selectedMode.value = mode;
    localStorage.setItem(MODE_KEY, mode);
  };

  const start = async (mode: RadioMode = selectedMode.value) => {
    setMode(mode);
    isStarting.value = true;
    audioStore.startLoading();
    try {
      await invoke('start_radio', { mode });
    } catch (e) {
      audioStore.stopLoading();
      notificationStore.addNotification(i18n.global.t('radio.failed', { error: String(e) }), 'error');
    } finally {
      isStarting.value = false;
    }
  };

  const stop = async () => {
    try {
      await invoke('stop_radio');
    } catch (e) {
      notificationStore.addNotification(i18n.global.t('radio.failed', { error: String(e) }), 'error');
    }
  };

  const toggle = async () => {
    if (isActive.value) await stop();
    else await start();
  };

  /// Picking a source while radio is playing rebuilds the queue from it right away,
  /// otherwise it just remembers the choice for the next start.
  const selectMode = async (mode: RadioMode) => {
    if (mode === selectedMode.value && isActive.value) return;
    if (isActive.value) await start(mode);
    else setMode(mode);
  };

  return {
    selectedMode,
    isStarting,
    activeMode,
    isActive,
    setMode,
    selectMode,
    start,
    stop,
    toggle,
  };
});
