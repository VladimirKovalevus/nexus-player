<template>
  <aside :class="$style.sidebar">
    <div :class="$style.logo">
      <div :class="$style.logoIcon"></div>
      <div :class="$style.logoText">
        <div>Nexus</div>
        <div :class="$style.logoSubtext">Audio Player</div>
      </div>
    </div>

    <nav :class="$style.nav">
        <div :class="$style.card">
      <a href="#" :class="[$style.navItem, { [$style.active]: activeView === 'home' }]" @click.prevent="$emit('changeView', 'home')">
        <span :class="$style.icon">🏠</span>
        <span>Главная</span>
      </a>
      <a href="#" :class="[$style.navItem, { [$style.active]: activeView === 'vk' }]" @click.prevent="$emit('changeView', 'vk')">
        <span :class="$style.icon">VK</span>
        <span>VK Музыка</span>
      </a>
      <a href="#" :class="[$style.navItem, { [$style.active]: activeView === 'search' }]" @click.prevent="$emit('changeView', 'search')">
        <span :class="$style.icon">🔍</span>
        <span>Поиск</span>
      </a>
      <a href="#" :class="[$style.navItem, { [$style.active]: activeView === 'library' }]" @click.prevent="$emit('changeView', 'library')">
        <span :class="$style.icon">🎵</span>
        <span>Музыка</span>
      </a>
        </div>
        <div :class="$style.card">
      <a href="#" :class="[$style.navItem, { [$style.active]: activeView === 'playlists' }]" @click.prevent="$emit('changeView', 'playlists')">
        <span :class="$style.icon">📋</span>
        <span>Плейлисты</span>
      </a>
      <a href="#" :class="[$style.navItem, { [$style.active]: activeView === 'history' }]" @click.prevent="$emit('changeView', 'history')">
        <span :class="$style.icon">🕒</span>
        <span>История</span>
      </a>
      <button :class="$style.settingsButton" @click="isSettingsOpen = true">
        <span :class="$style.icon">⚙️</span>
        <span>Настройки</span>
      </button>
        </div>
    </nav>



    <SettingsModal :is-open="isSettingsOpen" @close="isSettingsOpen = false" />
  </aside>
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
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.sidebar {
  height: 100%;
  background-color: $color-bg-sidebar;
  border-right: 1px solid $color-border;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  overflow-x: hidden;
  @include custom-scrollbar($color-border-light, $color-bg-sidebar);
}

.logo {
  padding: $spacing-lg;
  @include flex-start;
  gap: $spacing-md;
  border-bottom: 1px solid $color-border;
}

.logoIcon {
  width: 40px;
  height: 40px;
  background: black;
  border-radius: $radius-md;
}

.logoText {
  font-size: $font-size-lg;
  font-weight: $font-weight-semibold;
  color: $color-text-primary;
}

.logoSubtext {
  font-size: $font-size-xs;
  font-weight: $font-weight-normal;
  color: $color-text-secondary;
}

.nav {
  padding: $spacing-md 0;
}

.navItem {
  @include flex-start;
  gap: $spacing-md;
  padding: $spacing-md $spacing-lg;
  color: $color-text-secondary;
  text-decoration: none;
  transition: all $transition-fast;
  cursor: pointer;

  &:hover {
    background-color: rgba(255, 255, 255, 0.05);
    color: $color-text-primary;
  }

  &.active {
    color: $color-text-primary;
    background-color: #151515;
  }
}

.icon {
  font-size: $font-size-xl;
}

.library {
  padding: $spacing-md $spacing-lg;
  margin-top: auto;
}

.libraryTitle {
  font-size: $font-size-sm;
  font-weight: $font-weight-semibold;
  color: $color-text-secondary;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: $spacing-md;
}

.footer {
  padding: $spacing-md;
  border-top: 1px solid $color-border;
}

.settingsButton {
  @include flex-start;
  gap: $spacing-md;
  width: 100%;
  padding: $spacing-md;
  background: transparent;
  border: none;
  color: $color-text-secondary;
  border-radius: $radius-md;
  cursor: pointer;
  transition: all $transition-fast;
  font-size: $font-size-base;

  &:hover {
    background-color: rgba(255, 255, 255, 0.05);
    color: $color-text-primary;
  }
}
.card {
  background-color: #474747;
  margin: $spacing-md;
  border: 1px solid rgba(255, 255, 255, 0.03);
}
</style>
