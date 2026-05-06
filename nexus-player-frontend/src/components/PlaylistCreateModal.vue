<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import AppIcon from '@/components/common/AppIcon.vue';

import type { TrackDto } from '@/types/types';

interface Props {
  isOpen: boolean;
}

defineProps<Props>();
const emit = defineEmits<{
  close: [];
  created: [];
}>();

const title = ref('');
const description = ref('');
const allTracks = ref<TrackDto[]>([]);
const selectedTrackIds = ref<Set<number>>(new Set());
const loading = ref(false);
const error = ref<string | null>(null);

onMounted(async () => {
  await fetchTracks();
});

const fetchTracks = async () => {
  try {
    allTracks.value = await invoke<TrackDto[]>('get_all_tracks');
  } catch (e) {
    console.error('Failed to fetch tracks:', e);
    error.value = `Failed to load tracks: ${e}`;
  }
};

const toggleTrackSelection = (trackId: number) => {
  if (selectedTrackIds.value.has(trackId)) {
    selectedTrackIds.value.delete(trackId);
  } else {
    selectedTrackIds.value.add(trackId);
  }
};

const handleSave = async () => {
  if (!title.value.trim()) {
    error.value = 'Title is required';
    return;
  }

  loading.value = true;
  error.value = null;

  try {
    const tracksToSave = allTracks.value.filter(t => selectedTrackIds.value.has(t.id));

    await invoke<number>('create_playlist_with_tracks', {
      request: {
        title: title.value.trim(),
        description: description.value.trim() || null,
        tracks: tracksToSave,
      },
    });

    emit('created');
    handleClose();
  } catch (e) {
    console.error('Failed to create playlist:', e);
    error.value = `Failed to create playlist: ${e}`;
  } finally {
    loading.value = false;
  }
};

const handleClose = () => {
  title.value = '';
  description.value = '';
  selectedTrackIds.value.clear();
  error.value = null;
  emit('close');
};

const formatDuration = (seconds: number | null): string => {
  if (!seconds) return '0:00';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};
</script>

<template>
  <Teleport to="body">
    <div v-if="isOpen" class="modal-overlay" @click="handleClose">
      <div class="modal-content" @click.stop>
        <div class="modal-header">
          <h2>Создать плейлист</h2>
          <button class="close-btn" @click="handleClose"><AppIcon name="close" :size="16" /></button>
        </div>

        <div class="modal-body">
          <div v-if="error" class="error-message">{{ error }}</div>

          <div class="form-group">
            <label for="playlist-title">Название *</label>
            <input
              id="playlist-title"
              v-model="title"
              type="text"
              placeholder="Мой плейлист"
              class="form-input"
              :disabled="loading"
            />
          </div>

          <div class="form-group">
            <label for="playlist-description">Описание</label>
            <textarea
              id="playlist-description"
              v-model="description"
              placeholder="Описание плейлиста"
              class="form-textarea"
              :disabled="loading"
              rows="3"
            />
          </div>

          <div class="form-group">
            <label>Выбрать треки ({{ selectedTrackIds.size }} выбрано)</label>
            <div class="track-list">
              <div
                v-for="track in allTracks"
                :key="track.id"
                class="track-item"
                @click="toggleTrackSelection(track.id)"
              >
                <input
                  type="checkbox"
                  :checked="selectedTrackIds.has(track.id)"
                  :disabled="loading"
                  @click.stop="toggleTrackSelection(track.id)"
                />
                <div class="track-info">
                  <div class="track-title">{{ track.title }}</div>
                  <div class="track-meta">
                    {{ track.artist || 'Unknown Artist' }}
                    <span v-if="track.durationSecs">
                      • {{ formatDuration(track.durationSecs) }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="modal-footer">
          <button class="btn btn-secondary" @click="handleClose" :disabled="loading">
            Отмена
          </button>
          <button class="btn btn-primary" @click="handleSave" :disabled="loading">
            {{ loading ? 'Сохранение...' : 'Сохранить' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
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
  max-width: 600px;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem;
  border-bottom: 1px solid var(--color-border);
}

.modal-header h2 {
  margin: 0;
  font-size: 1.5rem;
  color: var(--color-text-primary);
}

.close-btn {
  background: none;
  border: none;
  font-size: 1.5rem;
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
  padding: 1.5rem;
  overflow-y: auto;
  flex: 1;
}

.error-message {
  padding: 0.75rem;
  margin-bottom: 1rem;
  background-color: rgba(231, 76, 60, 0.1);
  color: #e74c3c;
  border-radius: 6px;
  font-size: 0.875rem;
}

.form-group {
  margin-bottom: 1.5rem;
}

.form-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 600;
  color: var(--color-text-primary);
  font-size: 0.875rem;
}

.form-input,
.form-textarea {
  width: 100%;
  padding: 0.75rem;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  font-size: 1rem;
  background-color: var(--color-bg-secondary);
  color: var(--color-text-primary);
  transition: all 0.2s;
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

.form-textarea {
  resize: vertical;
  min-height: 80px;
}

.track-list {
  max-height: 300px;
  overflow-y: auto;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-bg-secondary);
}

.track-item {
  display: flex;
  align-items: center;
  padding: 0.75rem;
  gap: 0.75rem;
  cursor: pointer;
  transition: background-color 0.2s;
  border-bottom: 1px solid var(--color-border);
}

.track-item:last-child {
  border-bottom: none;
}

.track-item:hover {
  background-color: var(--color-bg-hover);
}

.track-item input[type='checkbox'] {
  cursor: pointer;
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}

.track-info {
  flex: 1;
  min-width: 0;
}

.track-title {
  font-weight: 500;
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track-meta {
  font-size: 0.875rem;
  color: var(--color-text-secondary);
  margin-top: 0.25rem;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.75rem;
  padding: 1.5rem;
  border-top: 1px solid var(--color-border);
}

.btn {
  padding: 0.75rem 1.5rem;
  border-radius: 8px;
  font-weight: 600;
  font-size: 0.875rem;
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

.btn-primary {
  background: black;
  color: white;
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
}

.btn-primary:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 6px 16px rgba(102, 126, 234, 0.4);
}

.btn-primary:active:not(:disabled) {
  transform: translateY(0);
}
</style>
