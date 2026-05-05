<template>
  <Teleport to="body">
  <div v-if="isOpen" :class="$style.overlay" @click="handleOverlayClick">
    <div :class="$style.modal" @click.stop>
      <div :class="$style.header">
        <h2 :class="$style.title">Настройки</h2>
        <button :class="$style.closeButton" @click="close">×</button>
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
                <span v-else-if="tgStatus === 'ok'">✓ Подключено · {{ tgSavedChatId }}</span>
                <span v-else-if="tgStatus === 'error'">✗ {{ tgError }}</span>
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

            <div :class="$style.provider">
              <div :class="$style.providerHeader">
                <label :class="$style.checkboxLabel">
                  <input type="checkbox" v-model="config.providers.vk.enabled" />
                  VK
                </label>
              </div>
              <div v-if="config.providers.vk.enabled" :class="$style.providerDetails">
                <div :class="$style.field">
                  <label>Телефон или email</label>
                  <input v-model="vkLogin" autocomplete="username" placeholder="Логин VK" />
                </div>
                <div :class="$style.field">
                  <label>Пароль</label>
                  <div :class="$style.inputGroup">
                    <input v-model="vkPassword" type="password" autocomplete="current-password" placeholder="Пароль VK" />
                    <button @click="handleVkAuth" :class="$style.smallButton" :disabled="vkAuthLoading || !vkLogin || !vkPassword">
                      {{ vkAuthLoading ? '...' : 'Войти' }}
                    </button>
                  </div>
                  <p :class="$style.hint">Пароль используется только во время входа и не сохраняется.</p>
                </div>

                <div v-if="vkAuthStatus" :class="$style.vkLoginBox">
                  <p :class="$style.hint">{{ vkAuthStatus }}</p>
                  <div v-if="vkCaptchaUrl" :class="$style.field">
                    <img :src="vkCaptchaUrl" alt="VK captcha" style="max-width: 100%; border-radius: 8px" />
                    <div :class="$style.inputGroup">
                      <input v-model="vkCaptchaAnswer" autocomplete="off" placeholder="Текст с картинки" />
                      <button @click="handleVkCaptchaAnswer" :class="$style.smallButton" :disabled="!vkCaptchaAnswer">Отправить</button>
                    </div>
                  </div>
                  <div v-if="vkNeedsVerification" :class="$style.inputGroup">
                    <input v-model="vkVerificationCode" inputmode="numeric" autocomplete="one-time-code" placeholder="Код подтверждения" />
                    <button @click="handleVkVerification" :class="$style.smallButton" :disabled="!vkVerificationCode">Отправить</button>
                  </div>
                </div>

                <details :class="$style.field">
                  <summary>Расширенные настройки</summary>
                  <label>Access token вручную</label>
                  <div :class="$style.inputGroup">
                    <input v-model="vkManualToken" type="password" autocomplete="off" placeholder="VK access token" />
                    <button @click="handleVkManualToken" :class="$style.smallButton" :disabled="!vkManualToken">Применить</button>
                  </div>
                </details>
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
import { ref, watch, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AppConfig, SyncType } from '@/types/types';
import { useVkStore } from '@/stores/vk';
import { useThemeStore } from '@/stores/theme';

