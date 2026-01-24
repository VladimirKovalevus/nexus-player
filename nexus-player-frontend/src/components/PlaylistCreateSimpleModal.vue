<script setup lang="ts">
import { ref } from 'vue';

interface Props {
  isOpen: boolean;
  isLoading: boolean;
}

defineProps<Props>();
const emit = defineEmits<{
  close: [];
  save: [name: string];
}>();

const name = ref('');

const handleSave = () => {
  if (name.value.trim()) {
    emit('save', name.value.trim());
  }
};

const handleClose = () => {
  name.value = '';
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
            <label for="playlist-name">Название плейлиста</label>
            <input
              id="playlist-name"
              v-model="name"
              type="text"
              placeholder="Новый плейлист"
              class="form-input"
              @keyup.enter="handleSave"
              :disabled="isLoading"
            />
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
  max-width: 400px;
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
  font-size: 1.25rem;
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
}

.form-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 600;
  color: var(--color-text-primary);
  font-size: 0.875rem;
}

.form-input {
  width: 100%;
  padding: 0.75rem;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  font-size: 1rem;
  background-color: var(--color-bg-secondary);
  color: var(--color-text-primary);
  transition: all 0.2s;
}

.form-input:focus {
  outline: none;
  border-color: var(--color-accent-primary);
  box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
}

.form-input:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.75rem;
  padding: 1.5rem;
  padding-top: 0;
}

.btn {
  padding: 0.6rem 1.2rem;
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
  background: linear-gradient(135deg, var(--color-accent-primary) 0%, var(--color-accent-purple) 100%);
  color: white;
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
}

.btn-primary:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 6px 16px rgba(102, 126, 234, 0.4);
}
</style>
