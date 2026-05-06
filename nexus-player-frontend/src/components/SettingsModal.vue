<template>
  <Teleport to="body">
  <div v-if="isOpen" :class="$style.overlay" @click="handleOverlayClick">
    <div :class="$style.modal" @click.stop>
      <div :class="$style.header">
        <h2 :class="$style.title">Настройки</h2>
        <button :class="$style.closeButton" @click="close"><AppIcon name="close" :size="18" /></button>
      </div>

      <div :class="$style.content">
        <div v-if="loading" :class="$style.loading">Загрузка...</div>
        <div v-else-if="error" :class="$style.error">{{ error }}</div>
        <div v-else :class="$style.form">
          <!-- Network Settings -->
          <section :class="$style.section">
            <h3 :class="$style.sectionTitle">Сеть и Буферизация</h3>
            <div :class="$style.field">
              <label>Timeout (сек)</label>
              <input type="number" v-model.number="config.network.timeoutSeconds" />
            </div>
            <div :class="$style.field">
              <label>Chunk Size (байт)</label>
              <input type="number" v-model.number="config.network.chunkSize" />
            </div>
            <div :class="$style.field">
              <label>Buffer Size (байт)</label>
              <input type="number" v-model.number="config.network.bufferSize" />
            </div>
          </section>

          <!-- Audio Settings -->
          <section :class="$style.section">
            <h3 :class="$style.sectionTitle">Аудио</h3>
            <div :class="$style.field">
              <label :class="$style.checkboxLabel">
                <input type="checkbox" v-model="config.audio.waveformEnabled" />
                Включить анализ формы волны (Waveform)
              </label>
              <p :class="$style.hint">Анализирует трек во время загрузки для отображения столбиков звука.</p>
            </div>
            <div :class="$style.field">
              <label>Лимит памяти для кэша (байт)</label>
              <input type="number" v-model.number="config.audio.memLimit" />
              <p :class="$style.hint">Например, 104857600 для 100MB.</p>
            </div>

            <div :class="$style.field">
              <label :class="$style.checkboxLabel">
                <input type="checkbox" v-model="config.audio.fadeInEnabled" />
                Fade-in при смене треков
              </label>
              <div v-if="config.audio.fadeInEnabled" :class="$style.fadeRow">
                <input
                  type="range"
                  min="100"
                  max="3000"
                  step="100"
                  v-model.number="config.audio.fadeInMs"
                  :class="$style.fadeSlider"
                />
                <span :class="$style.fadeValue">{{ config.audio.fadeInMs }} мс</span>
              </div>
            </div>

            <div :class="$style.field">
              <label :class="$style.checkboxLabel">
                <input type="checkbox" v-model="config.audio.fadeOutEnabled" />
                Fade-out в конце треков
              </label>
              <div v-if="config.audio.fadeOutEnabled" :class="$style.fadeRow">
                <input
                  type="range"
                  min="100"
                  max="3000"
                  step="100"
                  v-model.number="config.audio.fadeOutMs"
                  :class="$style.fadeSlider"
                />
                <span :class="$style.fadeValue">{{ config.audio.fadeOutMs }} мс</span>
              </div>
            </div>
          </section>

          <!-- Appearance Settings -->
          <section :class="$style.section">
            <h3 :class="$style.sectionTitle">Внешний вид</h3>
            <div :class="$style.field">
              <label>Тема оформления</label>
              <div :class="$style.themeGrid">
                <button 
                  v-for="t in ['dark', 'retro', 'cyberpunk']" 
                  :key="t"
                  :class="[$style.themeButton, themeStore.activeTheme === t && $style.active]"
                  @click="themeStore.setTheme(t)"
                >
                  {{ formatThemeDisplayName(t) }}
                </button>
              </div>
            </div>
            
            <div v-if="false" :class="$style.field">
              <label>Пользовательский CSS</label>
              <textarea 
                v-model="themeStore.customCss" 
                @input="themeStore.setCustomCss(($event.target as HTMLTextAreaElement).value)"
                placeholder="/* Вставьте ваш CSS сюда */"
                rows="5"
                :class="$style.customCssInput"
              ></textarea>
            </div>
          </section>

          <!-- Sync Settings -->
          <section :class="$style.section">
            <h3 :class="$style.sectionTitle">Синхронизация</h3>

            <div :class="$style.provider">
              <div :class="$style.field">
                <label>Bot Token</label>
                <input
                  type="password"
                  v-model="telegramToken"
                  placeholder="123456:ABC-DEF..."
                  autocomplete="off"
                />
                <p :class="$style.hint">Создайте бота через @BotFather и вставьте токен.</p>
              </div>
              <div :class="$style.field">
                <label>Chat ID</label>
                <div :class="$style.inputGroup">
                  <input
                    type="text"
                    v-model="telegramChatId"
                    :placeholder="tgSavedChatId || '-100XXXXXXXXXX'"
                  />
                  <button
                    :class="$style.smallButton"
                    @click="handleTelegramSave"
                    :disabled="tgLoading || !telegramToken || !telegramChatId"
                  >
                    {{ tgLoading ? '...' : 'Сохранить' }}
                  </button>
                </div>
                <p :class="$style.hint">Создайте приватный канал, добавьте бота как admin, скопируйте ID канала.</p>
              </div>

              <div v-if="tgStatus !== 'idle'" :class="[$style.syncBadge, $style[`badge_${tgStatus}`]]">
                <span v-if="tgStatus === 'checking'">Проверка...</span>
                <span v-else-if="tgStatus === 'ok'"><AppIcon name="check" :size="14" /> Подключено · {{ tgSavedChatId }}</span>
                <span v-else-if="tgStatus === 'error'"><AppIcon name="x-circle" :size="14" /> {{ tgError }}</span>
                <span v-else-if="tgStatus === 'saved'">Сохранено</span>
              </div>
            </div>

            <div v-if="tgSavedChatId" :class="$style.syncActions">
              <div :class="$style.syncRow">
                <div :class="$style.syncInfo">
                  <span :class="$style.syncLabel">Full Sync</span>
                  <span :class="$style.syncHint">Заменяет всё на принимающей стороне</span>
                </div>
                <div :class="$style.syncButtons">
                  <button
                    :class="[$style.smallButton, $style.syncBtn]"
                    @click="handleTelegramPush('full')"
                    :disabled="!!tgSyncLoading"
                  >{{ tgSyncLoading === 'push' ? '...' : 'Отправить' }}</button>
                  <button
                    :class="[$style.smallButton, $style.syncBtn]"
                    @click="handleTelegramPull"
                    :disabled="!!tgSyncLoading"
                  >{{ tgSyncLoading === 'pull' ? '...' : 'Получить' }}</button>
                </div>
              </div>
              <div :class="$style.syncRow">
                <div :class="$style.syncInfo">
                  <span :class="$style.syncLabel">Diff Sync</span>
                  <span :class="$style.syncHint">Только добавляет новое</span>
                </div>
                <div :class="$style.syncButtons">
                  <button
                    :class="[$style.smallButton, $style.syncBtn]"
                    @click="handleTelegramPush('diff')"
                    :disabled="!!tgSyncLoading"
                  >{{ tgSyncLoading === 'push_diff' ? '...' : 'Отправить' }}</button>
                </div>
              </div>

              <p v-if="tgSyncError" :class="$style.syncError">{{ tgSyncError }}</p>
              <p v-if="tgSyncSuccess" :class="$style.syncSuccess">{{ tgSyncSuccess }}</p>
            </div>
          </section>

          <!-- Providers Settings -->
          <section :class="$style.section">
            <h3 :class="$style.sectionTitle">Провайдеры</h3>
            

            <div :class="$style.provider">
              <div :class="$style.providerHeader">
                <label :class="$style.checkboxLabel">
                  <input type="checkbox" v-model="config.providers.soundcloud.enabled" />
                  SoundCloud
                </label>
              </div>
            </div>

            <div :class="$style.provider">
              <div :class="$style.providerHeader">
                <label :class="$style.checkboxLabel">
                  <input type="checkbox" v-model="config.providers.bandcamp.enabled" />
                  Bandcamp
                </label>
              </div>
            </div>

          </section>
        </div>
      </div>

      <div :class="$style.footer">
        <button :class="$style.buttonSecondary" @click="close">Отмена</button>
        <button :class="$style.buttonPrimary" @click="save" :disabled="loading">Сохранить</button>
      </div>
    </div>
  </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import AppIcon from '@/components/common/AppIcon.vue';
