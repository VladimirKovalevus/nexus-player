<template>
  <div :class="$style.settings">
    <div class="view-header">
      <h1 class="view-title">{{ $t('nav.settings') }}</h1>
      <span class="view-spacer" />
      <button class="deck-btn" @click="emit('close')">{{ $t('common.cancel') }}</button>
      <button class="deck-btn deck-btn--primary" :disabled="loading || !!loadError" @click="save">
        {{ $t('common.save') }}
      </button>
    </div>

    <p v-if="loading" :class="$style.message">{{ $t('common.loading') }}</p>
    <p v-else-if="loadError" :class="[$style.message, $style.bad]">{{ loadError }}</p>

    <div v-else :class="$style.rack">
      <section :class="$style.unit">
        <h2 :class="$style.unitTitle">{{ $t('settings.appearance.title') }}</h2>
        <div :class="$style.row">
          <span :class="$style.label">{{ $t('settings.appearance.theme') }}</span>
          <div class="seg">
            <button
              v-for="theme in THEMES"
              :key="theme"
              :aria-pressed="themeStore.activeTheme === theme"
              @click="themeStore.setTheme(theme)"
            >
              {{ $t(`settings.appearance.themes.${theme}`) }}
            </button>
          </div>
        </div>
        <div :class="$style.row">
          <span :class="$style.label">{{ $t('settings.appearance.language') }}</span>
          <div class="seg">
            <button
              v-for="value in LOCALES"
              :key="value"
              :aria-pressed="locale === value"
              @click="setLocale(value)"
            >
              {{ $t(`settings.appearance.languages.${value}`) }}
            </button>
          </div>
        </div>
      </section>

      <section :class="$style.unit">
        <h2 :class="$style.unitTitle">{{ $t('settings.audio.title') }}</h2>
        <label :class="$style.row">
          <span>
            <span :class="$style.label">{{ $t('settings.audio.waveform') }}</span>
            <small :class="$style.hint">{{ $t('settings.audio.waveformHint') }}</small>
          </span>
          <input v-model="config.audio.waveformEnabled" type="checkbox" :class="$style.check" />
        </label>
        <div :class="$style.row">
          <span>
            <label :class="$style.label" for="settings-mem-limit">{{ $t('settings.audio.memLimit') }}</label>
            <small :class="$style.hint">{{ $t('settings.audio.memLimitHint') }}</small>
          </span>
          <input id="settings-mem-limit" v-model.number="config.audio.memLimit" type="number" class="deck-input" :class="$style.number" />
        </div>
        <div :class="$style.row">
          <label :class="$style.checkLabel">
            <input v-model="config.audio.fadeInEnabled" type="checkbox" :class="$style.check" />
            <span :class="$style.label">{{ $t('settings.audio.fadeIn') }}</span>
          </label>
          <div v-if="config.audio.fadeInEnabled" :class="$style.fade">
            <input v-model.number="config.audio.fadeInMs" type="range" min="100" max="3000" step="100" :aria-label="$t('settings.audio.fadeIn')" />
            <span :class="$style.value">{{ $t('settings.audio.ms', { value: config.audio.fadeInMs }) }}</span>
          </div>
        </div>
        <div :class="$style.row">
          <label :class="$style.checkLabel">
            <input v-model="config.audio.fadeOutEnabled" type="checkbox" :class="$style.check" />
            <span :class="$style.label">{{ $t('settings.audio.fadeOut') }}</span>
          </label>
          <div v-if="config.audio.fadeOutEnabled" :class="$style.fade">
            <input v-model.number="config.audio.fadeOutMs" type="range" min="100" max="3000" step="100" :aria-label="$t('settings.audio.fadeOut')" />
            <span :class="$style.value">{{ $t('settings.audio.ms', { value: config.audio.fadeOutMs }) }}</span>
          </div>
        </div>
      </section>

      <section :class="$style.unit">
        <h2 :class="$style.unitTitle">{{ $t('settings.network.title') }}</h2>
        <div :class="$style.row">
          <label :class="$style.label" for="settings-timeout">{{ $t('settings.network.timeout') }}</label>
          <input id="settings-timeout" v-model.number="config.network.timeoutSeconds" type="number" class="deck-input" :class="$style.number" />
        </div>
        <div :class="$style.row">
          <label :class="$style.label" for="settings-chunk">{{ $t('settings.network.chunkSize') }}</label>
          <input id="settings-chunk" v-model.number="config.network.chunkSize" type="number" class="deck-input" :class="$style.number" />
        </div>
        <div :class="$style.row">
          <label :class="$style.label" for="settings-buffer">{{ $t('settings.network.bufferSize') }}</label>
          <input id="settings-buffer" v-model.number="config.network.bufferSize" type="number" class="deck-input" :class="$style.number" />
        </div>
      </section>

      <section :class="$style.unit">
        <h2 :class="$style.unitTitle">{{ $t('settings.providers.title') }}</h2>
        <label :class="$style.row">
          <span :class="$style.label">SoundCloud</span>
          <input v-model="config.providers.soundcloud.enabled" type="checkbox" :class="$style.check" />
        </label>
        <label :class="$style.row">
          <span :class="$style.label">Bandcamp</span>
          <input v-model="config.providers.bandcamp.enabled" type="checkbox" :class="$style.check" />
        </label>
      </section>

      <section :class="$style.unit">
        <h2 :class="$style.unitTitle">{{ $t('settings.sync.title') }}</h2>
        <div :class="$style.stack">
          <div class="deck-field">
            <label class="section-label" for="settings-bot-token">{{ $t('settings.sync.botToken') }}</label>
            <input
              id="settings-bot-token"
              v-model="telegramToken"
              class="deck-input"
              type="password"
              placeholder="123456:ABC-DEF..."
              autocomplete="off"
            />
            <small :class="$style.hint">{{ $t('settings.sync.botTokenHint') }}</small>
          </div>
          <div class="deck-field">
            <label class="section-label" for="settings-chat-id">{{ $t('settings.sync.chatId') }}</label>
            <div :class="$style.inputGroup">
              <input
                id="settings-chat-id"
                v-model="telegramChatId"
                class="deck-input"
                type="text"
                :placeholder="tgSavedChatId || '-100XXXXXXXXXX'"
              />
              <button
                class="deck-btn"
                :class="$style.groupButton"
                :disabled="tgLoading || !telegramToken || !telegramChatId"
                @click="handleTelegramSave"
              >
                {{ tgLoading ? '…' : $t('common.save') }}
              </button>
            </div>
            <small :class="$style.hint">{{ $t('settings.sync.chatIdHint') }}</small>
          </div>
          <p v-if="tgStatus === 'checking'" :class="$style.hint">{{ $t('settings.sync.checking') }}</p>
          <p v-else-if="tgStatus === 'ok'" :class="[$style.status, $style.good]">
            <AppIcon name="check" :size="13" /> {{ $t('settings.sync.connected', { chatId: tgSavedChatId }) }}
          </p>
          <p v-else-if="tgStatus === 'error'" :class="[$style.status, $style.bad]">
            <AppIcon name="x-circle" :size="13" /> {{ tgError }}
          </p>
        </div>

        <template v-if="tgSavedChatId">
          <div :class="$style.row">
            <span>
              <span :class="$style.label">{{ $t('settings.sync.full') }}</span>
              <small :class="$style.hint">{{ $t('settings.sync.fullHint') }}</small>
            </span>
            <span :class="$style.buttons">
              <button class="deck-btn" :disabled="!!tgSyncLoading" @click="handleTelegramPush('full')">
                {{ tgSyncLoading === 'push' ? '…' : $t('settings.sync.send') }}
              </button>
              <button class="deck-btn" :disabled="!!tgSyncLoading" @click="handleTelegramPull">
                {{ tgSyncLoading === 'pull' ? '…' : $t('settings.sync.receive') }}
              </button>
            </span>
          </div>
          <div :class="$style.row">
            <span>
              <span :class="$style.label">{{ $t('settings.sync.diff') }}</span>
              <small :class="$style.hint">{{ $t('settings.sync.diffHint') }}</small>
            </span>
            <button class="deck-btn" :disabled="!!tgSyncLoading" @click="handleTelegramPush('diff')">
              {{ tgSyncLoading === 'push_diff' ? '…' : $t('settings.sync.send') }}
            </button>
          </div>
          <p v-if="tgSyncError" :class="[$style.result, $style.bad]">{{ tgSyncError }}</p>
          <p v-if="tgSyncSuccess" :class="[$style.result, $style.good]">{{ tgSyncSuccess }}</p>
        </template>
      </section>

      <section :class="$style.unit">
        <h2 :class="$style.unitTitle">{{ $t('settings.library.title') }}</h2>
        <div :class="$style.row">
          <span>
            <span :class="$style.label">{{ $t('settings.library.cleanup') }}</span>
            <small :class="$style.hint">{{ $t('settings.library.cleanupHint') }}</small>
          </span>
          <button class="deck-btn deck-btn--danger" :disabled="cleanupLoading" @click="handleCleanup">
            {{ cleanupLoading ? '…' : $t('settings.library.cleanupAction') }}
          </button>
        </div>
        <p v-if="cleanupError" :class="[$style.result, $style.bad]">{{ cleanupError }}</p>
        <p v-if="cleanupResult" :class="[$style.result, $style.good]">{{ cleanupResult }}</p>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@/utils/providerAuth';
