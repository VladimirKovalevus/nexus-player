<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import AppIcon from '@/components/common/AppIcon.vue';

interface PlaylistInitialData {
  title?: string;
  description?: string;
  artworkUrl?: string;
}

interface Props {
  isOpen: boolean;
  isLoading: boolean;
  initialData?: PlaylistInitialData;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  close: [];
  save: [data: { title: string; description: string | null; artworkUrl: string | null }];
}>();

const title = ref('');
const description = ref('');
const artworkUrl = ref('');

const handleSelectArtwork = async () => {
  try {
    const filePath = await invoke<string | null>('select_image_file');
    if (filePath) {
      // In Tauri v2, convertFileSrc handles the protocol mapping (asset://)
      artworkUrl.value = convertFileSrc(filePath);
      console.log('Original path:', filePath);
      console.log('Converted URL:', artworkUrl.value);
    }
  } catch (e) {
    console.error('Failed to select artwork:', e);
  }
};

watch(() => props.isOpen, (isOpen) => {
  if (isOpen && props.initialData) {
    title.value = props.initialData.title || '';
    description.value = props.initialData.description || '';
    artworkUrl.value = props.initialData.artworkUrl || '';
  } else if (isOpen) {
    title.value = '';
    description.value = '';
    artworkUrl.value = '';
  }
});

const handleSave = () => {
  if (title.value.trim()) {
    emit('save', {
      title: title.value.trim(),
      description: description.value.trim() || null,
      artworkUrl: artworkUrl.value.trim() || null,
    });
  }
};

const handleClose = () => {
  emit('close');
};
</script>

<template>
  <Teleport to="body">
    <div v-if="isOpen" :class="$style.overlay" @click="handleClose">
      <div :class="$style.modal" @click.stop>
        <div :class="$style.header">
          <h2 :class="$style.title">Сформировать плейлист</h2>
          <button :class="$style.closeButton" @click="handleClose"><AppIcon name="close" :size="16" /></button>
        </div>

        <div :class="$style.content">
          <div :class="$style.field">
            <label for="playlist-title">Название плейлиста *</label>
            <input
              id="playlist-title"
              v-model="title"
              type="text"
              placeholder="Новый плейлист"
              :disabled="isLoading"
            />
          </div>

          <div :class="$style.field">
            <label for="playlist-description">Описание</label>
            <textarea
              id="playlist-description"
              v-model="description"
              placeholder="О чем этот плейлист?"
              :disabled="isLoading"
              rows="3"
            />
          </div>

          <div :class="$style.field">
            <label for="playlist-artwork">URL обложки</label>
            <div :class="$style.inputGroup">
              <input
                id="playlist-artwork"
                v-model="artworkUrl"
                type="text"
                placeholder="https://example.com/image.jpg"
                :disabled="isLoading"
              />
              <button 
                type="button" 
                :class="$style.smallButton" 
                @click="handleSelectArtwork"
                :disabled="isLoading"
              >
                Выбрать
              </button>
            </div>
          </div>
        </div>

        <div :class="$style.footer">
          <button :class="$style.buttonSecondary" @click="handleClose" :disabled="isLoading">
            Отмена
          </button>
          <button :class="$style.buttonPrimary" @click="handleSave" :disabled="isLoading">
            {{ isLoading ? 'Сохранение...' : 'Сохранить' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

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
  border: 1px solid var(--color-bg-primary);
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

.field {
  margin-bottom: $spacing-md;
  display: flex;
  flex-direction: column;
  gap: $spacing-xs;

  label {
    font-size: $font-size-sm;
    color: var(--color-bg-primary);
  }

  input[type="text"],
  textarea {
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

    &:disabled {
      opacity: 0.6;
      cursor: not-allowed;
    }
  }

  textarea {
    resize: vertical;
    min-height: 80px;
  }
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
  white-space: nowrap;

  &:hover:not(:disabled) {
    background-color: lighten(#764ba2, 5%);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
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

  &:hover:not(:disabled) {
    border-color: var(--color-text-primary);
    color: var(--color-text-primary);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}

.buttonPrimary {
  background: black;
  color: white;

  &:hover:not(:disabled) {
    opacity: 0.9;
    transform: translateY(-1px);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }
}
</style>
