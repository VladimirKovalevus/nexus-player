<script setup lang="ts">
import { ref, watch } from 'vue';
import { convertFileSrc } from '@tauri-apps/api/core';
import { invoke } from '@/utils/providerAuth';
import AppModal from '@/components/common/AppModal.vue';

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
  <AppModal :open="isOpen" :title="$t('playlistForm.createFromQueue')" @close="handleClose">
    <div class="deck-field">
      <label class="section-label" for="playlist-title">{{ $t('playlistForm.title') }}</label>
      <input
        id="playlist-title"
        v-model="title"
        class="deck-input"
        type="text"
        :placeholder="$t('playlistForm.titlePlaceholder')"
        :disabled="isLoading"
        @keydown.enter="handleSave"
      />
    </div>

    <div class="deck-field">
      <label class="section-label" for="playlist-description">{{ $t('playlistForm.description') }}</label>
      <textarea
        id="playlist-description"
        v-model="description"
        class="deck-input"
        :placeholder="$t('playlistForm.descriptionPlaceholder')"
        :disabled="isLoading"
        rows="3"
      />
    </div>

    <div class="deck-field">
      <label class="section-label" for="playlist-artwork">{{ $t('playlistForm.artwork') }}</label>
      <div :class="$style.inputGroup">
        <input
          id="playlist-artwork"
          v-model="artworkUrl"
          class="deck-input"
          type="text"
          placeholder="https://"
          :disabled="isLoading"
        />
        <button type="button" class="deck-btn" :class="$style.pick" :disabled="isLoading" @click="handleSelectArtwork">
          {{ $t('playlistForm.pickFile') }}
        </button>
      </div>
    </div>

    <template #footer>
      <button class="deck-btn" :disabled="isLoading" @click="handleClose">{{ $t('common.cancel') }}</button>
      <button class="deck-btn deck-btn--primary" :disabled="isLoading || !title.trim()" @click="handleSave">
        {{ isLoading ? $t('playlistForm.saving') : $t('common.save') }}
      </button>
    </template>
  </AppModal>
</template>

<style module lang="scss">
.inputGroup {
  display: flex;
  gap: 6px;
}

.pick {
  flex: none;
  height: auto;
}
</style>
