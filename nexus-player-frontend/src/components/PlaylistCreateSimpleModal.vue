<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';

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
    <div v-if="isOpen" class="modal-overlay" @click="handleClose">
      <div class="modal-content" @click.stop>
        <div class="modal-header">
          <h2>Сформировать плейлист</h2>
          <button class="close-btn" @click="handleClose">✕</button>
        </div>

        <div class="modal-body">
          <div class="form-group">
            <label for="playlist-title">Название плейлиста *</label>
            <input
              id="playlist-title"
              v-model="title"
              type="text"
              placeholder="Новый плейлист"
              class="form-input"
              :disabled="isLoading"
            />
          </div>

          <div class="form-group">
            <label for="playlist-description">Описание</label>
            <textarea
              id="playlist-description"
              v-model="description"
              placeholder="О чем этот плейлист?"
              class="form-textarea"
              :disabled="isLoading"
              rows="2"
            />
          </div>

          <div class="form-group">
            <label for="playlist-artwork">URL обложки</label>
            <div class="input-group">
              <input
                id="playlist-artwork"
                v-model="artworkUrl"
                type="text"
                placeholder="https://example.com/image.jpg"
                class="form-input"
                :disabled="isLoading"
              />
              <button 
                type="button" 
                class="btn btn-secondary btn-small" 
                @click="handleSelectArtwork"
                :disabled="isLoading"
              >
                Выбрать
              </button>
            </div>
          </div>
        </div>

        <div class="modal-footer">
          <button class="btn btn-secondary" @click="handleClose" :disabled="isLoading">
            Отмена
          </button>
          <button class="btn btn-primary" @click="handleSave" :disabled="isLoading">
            {{ isLoading ? 'Сохранение...' : 'Сохранить' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped lang="scss">
@import '@/styles/variables.scss';

.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  backdrop-filter: blur(4px);
}

.modal-content {
  background-color: var(--color-bg-primary);
  border-radius: 12px;
  width: 90%;
  max-width: 500px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  overflow: hidden;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: $spacing-lg;
  border-bottom: 1px solid var(--color-border);
}

.modal-header h2 {
  margin: 0;
  font-size: $font-size-lg;
  color: var(--color-text-primary);
}

.close-btn {
  background: none;
  border: none;
  font-size: 1.25rem;
  cursor: pointer;
  color: var(--color-text-secondary);
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  transition: all 0.2s;
}

.close-btn:hover {
  background-color: var(--color-bg-hover);
  color: var(--color-text-primary);
}

.modal-body {
  padding: $spacing-lg;
  max-height: 70vh;
  overflow-y: auto;
}

.form-group {
  margin-bottom: $spacing-md;

  label {
    display: block;
    margin-bottom: $spacing-xs;
    font-weight: 600;
    color: var(--color-text-primary);
    font-size: $font-size-sm;
  }
}

.form-input,
.form-textarea {
  width: 100%;
  padding: $spacing-sm;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  font-size: $font-size-base;
  background-color: var(--color-bg-secondary);
  color: var(--color-text-primary);
  transition: all 0.2s;
  font-family: inherit;
}

.form-textarea {
  resize: vertical;
  min-height: 60px;
}

.input-group {
  display: flex;
  gap: $spacing-xs;

  .form-input {
    flex: 1;
  }
}

.form-input:focus,
.form-textarea:focus {
  outline: none;
  border-color: var(--color-accent-primary);
  box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
}

.form-input:disabled,
.form-textarea:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: $spacing-sm;
  padding: $spacing-lg;
  border-top: 1px solid var(--color-border);
}

.btn {
  padding: $spacing-sm $spacing-lg;
  border-radius: 8px;
  font-weight: 600;
  font-size: $font-size-sm;
  cursor: pointer;
  transition: all 0.2s;
  border: none;
}

.btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-secondary {
  background-color: var(--color-bg-secondary);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border);
}

.btn-secondary:hover:not(:disabled) {
  background-color: var(--color-bg-hover);
}

.btn-small {
  padding: $spacing-xs $spacing-md;
  font-size: $font-size-xs;
  white-space: nowrap;
}

.btn-primary {
  background: var(--color-accent-primary);
  color: white;
}

.btn-primary:hover:not(:disabled) {
  transform: translateY(-1px);
  filter: brightness(1.1);
}
</style>