import AppIcon from '@/components/common/AppIcon.vue';
import { confirmAction } from '@/composables/useConfirm';
import { useNotificationStore } from '@/stores/notifications';
import { THEMES, useThemeStore } from '@/stores/theme';
import { LOCALES, setLocale } from '@/i18n';
import type { AppConfig, CleanupResult, SyncType } from '@/types/types';

const emit = defineEmits<{ close: [] }>();

const { t, locale } = useI18n();
const themeStore = useThemeStore();
const notifications = useNotificationStore();

const loading = ref(true);
const loadError = ref<string | null>(null);
const config = ref<AppConfig>({
  providers: {
    soundcloud: { enabled: true },
    bandcamp: { enabled: true },
  },
  audio: {
    waveformEnabled: true,
    memLimit: 104857600,
    fadeInEnabled: false,
    fadeOutEnabled: false,
    fadeInMs: 500,
    fadeOutMs: 500,
  },
  network: { timeoutSeconds: 30, chunkSize: 1048576, bufferSize: 10485760 },
});

const telegramToken = ref('');
const telegramChatId = ref('');
const tgSavedChatId = ref('');
const tgStatus = ref<'idle' | 'checking' | 'ok' | 'error'>('idle');
const tgError = ref('');
const tgLoading = ref(false);
const tgSyncLoading = ref<false | 'push' | 'push_diff' | 'pull'>(false);
const tgSyncError = ref('');
const tgSyncSuccess = ref('');