import type { AppConfig, SyncType } from '@/types/types';
import { useThemeStore } from '@/stores/theme';

const themeStore = useThemeStore();
const props = defineProps<{
  isOpen: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'saved'): void;
}>();

const loading = ref(false);
const error = ref<string | null>(null);
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

// ── Telegram Sync ─────────────────────────────────────────────────────────────
const telegramToken = ref('');
const telegramChatId = ref('');
const tgSavedChatId = ref('');
const tgStatus = ref<'idle' | 'checking' | 'ok' | 'error' | 'saved'>('idle');
const tgError = ref('');
const tgLoading = ref(false);
const tgSyncLoading = ref<false | 'push' | 'push_diff' | 'pull'>(false);
const tgSyncError = ref('');
const tgSyncSuccess = ref('');

const loadTelegramConfig = async () => {
  try {
    const cfg = await invoke<{ chatId: string } | null>('telegram_get_config');
    if (cfg) {
      tgSavedChatId.value = cfg.chatId;
      tgStatus.value = 'idle';
    }
  } catch {
    // not configured yet — silent
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
      tgError.value = 'Бот не отвечает. Проверьте токен и Chat ID.';
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
    tgSyncSuccess.value = syncType === 'full' ? 'Full sync отправлен.' : 'Diff sync отправлен.';
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
    tgSyncSuccess.value = 'Синхронизация применена.';
  } catch (e) {
    tgSyncError.value = String(e);
  } finally {
    tgSyncLoading.value = false;
  }
};
// ─────────────────────────────────────────────────────────────────────────────