const vkStore = useVkStore();
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
const vkLogin = ref('');
const vkPassword = ref('');
const vkManualToken = ref('');
const vkVerificationCode = ref('');
const vkCaptchaUrl = ref('');
const vkCaptchaAnswer = ref('');
const vkNeedsVerification = ref(false);
const vkAuthLoading = ref(false);
const vkCaptchaOpening = ref(false);
const vkAuthStatus = ref('');
let unlistenVkAuthStep: UnlistenFn | undefined;
let vkAuthListenerGeneration = 0;
const config = ref<AppConfig>({
  providers: {
    soundcloud: { enabled: true },
    bandcamp: { enabled: true },
    vk: { enabled: false, credentials: null },
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

type VkAuthStep =
  | { status: 'authorized' }
  | { status: 'captchaRequired'; url: string; interactive: boolean }
  | { status: 'verificationRequired'; method: string; phoneMask?: string | null };

const applyVkAuthStep = async (step: VkAuthStep) => {
  console.info('[VK Auth][Frontend] applying auth step', step.status);
  if (step.status === 'authorized') {
    vkAuthStatus.value = '✓ VK подключён';
    vkNeedsVerification.value = false;
    vkVerificationCode.value = '';
    vkPassword.value = '';
    vkStore.clear();
    await fetchConfig();
    return;
  }
  if (step.status === 'verificationRequired') {
    vkNeedsVerification.value = true;
    vkAuthStatus.value = `Введите код (${step.method})${step.phoneMask ? `: ${step.phoneMask}` : ''}`;
    return;
  }

  if (vkCaptchaOpening.value) return;
  console.info('[VK Auth][Frontend] opening captcha', { interactive: step.interactive, android: /Android/.test(navigator.userAgent) });
  vkCaptchaOpening.value = true;
  vkAuthStatus.value = 'Решите captcha в открывшемся окне';
  let nextStep: VkAuthStep | undefined;
  try {
    if (/Android/.test(navigator.userAgent)) {
      const solved = await invoke<{ successToken: string }>('plugin:vkLogin|openCaptcha', {
        url: step.url,
        interactive: step.interactive,
      });
      console.info('[VK Auth][Frontend] Android captcha plugin resolved');
      nextStep = await invoke<VkAuthStep>('submit_vk_captcha_result', { successToken: solved.successToken });
      console.info('[VK Auth][Frontend] captcha result accepted by Rust', nextStep.status);
    } else if (!step.interactive) {
      vkCaptchaUrl.value = step.url;
      vkAuthStatus.value = 'Введите текст с изображения';
    } else {
      await invoke('open_vk_captcha_window', { url: step.url });
      console.info('[VK Auth][Frontend] desktop captcha window opened');
    }
  } finally {
    vkCaptchaOpening.value = false;
  }
  // A repeated captcha event can arrive while the Android plugin invocation
  // is still active and is intentionally deduplicated above. Apply the command
  // result after releasing the guard so that this next challenge is not lost.
  if (nextStep?.status === 'captchaRequired') {
    await applyVkAuthStep(nextStep);
  }
};

const handleVkCaptchaAnswer = async () => {
  try {
    await invoke<VkAuthStep>('submit_vk_captcha_result', {
      successToken: vkCaptchaAnswer.value,
    });
    vkCaptchaAnswer.value = '';
    vkCaptchaUrl.value = '';
  } catch (e) {
    vkAuthStatus.value = `Ошибка: ${String(e)}`;
  }
};

const handleVkAuth = async () => {
  console.info('[VK Auth][Frontend] login requested');
  vkAuthLoading.value = true;
  vkAuthStatus.value = 'Выполняется вход...';
  try {
    const step = await invoke<VkAuthStep>('start_vk_auth', {
      login: vkLogin.value,
      password: vkPassword.value,
    });
    console.info('[VK Auth][Frontend] start_vk_auth returned', step.status);
    await applyVkAuthStep(step);
  } catch (e) {
    console.error('[VK Auth][Frontend] login failed', e);
    vkAuthStatus.value = `Ошибка: ${String(e)}`;
  } finally {
    vkAuthLoading.value = false;
  }
};

const handleVkVerification = async () => {
  console.info('[VK Auth][Frontend] verification code submitted');
  vkAuthLoading.value = true;
  try {
    const step = await invoke<VkAuthStep>('continue_vk_verification', { code: vkVerificationCode.value });
    await applyVkAuthStep(step);
  } catch (e) {
    console.error('[VK Auth][Frontend] verification failed', e);
    vkAuthStatus.value = `Ошибка: ${String(e)}`;
  } finally {
    vkAuthLoading.value = false;
  }
};

const handleVkManualToken = async () => {
  try {
    await invoke('set_vk_manual_token', { token: vkManualToken.value });
    vkManualToken.value = '';
    vkAuthStatus.value = '✓ Access token применён';
    vkStore.clear();
    await fetchConfig();
  } catch (e) {
    vkAuthStatus.value = `Ошибка: ${String(e)}`;
  }
};

const startVkAuthStepListener = async () => {
  const generation = ++vkAuthListenerGeneration;
  unlistenVkAuthStep?.();
  unlistenVkAuthStep = undefined;
  const unlisten = await listen<VkAuthStep>('vk-auth-step', async (event) => {
    console.info('[VK Auth][Frontend] vk-auth-step event received', event.payload.status);
    try {
      await applyVkAuthStep(event.payload);
    } catch (e) {
      vkAuthStatus.value = `Ошибка: ${String(e)}`;
    }
  });
  // The modal may have closed while Tauri was registering the listener.
  if (generation !== vkAuthListenerGeneration || !props.isOpen) {
    unlisten();
  } else {
    unlistenVkAuthStep = unlisten;
  }
};

watch(
  () => props.isOpen,
  (isOpen) => {
    if (isOpen) {
      void startVkAuthStepListener();
    } else {
      vkAuthListenerGeneration++;
      unlistenVkAuthStep?.();
      unlistenVkAuthStep = undefined;
    }
  },
  { immediate: true },
);

onUnmounted(() => {
  vkAuthListenerGeneration++;
  unlistenVkAuthStep?.();
  unlistenVkAuthStep = undefined;
});

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

.vkLoginBox {
  margin-top: $spacing-md;
  padding: $spacing-md;
  background-color: var(--color-bg-player);
  border: 1px dashed var(--color-accent-primary);
  border-radius: $radius-md;
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
