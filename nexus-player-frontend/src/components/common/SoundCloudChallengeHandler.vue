<script setup lang="ts">
import { watch } from 'vue';
import { useAudioStore } from '@/stores/audio';
import { invoke } from '@tauri-apps/api/core';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { listen } from '@tauri-apps/api/event';

const audioStore = useAudioStore();

watch(() => audioStore.activeChallenge, async (challenge) => {
  if (challenge && challenge.provider === 'soundcloud') {
    console.log('[SC-Handler] Starting SoundCloud challenge resolution for:', challenge.url);
    
    try {
      const label = 'soundcloud-challenge';
      
      // Establish listener BEFORE opening the window
      const unlistenSolved = await listen('challenge-solved-global', async (event: any) => {
        console.log('[SC-Handler] Received challenge-solved-global event:', event);
        const { clientId, label: eventLabel } = event.payload;
        
        if (eventLabel === label) {
          console.log('[SC-Handler] Validated challenge solved! ID:', clientId);
          
          await invoke('update_provider_auth', { 
            provider: 'soundcloud', 
            authData: clientId 
          });
          
          audioStore.activeChallenge = null;
          
          // Close the window
          try {
            const win = await WebviewWindow.getByLabel(label);
            if (win) await win.destroy();
          } catch (e) {}
          
          unlistenSolved();

          // Retry playback via backend state (frontend currentTrack might be null due to PlaybackStopped)
          console.log('[SC-Handler] Invoking retry_playback in 1s...');
          setTimeout(() => {
             invoke('retry_playback').catch(e => console.error('[SC-Handler] Retry failed:', e));
          }, 1000);
        }
      });

      const script = `
        (async function() {
          console.log('[SC-Challenge] Initialization script started');
          
          let solved = false;
          async function emitSolved(id, method) {
            if (solved) return;
            console.log('[SC-Challenge] ID found via ' + method + ': ' + id);
            solved = true;
            try {
              await window.__TAURI__.core.invoke('submit_challenge_result', { 
                clientId: id, 
                label: "${label}" 
              });
            } catch (e) {
              console.error('[SC-Challenge] Failed to invoke Rust command:', e);
            }
          }

          const oldOpen = XMLHttpRequest.prototype.open;
          XMLHttpRequest.prototype.open = function(method, url) {
            const urlStr = url.toString();
            if (urlStr.includes('client_id=')) {
              const m = urlStr.match(/client_id=([a-zA-Z0-9]{32})/);
              if (m) emitSolved(m[1], 'XHR');
            }
            return oldOpen.apply(this, arguments);
          };

          const oldFetch = window.fetch;
          window.fetch = function(input, init) {
            const url = (typeof input === 'string') ? input : (input instanceof URL ? input.href : (input && input.url));
            if (url && url.includes('client_id=')) {
              const m = url.match(/client_id=([a-zA-Z0-9]{32})/);
              if (m) emitSolved(m[1], 'Fetch');
            }
            return oldFetch.apply(this, arguments);
          };
          
          setInterval(() => {
            if (solved) return;
            if (window.__sc_config && window.__sc_config.client_id) {
               emitSolved(window.__sc_config.client_id, 'Config');
            }
          }, 2000);
        })()
      `;

      await invoke('open_challenge_window', {
        label,
        url: challenge.url,
        script
      });

      setTimeout(async () => {
        if (audioStore.activeChallenge?.provider === 'soundcloud') {
           console.warn('[SC-Handler] SoundCloud challenge resolution timed out');
           audioStore.activeChallenge = null;
           unlistenSolved();
           try {
             const w = await WebviewWindow.getByLabel(label);
             if (w) await w.destroy();
           } catch (e) {}
        }
      }, 45000);

    } catch (e) {
      console.error('[SC-Handler] Failed to handle SoundCloud challenge:', e);
      audioStore.activeChallenge = null;
    }
  }
});
</script>

<template>
  <div v-if="false"></div>
</template>