const fetchConfig = async () => {
  loading.value = true;
  try {
    const res = await invoke<AppConfig>('get_app_config');
    // Ensure nested objects exist just in case backend returns partial (though it shouldn't)
    console.log(res,"APP CONFIG")
    config.value = res;
  } catch (e) {
    error.value = `Failed to load config: ${e}`;
  } finally {
    loading.value = false;
  }
};

const save = async () => {
  loading.value = true;
  try {
    await invoke('update_app_config', { config: config.value });
    emit('saved');
    emit('close');
  } catch (e) {
    error.value = `Failed to save config: ${e}`;
  } finally {
    loading.value = false;
  }
};

const formatThemeDisplayName = (theme: string) => {
  switch (theme) {
    case 'dark': return 'Темная';
    case 'light': return 'Светлая';
    case 'retro': return 'Ретро (NES)';
    case 'cyberpunk': return 'Cyberpunk';
    default: return theme;
  }
};

const close = () => {
  emit('close');
};

const handleOverlayClick = () => {
  close();
};

watch(
  () => props.isOpen,
  (newVal) => {
    if (newVal) {
      fetchConfig();
      loadTelegramConfig();
    }
  }
);
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  backdrop-filter: blur(4px);
}

.modal {
  background-color: var(--modal-bg);
  width: 600px;
  max-width: 90%;
  max-height: 90vh;
  border-radius: $radius-lg;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--modal-border);
}

