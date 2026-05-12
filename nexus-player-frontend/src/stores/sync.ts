import { ref } from 'vue';
import { invoke } from '@/utils/providerAuth';
import type { SyncType } from '../types/types';

export type SyncStatus = 'idle' | 'running' | 'done' | 'error';

export function useSyncStore() {
  const status = ref<SyncStatus>('idle');
  const lastPath = ref<string | null>(null);
  const error = ref<string | null>(null);

  function wrap<T>(fn: () => Promise<T>): Promise<T | null> {
    status.value = 'running';
    error.value = null;
    return fn().then(
      (v) => { status.value = 'done'; return v; },
      (e) => { error.value = String(e); status.value = 'error'; return null; },
    );
  }

  async function exportSync(syncType: SyncType, deviceName?: string): Promise<string | null> {
    return wrap(async () => {
      const path = await invoke<string>('export_sync', {
        syncType,
        deviceName: deviceName ?? null,
      });
      lastPath.value = path;
      return path;
    });
  }

  async function importSync(path: string): Promise<boolean> {
    const result = await wrap(() => invoke<void>('import_sync', { path }));
    return result !== null;
  }

  async function importWithPicker(): Promise<boolean> {
    const path = await invoke<string | null>('select_sync_file');
    if (!path) return false;
    return importSync(path);
  }

  // ── Telegram transport ──────────────────────────────────────────────────────

  async function saveTelegramConfig(botToken: string, chatId: string): Promise<boolean> {
    const result = await wrap(() =>
      invoke<void>('telegram_save_config', { botToken, chatId }),
    );
    return result !== null;
  }

  async function checkTelegramConfig(): Promise<boolean> {
    const result = await wrap(() => invoke<boolean>('telegram_check_config'));
    return result === true;
  }

  async function telegramPush(syncType: SyncType, deviceName?: string): Promise<boolean> {
    const result = await wrap(() =>
      invoke<void>('telegram_push', { syncType, deviceName: deviceName ?? null }),
    );
    return result !== null;
  }

  async function telegramPull(): Promise<boolean> {
    const result = await wrap(() => invoke<void>('telegram_pull'));
    return result !== null;
  }

  return {
    status,
    lastPath,
    error,
    exportSync,
    importSync,
    importWithPicker,
    saveTelegramConfig,
    checkTelegramConfig,
    telegramPush,
    telegramPull,
  };
}
