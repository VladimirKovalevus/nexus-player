import { errorMessage } from '@/utils/errorMessage';
import { i18n } from '@/i18n';
import { invoke as rawInvoke, type InvokeArgs, type InvokeOptions } from '@tauri-apps/api/core';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { listen } from '@tauri-apps/api/event';
import { useNotificationStore } from '@/stores/notifications';
import { isAndroid } from '@/utils/platform';
import { PLAYLISTS_CHANGED_EVENT } from '@/utils/playlistEvents';

// Single entry point for backend calls: when a command fails because a streaming provider
// needs a challenge (SoundCloud client_id), solve it once and retry the call.

export interface ProviderChallenge {
  provider: string;
  url: string;
}

// Mirrors StreamError::ChallengeRequired message in nexus-player-backend/src/streaming/error.rs
const CHALLENGE_RE = /Challenge required for (\w+): (\S+)/;

export function parseChallenge(err: unknown): ProviderChallenge | null {
  const m = String(err instanceof Error ? err.message : err).match(CHALLENGE_RE);
  return m ? { provider: m[1], url: m[2] } : null;
}

export { PLAYLISTS_CHANGED_EVENT };

const PLAYLIST_COMMANDS = new Set([
  'create_playlist',
  'create_playlist_with_tracks',
  'create_playlist_from_container',
  'delete_playlist',
  'add_track_to_playlist',
  'remove_track_from_playlist',
  'add_playlist_to_playlist',
  'remove_playlist_from_playlist',
  'cleanup_library',
  'telegram_pull',
]);

async function invokeWithChallenge<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  try {
    return await rawInvoke<T>(cmd, args, options);
  } catch (e) {
    const challenge = parseChallenge(e);
    if (!challenge) throw e;
    await requestProviderAuth(challenge.provider, challenge.url);
    return rawInvoke<T>(cmd, args, options);
  }
}

export async function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  const result = await invokeWithChallenge<T>(cmd, args, options);
  if (PLAYLIST_COMMANDS.has(cmd)) window.dispatchEvent(new Event(PLAYLISTS_CHANGED_EVENT));
  return result;
}

// One in-flight challenge per provider; concurrent callers share it.
const pending = new Map<string, Promise<void>>();

export function requestProviderAuth(provider: string, url: string): Promise<void> {
  const existing = pending.get(provider);
  if (existing) return existing;

  const p = (async () => {
    if (provider !== 'soundcloud') {
      throw new Error(`Unsupported provider challenge: ${provider}`);
    }
    const notificationStore = useNotificationStore();
    notificationStore.addNotification(
      i18n.global.t('provider.challenge', { provider }),
      'info',
    );
    try {
      const clientId = await solveSoundCloudChallenge(url);
      console.log('[SC-Auth] Resolved client_id, updating provider auth');
      await rawInvoke('update_provider_auth', { provider, authData: clientId });
    } catch (e) {
      console.error('[SC-Auth] Failed:', e);
      notificationStore.addNotification(i18n.global.t('provider.failed', { error: errorMessage(e) }), 'error');
      throw e;
    }
  })().finally(() => pending.delete(provider));

  pending.set(provider, p);
  return p;
}

const SC_LABEL = 'soundcloud-challenge';
const SC_TIMEOUT_MS = 45000;

// Desktop: resolves via challenge-solved-global event emitted by submit_challenge_result command.
// Android: resolves via Kotlin plugin's JavascriptInterface → resolve() callback returned directly.
async function solveSoundCloudChallenge(url: string): Promise<string> {
  console.log('[SC-Auth] Starting SoundCloud challenge for:', url);
  const notificationStore = useNotificationStore();

  if (isAndroid) {
    notificationStore.addNotification(i18n.global.t('provider.opening'), 'info');
    const slowHint = setTimeout(() => {
      notificationStore.addNotification(i18n.global.t('provider.waiting'), 'info');
    }, 3000);
    try {
      const result = await rawInvoke<{ clientId: string; label: string }>(
        'plugin:soundCloudChallenge|open',
        { label: SC_LABEL, url, script: '' },
      );
      notificationStore.addNotification(i18n.global.t('provider.received'), 'success');
      return result.clientId;
    } finally {
      clearTimeout(slowHint);
    }
  }

  // Desktop: hidden window + client_id interception via initialization_script
  const closeWindow = async () => {
    try {
      const win = await WebviewWindow.getByLabel(SC_LABEL);
      if (win) await win.close();
    } catch (_) {}
  };

  let unlisten: (() => void) | null = null;
  let timeoutId: ReturnType<typeof setTimeout> | null = null;

  try {
    return await new Promise<string>(async (resolve, reject) => {
      try {
        unlisten = await listen<{ clientId: string; label: string }>('challenge-solved-global', (event) => {
          if (event.payload.label !== SC_LABEL) return;
          console.log('[SC-Auth] challenge-solved-global received');
          resolve(event.payload.clientId);
        });
        timeoutId = setTimeout(() => reject(new Error('SoundCloud challenge timed out')), SC_TIMEOUT_MS);
        await rawInvoke('open_challenge_window', { label: SC_LABEL, url, script: SC_INTERCEPT_SCRIPT });
      } catch (e) {
        reject(e);
      }
    });
  } finally {
    if (timeoutId) clearTimeout(timeoutId);
    (unlisten as (() => void) | null)?.();
    await closeWindow();
  }
}

const SC_INTERCEPT_SCRIPT = `
  (async function() {
    if (window._nexusScWatching) return;
    window._nexusScWatching = true;

    async function emitSolved(id, method) {
      if (window._nexusScSolved) return;
      window._nexusScSolved = true;
      console.log('[SC-Challenge] ID via ' + method + ': ' + id);
      try {
        await window.__TAURI__.core.invoke('submit_challenge_result', {
          clientId: id,
          label: "${SC_LABEL}"
        });
      } catch (e) {
        console.error('[SC-Challenge] invoke failed:', e);
      }
    }

    const oldOpen = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function(method, url) {
      const u = url ? url.toString() : '';
      if (u.includes('client_id=')) {
        const m = u.match(/client_id=([a-zA-Z0-9]{20,})/);
        if (m) emitSolved(m[1], 'XHR');
      }
      return oldOpen.apply(this, arguments);
    };

    const oldFetch = window.fetch;
    window.fetch = function(input, init) {
      const u = typeof input === 'string' ? input
        : (input instanceof URL ? input.href
        : (input && input.url ? input.url : ''));
      if (u && u.includes('client_id=')) {
        const m = u.match(/client_id=([a-zA-Z0-9]{20,})/);
        if (m) emitSolved(m[1], 'Fetch');
      }
      return oldFetch.apply(this, arguments);
    };

    setInterval(() => {
      if (window._nexusScSolved) return;
      if (window.__sc_config && window.__sc_config.client_id) {
        emitSolved(window.__sc_config.client_id, 'Config');
      }
    }, 1000);
  })()
`;