const loadTelegramConfig = async () => {
  try {
    const saved = await invoke<{ chatId: string } | null>('telegram_get_config');
    if (saved) tgSavedChatId.value = saved.chatId;
  } catch {
    // Not configured yet.
  }
};

const handleTelegramSave = async () => {
  if (!telegramToken.value || !telegramChatId.value) return;
  tgLoading.value = true;
  tgStatus.value = 'checking';
  tgError.value = '';
  try {
    await invoke('telegram_save_config', {
      botToken: telegramToken.value,
      chatId: telegramChatId.value,
    });
    const ok = await invoke<boolean>('telegram_check_config');
    if (ok) {
      tgSavedChatId.value = telegramChatId.value;
      tgStatus.value = 'ok';
      telegramToken.value = '';
      telegramChatId.value = '';
    } else {
      tgStatus.value = 'error';
      tgError.value = t('settings.sync.botNotResponding');
    }
  } catch (e) {
    tgStatus.value = 'error';
    tgError.value = String(e);
  } finally {
    tgLoading.value = false;
  }
};

const handleTelegramPush = async (syncType: SyncType) => {
  tgSyncLoading.value = syncType === 'full' ? 'push' : 'push_diff';
  tgSyncError.value = '';
  tgSyncSuccess.value = '';
  try {
    await invoke('telegram_push', { syncType, deviceName: null });
    tgSyncSuccess.value = t(syncType === 'full' ? 'settings.sync.fullSent' : 'settings.sync.diffSent');
  } catch (e) {
    tgSyncError.value = String(e);
  } finally {
    tgSyncLoading.value = false;
  }
};

