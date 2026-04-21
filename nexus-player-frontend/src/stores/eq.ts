import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EqSettingsDto, EqPresetDto } from '@/types/types';

export const EQ_FREQUENCIES = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const;

export const useEqStore = defineStore('eq', () => {
  const settings = ref<EqSettingsDto>({
    enabled: true,
    gains: new Array(10).fill(0),
    presetName: 'Плоский',
  });

  const presets = ref<EqPresetDto[]>([]);
  const isLoading = ref(false);
  const isOpen = ref(false);

  async function load() {
    isLoading.value = true;
    try {
      const [s, p] = await Promise.all([
        invoke<EqSettingsDto>('get_eq_settings'),
        invoke<EqPresetDto[]>('get_eq_presets'),
      ]);
      settings.value = s;
      presets.value = p;
    } catch (e) {
      console.error('Failed to load EQ settings:', e);
    } finally {
      isLoading.value = false;
    }
  }

  async function applySettings(newSettings: EqSettingsDto) {
    settings.value = newSettings;
    try {
      await invoke('set_eq_settings', { settings: newSettings });
    } catch (e) {
      console.error('Failed to apply EQ settings:', e);
    }
  }

  async function setBandGain(bandIndex: number, gainDb: number) {
    const clamped = Math.max(-12, Math.min(12, gainDb));
    await applySettings({
      ...settings.value,
      gains: settings.value.gains.map((g, i) => (i === bandIndex ? clamped : g)),
      presetName: null,
    });
  }

  async function setEnabled(enabled: boolean) {
    await applySettings({ ...settings.value, enabled });
  }

  async function applyPreset(preset: EqPresetDto) {
    await applySettings({
      enabled: settings.value.enabled,
      gains: [...preset.gains],
      presetName: preset.name,
    });
  }

  function toggleOpen() {
    isOpen.value = !isOpen.value;
  }

  function close() {
    isOpen.value = false;
  }

  return {
    settings,
    presets,
    isLoading,
    isOpen,
    load,
    applySettings,
    setBandGain,
    setEnabled,
    applyPreset,
    toggleOpen,
    close,
  };
});
