<template>
  <nav :class="$style.nav">
    <button :class="$style.btn" title="Настройки" @click="isSettingsOpen = true">
      <AppIcon name="settings" :class="$style.icon" />
    </button>
    <button
      v-for="item in navItems"
      :key="item.view"
      :class="[$style.btn, { [$style.active]: activeView === item.view }]"
      @click="$emit('changeView', item.view)"
      :title="item.label"
    >
      <AppIcon :name="item.icon" :class="$style.icon" />
    </button>


    <SettingsModal :is-open="isSettingsOpen" @close="isSettingsOpen = false" />
  </nav>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import SettingsModal from '@/components/SettingsModal.vue';
import AppIcon from '@/components/common/AppIcon.vue';

defineProps<{
  activeView: string;
}>();

defineEmits<{
  changeView: [view: string];
}>();

const isSettingsOpen = ref(false);

const navItems = [
  { view: 'home', icon: 'home', label: 'Главная' },
  { view: 'search', icon: 'search', label: 'Поиск' },
  { view: 'library', icon: 'music-note', label: 'Музыка' },
  { view: 'playlists', icon: 'playlist', label: 'Плейлисты' },
  { view: 'radio', icon: 'radio', label: 'Радио' },
  { view: 'history', icon: 'history', label: 'История' },
  { view: 'cache', icon: 'album', label: 'Кэш' },
  { view: 'downloads', icon: 'download-tray', label: 'Загрузки' },
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
  min-width: 0;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: $color-text-secondary;
  cursor: pointer;
  padding: $spacing-sm;
  transition: all $transition-fast;

  &:hover {
    color: $color-text-primary;
    background-color: $color-bg-hover;
  }

  &.active {
    color: $color-accent-primary;
    background-color: $color-bg-primary;
  }
}

.icon {
  font-size: $font-size-xl;
  line-height: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
