<template>
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
              <input type="number" v-model.number="config.network.timeout_seconds" />
            </div>
            <div :class="$style.field">
              <label>Chunk Size (байт)</label>
              <input type="number" v-model.number="config.network.chunk_size" />
            </div>
            <div :class="$style.field">
              <label>Buffer Size (байт)</label>
              <input type="number" v-model.number="config.network.buffer_size" />
            </div>
          </section>

          <!-- Providers Settings -->
          <section :class="$style.section">
            <h3 :class="$style.sectionTitle">Провайдеры</h3>
            
            <div :class="$style.provider">
              <div :class="$style.providerHeader">
                <label :class="$style.checkboxLabel">
                  <input type="checkbox" v-model="config.providers.youtube.enabled" />
                  YouTube
                </label>
              </div>
            </div>

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
                   <label>Token</label>
                   <div :class="$style.inputGroup">
                     <input type="password" v-model="config.providers.vk.credentials" placeholder="Введите токен или войдите" />
                     <button @click="handleVkAuth" :class="$style.smallButton">Войти</button>
                   </div>
                 </div>
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
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '@/types/types';

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
    youtube: { enabled: true },
    soundcloud: { enabled: true },
    bandcamp: { enabled: true },
    vk: { enabled: false, credentials: null },
  },
  audio: {},
  network: { timeout_seconds: 30, chunk_size: 1048576, buffer_size: 10485760 },
});

const fetchConfig = async () => {
  loading.value = true;
  try {
    const res = await invoke<AppConfig>('get_app_config');
    // Ensure nested objects exist just in case backend returns partial (though it shouldn't)
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

const handleVkAuth = async () => {
  try {
    const token = await invoke<string>('authenticate_vk');
    if (config.value.providers.vk) {
       config.value.providers.vk.credentials = token;
    }
  } catch (e) {
    alert(`VK Auth failed: ${e}`);
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
  background-color: var(--color-bg-secondary);
  width: 600px;
  max-width: 90%;
  max-height: 90vh;
  border-radius: $radius-lg;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--color-border);
}

.header {
  padding: $spacing-lg;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title {
  margin: 0;
  font-size: $font-size-xl;
  color: var(--color-text-primary);
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
  color: var(--color-accent-primary);
  border-bottom: 1px solid var(--color-border);
  padding-bottom: $spacing-xs;
}

.field {
  margin-bottom: $spacing-md;
  display: flex;
  flex-direction: column;
  gap: $spacing-xs;

  label {
    font-size: $font-size-sm;
    color: var(--color-text-secondary);
  }

  input[type="number"],
  input[type="text"],
  input[type="password"] {
    padding: $spacing-sm;
    border-radius: $radius-sm;
    border: 1px solid var(--color-border);
    background-color: var(--color-bg-primary);
    color: var(--color-text-primary);
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
  background-color: var(--color-bg-primary);
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
  background: linear-gradient(135deg, var(--color-accent-primary) 0%, var(--color-accent-purple) 100%);
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
</style>
