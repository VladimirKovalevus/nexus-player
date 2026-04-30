<script setup lang="ts">
import { watch } from 'vue';
import { useAudioStore } from '@/stores/audio';
import { useNotificationStore } from '@/stores/notifications';
import { invoke } from '@tauri-apps/api/core';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { listen } from '@tauri-apps/api/event';

const audioStore = useAudioStore();
const notificationStore = useNotificationStore();

const label = 'soundcloud-challenge';

// Desktop: resolves via challenge-solved-global event emitted by submit_challenge_result command.
// Android: resolves via Kotlin plugin's JavascriptInterface → resolve() callback returned directly.
async function resolveChallenge(clientId: string) {
  console.log('[SC-Handler] Resolved client_id, updating provider auth');
  await invoke('update_provider_auth', {
    provider: 'soundcloud',
    authData: clientId
  });
  audioStore.activeChallenge = null;
  setTimeout(() => {
    invoke('retry_playback').catch(e => console.error('[SC-Handler] Retry failed:', e));
  }, 1000);
}

watch(() => audioStore.activeChallenge, async (challenge) => {
  if (challenge && challenge.provider === 'soundcloud') {
    console.log('[SC-Handler] Starting SoundCloud challenge for:', challenge.url);

    try {
      const isAndroid = /Android/.test(navigator.userAgent);

      if (isAndroid) {
        notificationStore.addNotification('[SC] Android: вызываем плагин...', 'info');
        console.log('[SC-Handler] Android path, invoking Kotlin plugin');

        let timeoutHandle: ReturnType<typeof setTimeout> | null = null;

        const pluginPromise = invoke<{ clientId: string; label: string }>(
          'plugin:soundCloudChallenge|open',
          { label, url: challenge.url, script: '' }
        );

        // Показываем промежуточный статус если долго
        timeoutHandle = setTimeout(() => {
          notificationStore.addNotification('[SC] Диалог открыт, ожидаем client_id...', 'info');
        }, 3000);

        let result: { clientId: string; label: string };
        try {
          result = await pluginPromise;
        } finally {
          if (timeoutHandle) clearTimeout(timeoutHandle);
        }

        notificationStore.addNotification(`[SC] Получен client_id: ${result!.clientId.slice(0, 8)}...`, 'success');
        console.log('[SC-Handler] Plugin resolved clientId:', result!.clientId);
        await resolveChallenge(result!.clientId);
      } else {
        // Desktop: скрытое окно + перехват через initialization_script
        const unlistenSolved = await listen('challenge-solved-global', async (event: any) => {
          const { clientId, label: eventLabel } = event.payload;
          if (eventLabel !== label) return;

          console.log('[SC-Handler] challenge-solved-global received, id:', clientId);
          unlistenSolved();
          clearTimeout(timeoutId);

          try {
            const win = await WebviewWindow.getByLabel(label);
            if (win) await win.close();
          } catch (_) {}

          await resolveChallenge(clientId);
        });

        const script = `
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
                  label: "${label}"
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

        await invoke('open_challenge_window', { label, url: challenge.url, script });

        const timeoutId = setTimeout(async () => {
          if (audioStore.activeChallenge?.provider === 'soundcloud') {
            console.warn('[SC-Handler] Challenge timed out');
            audioStore.activeChallenge = null;
            unlistenSolved();
            try {
              const w = await WebviewWindow.getByLabel(label);
              if (w) await w.close();
            } catch (_) {}
          }
        }, 45000);
      }
    } catch (e) {
      console.error('[SC-Handler] Failed:', e);
      notificationStore.addNotification(`[SC] Ошибка: ${e}`, 'error');
      audioStore.activeChallenge = null;
    }
  }
});
</script>

<template>
  <div v-if="false"></div>
</template>