const handleTelegramPull = async () => {
  tgSyncLoading.value = 'pull';
  tgSyncError.value = '';
  tgSyncSuccess.value = '';
  try {
    await invoke('telegram_pull');
    tgSyncSuccess.value = t('settings.sync.applied');
  } catch (e) {
    tgSyncError.value = String(e);
  } finally {
    tgSyncLoading.value = false;
  }
};

const save = async () => {
  try {
    await invoke('update_app_config', { config: config.value });
    notifications.addNotification(t('settings.saved'), 'success');
    emit('close');
  } catch (e) {
    // The form stays open, so nothing the user typed is lost.
    console.error('Failed to save config:', e);
    notifications.addNotification(t('common.error', { error: String(e) }), 'error');
  }
};

const cleanupLoading = ref(false);
const cleanupError = ref('');
const cleanupResult = ref('');

const handleCleanup = async () => {
  cleanupError.value = '';
  cleanupResult.value = '';

  const confirmed = await confirmAction({
    title: t('settings.library.cleanupConfirm'),
    message: t('settings.library.cleanupHint'),
    confirmLabel: t('common.delete'),
  });
  if (!confirmed) return;

  cleanupLoading.value = true;
  try {
    const result = await invoke<CleanupResult>('cleanup_library');
    cleanupResult.value = t('settings.library.cleanupResult', {
      tracks: result.deletedTracks,
      artworks: result.deletedArtworks,
    });
  } catch (e) {
    cleanupError.value = String(e);
  } finally {
    cleanupLoading.value = false;
  }
};

onMounted(async () => {
  loadTelegramConfig();
  try {
    config.value = await invoke<AppConfig>('get_app_config');
  } catch (e) {
    console.error('Failed to load config:', e);
    loadError.value = t('common.error', { error: String(e) });
  } finally {
    loading.value = false;
  }
});
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.settings {
  height: 100%;
  overflow-y: auto;
}

.message {
  padding: 24px 12px;
  text-align: center;
  color: var(--muted);
  overflow-wrap: anywhere;
}

.rack {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
  align-items: start;
  gap: 10px;
  padding: 10px;
}

.unit {
  min-width: 0;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
}

.unitTitle {
  padding: 9px 12px;
  border-bottom: 1px solid var(--line);
  font: 600 0.6563rem $font-family-mono;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--muted);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 6px 12px;
  min-height: 44px;
  padding: 7px 12px;
  border-bottom: 1px solid var(--line-soft);

  &:last-child {
    border-bottom: 0;
  }
}

.label {
  font-weight: 600;
}

.hint {
  display: block;
  max-width: 44ch;
  font-size: var(--font-size-xs);
  line-height: 1.25;
  color: var(--muted);
}

// Doubled to win over the full width of the shared .deck-input.
.number.number {
  flex: none;
  width: 120px;
  font-family: $font-family-mono;
}

.check {
  flex: none;
  width: 16px;
  height: 16px;
  accent-color: var(--accent);
}

.checkLabel {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.fade {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1 1 160px;

  input {
    flex: 1;
    min-width: 0;
    accent-color: var(--accent);
  }
}

.value {
  flex: none;
  min-width: 56px;
  font: 500 var(--font-size-xs) $font-family-mono;
  color: var(--muted);
  text-align: right;
}

.stack {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--line-soft);
}

.inputGroup {
  display: flex;
  gap: 6px;
}

.groupButton {
  flex: none;
  height: auto;
}

.buttons {
  display: flex;
  gap: 6px;
}

.status {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--font-size-sm);
  overflow-wrap: anywhere;
}

.result {
  padding: 8px 12px;
  font-size: var(--font-size-sm);
  overflow-wrap: anywhere;
}

.good {
  color: var(--success);
}

.bad {
  color: var(--danger);
}

@media (max-width: $mobile-breakpoint) {
  .rack {
    grid-template-columns: minmax(0, 1fr);
    padding: 8px;
  }

  .check {
    width: 20px;
    height: 20px;
  }

  .number.number {
    width: 140px;
  }
}
</style>
