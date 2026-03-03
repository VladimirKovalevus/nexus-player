<template>
  <nav :class="$style.nav">
    <button
      v-for="item in navItems"
      :key="item.view"
      :class="[$style.btn, { [$style.active]: activeView === item.view }]"
      style="padding:8px"
      @click="$emit('changeView', item.view)"
      :title="item.label"
    >
      <span :class="$style.icon">{{ item.icon }}</span>
    </button>

    <button :class="$style.btn" title="Настройки" @click="isSettingsOpen = true">
      <span :class="$style.icon">⚙️</span>
    </button>

    <SettingsModal :is-open="isSettingsOpen" @close="isSettingsOpen = false" />
  </nav>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import SettingsModal from '@/components/SettingsModal.vue';

defineProps<{
  activeView: string;
}>();

defineEmits<{
  changeView: [view: string];
}>();

const isSettingsOpen = ref(false);

const navItems = [
  { view: 'home', icon: '🏠', label: 'Главная' },
  { view: 'search', icon: '🔍', label: 'Поиск' },
  { view: 'library', icon: '🎵', label: 'Музыка' },
  { view: 'playlists', icon: '📋', label: 'Плейлисты' },
  { view: 'vk', icon: 'VK', label: 'VK Музыка' },
  { view: 'history', icon: '🕒', label: 'История' },
];
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.nav {
  display: flex;
  align-items: center;
  justify-content: space-around;
  width: 100%;
  height: 100%;
}

.btn {
  flex: 1;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: color $transition-fast, background-color $transition-fast;

  &:hover {
    color: var(--color-text-primary);
    background-color: rgba(255, 255, 255, 0.05);
  }

  &.active {
    color: var(--color-accent-primary);
  }
}

.icon {
  font-size: 1.3rem;
  line-height: 1;
}
</style>