.header {
  padding: $spacing-lg;
  border-bottom: 1px solid var(--modal-header-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title {
  margin: 0;
  font-size: $font-size-xl;
  color: var(--color-accent-primary);
}

.closeButton {
  background: none;
  border: none;
  color: var(--color-text-secondary);
  font-size: 24px;
  cursor: pointer;
  transition: color $transition-fast;

  &:hover {
    color: var(--color-text-primary);
  }
}

.content {
  padding: $spacing-lg;
  overflow-y: auto;
  flex: 1;
}

.section {
  margin-bottom: $spacing-xl;
}

.sectionTitle {
  font-size: $font-size-lg;
  margin-bottom: $spacing-md;
  color: var(--modal-section-title-color);
  border-bottom: 1px solid var(--modal-header-border);
  padding-bottom: $spacing-xs;
}

.field {
  margin-bottom: $spacing-md;
  display: flex;
  flex-direction: column;
  gap: $spacing-xs;

  label {
    font-size: $font-size-sm;
    color: var(--modal-label-color);
  }

  input[type="number"],
  input[type="text"],
  input[type="password"] {
    padding: $spacing-sm;
    border-radius: $radius-sm;
    border: 1px solid var(--modal-input-border);
    background-color: var(--modal-input-bg);
    color: var(--modal-input-color, var(--color-text-primary));
    font-family: inherit;

    &:focus {
      outline: none;
      border-color: var(--color-accent-primary);
    }
  }
}

.provider {
  margin-bottom: $spacing-md;
  padding: $spacing-md;
  background-color: var(--modal-card-bg);
  border-radius: $radius-md;
}

.providerHeader {
  display: flex;
  align-items: center;
}

.checkboxLabel {
  display: flex;
  align-items: center;
  gap: $spacing-sm;
  cursor: pointer;
  font-size: $font-size-base;
  color: var(--color-text-primary);

  input[type="checkbox"] {
    width: 18px;
    height: 18px;
    accent-color: var(--color-accent-primary);
  }
}

.providerDetails {
  margin-top: $spacing-md;
  padding-left: $spacing-lg;
}

.inputGroup {
  display: flex;
  gap: $spacing-sm;
  
  input {
    flex: 1;
  }
}

.hint {
  font-size: $font-size-sm;
  font-weight: $font-weight-bold;
  margin-bottom: $spacing-xs;
  color: var(--modal-hint-color);
}

.instructions {
  font-size: $font-size-xs;
  color: var(--color-text-secondary);
  padding-left: $spacing-lg;
  margin-bottom: $spacing-md;

  li {
    margin-bottom: $spacing-xxs;
  }
}

.link {
  color: var(--color-accent-primary);
  text-decoration: underline;
  
  &:hover {
    color: var(--color-accent-hover);
  }
}

.smallButton {
  padding: $spacing-xs $spacing-sm;
  background-color: var(--color-accent-secondary);
  color: white;
  border: none;
  border-radius: $radius-sm;
  cursor: pointer;
  font-size: $font-size-sm;

  &:hover {
    background-color: lighten(#764ba2, 5%);
  }
}

.footer {
  padding: $spacing-lg;
  border-top: 1px solid var(--color-border);
  display: flex;
  justify-content: flex-end;
  gap: $spacing-md;
}

.buttonPrimary,
.buttonSecondary {
  padding: $spacing-sm $spacing-xl;
  border-radius: $radius-md;
  font-size: $font-size-base;
  font-weight: $font-weight-semibold;
  cursor: pointer;
  transition: all $transition-fast;
  border: none;
}

.buttonSecondary {
  background-color: transparent;
  color: var(--color-text-secondary);
  border: 1px solid var(--color-border);

  &:hover {
    border-color: var(--color-text-primary);
    color: var(--color-text-primary);
  }
}

.buttonPrimary {
  background: black;
  color: white;

  &:hover {
    opacity: 0.9;
    transform: translateY(-1px);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }
}

.loading, .error {
  text-align: center;
  padding: $spacing-xl;
  color: var(--color-text-secondary);
}

.error {
  color: #ef4444;
}

.themeGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: $spacing-md;
  margin-top: $spacing-xs;
}

.themeButton {
  padding: $spacing-md;
  background-color: var(--modal-card-bg);
  font-size: $font-size-xs;
  border: 2px solid var(--modal-input-border);
  border-radius: $radius-md;
  color: var(--color-text-primary);
  cursor: pointer;
  transition: all $transition-fast;
  font-weight: $font-weight-medium;

  &:hover {
    border-color: var(--color-accent-primary);
    background-color: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  &.active {
    border-color: var(--color-accent-primary);
    background-color: var(--color-bg-hover);
    box-shadow: 0 0 0 1px var(--color-accent-primary);
    color: var(--color-text-primary);
  }
}

.fadeRow {
  display: flex;
  align-items: center;
  gap: $spacing-sm;
  margin-top: $spacing-xs;
}

.fadeSlider {
  flex: 1;
  accent-color: var(--color-accent-primary);
}

.fadeValue {
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
  min-width: 56px;
  text-align: right;
}

.customCssInput {
  width: 100%;
  font-family: monospace !important;
  font-size: $font-size-xs !important;
  resize: vertical;
}

.syncBadge {
  margin-top: $spacing-sm;
  padding: $spacing-xs $spacing-sm;
  border-radius: $radius-sm;
  font-size: $font-size-sm;
  display: inline-block;

  &.badge_checking {
    background-color: var(--color-bg-hover);
    color: var(--color-text-secondary);
  }

  &.badge_ok {
    background-color: rgba(34, 197, 94, 0.12);
    color: #22c55e;
    border: 1px solid rgba(34, 197, 94, 0.3);
  }

  &.badge_error {
    background-color: rgba(239, 68, 68, 0.12);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  &.badge_saved {
    background-color: var(--color-bg-hover);
    color: var(--color-text-secondary);
  }
}

.syncActions {
  margin-top: $spacing-md;
  display: flex;
  flex-direction: column;
  gap: $spacing-sm;
}

.syncRow {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: $spacing-sm $spacing-md;
  background-color: var(--modal-card-bg);
  border-radius: $radius-md;
  gap: $spacing-md;
}

.syncInfo {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.syncLabel {
  font-size: $font-size-sm;
  font-weight: $font-weight-semibold;
  color: var(--color-text-primary);
}

.syncHint {
  font-size: $font-size-xs;
  color: var(--color-text-secondary);
}

.syncButtons {
  display: flex;
  gap: $spacing-xs;
  flex-shrink: 0;
}

.syncBtn {
  min-width: 80px;
}

.syncError {
  font-size: $font-size-sm;
  color: #ef4444;
  margin-top: $spacing-xs;
}

.syncSuccess {
  font-size: $font-size-sm;
  color: #22c55e;
  margin-top: $spacing-xs;
}
</style>
