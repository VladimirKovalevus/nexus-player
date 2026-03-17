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
        <button :class="[$style.navItem, { [$style.active]: activeView === 'home' }]" @click.prevent="$emit('changeView', 'home')">
          <span :class="$style.icon">🏠</span>
          <span>Главная</span>
        </button>
        <button :class="[$style.navItem, { [$style.active]: activeView === 'vk' }]" @click.prevent="$emit('changeView', 'vk')">
          <span :class="$style.icon">VK</span>
          <span>VK Музыка</span>
        </button>
        <button :class="[$style.navItem, { [$style.active]: activeView === 'search' }]" @click.prevent="$emit('changeView', 'search')">
          <span :class="$style.icon">🔍</span>
          <span>Поиск</span>
        </button>
        <button :class="[$style.navItem, { [$style.active]: activeView === 'library' }]" @click.prevent="$emit('changeView', 'library')">
          <span :class="$style.icon">🎵</span>
          <span>Музыка</span>
        </button>
      </div>
      <div :class="$style.card">
        <button :class="[$style.navItem, { [$style.active]: activeView === 'playlists' }]" @click.prevent="$emit('changeView', 'playlists')">
          <span :class="$style.icon">📋</span>
          <span>Плейлисты</span>
        </button>
        <button :class="[$style.navItem, { [$style.active]: activeView === 'history' }]" @click.prevent="$emit('changeView', 'history')">
          <span :class="$style.icon">🕒</span>
          <span>История</span>
        </button>
        <button :class="[$style.navItem, { [$style.active]: activeView === 'downloads' }]" @click.prevent="$emit('changeView', 'downloads')">
          <span :class="$style.icon">📥</span>
          <span>Загрузки</span>
        </button>
        <button :class="$style.navItem" @click="isSettingsOpen = true">
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
  background: $color-bg-primary;
  border-radius: $radius-md;
}

.logoText {
  font-size: $font-size-xl;
  font-weight: $font-weight-semibold;
  color: $color-accent-primary;
}

.logoSubtext {
  font-size: $font-size-lg;
  font-weight: $font-weight-normal;
  color: $color-accent-hover;
}

.nav {
  padding: $spacing-md 0;
}

.navItem {
  @include flex-start;
  gap: $spacing-md;
  font-size: $font-size-sm;
  padding: $spacing-sm $spacing-md;
  color: $color-text-secondary;
  text-decoration: none;
  transition: all $transition-fast;
  cursor: pointer;
  background: transparent;
  border: none;
  width: 100%;
  text-align: left;

  &:hover {
    background-color: $color-bg-hover;
    color: $color-text-primary;
  }

  &.active {
    color: $color-text-primary;
    background-color: $color-bg-primary;
  }
}

.icon {
  font-size: $font-size-xl;
  display: flex;
  align-items: center;
  justify-content: center;
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

.card {
  background-color: $color-bg-player;
  margin: $spacing-md;
  border: 1px solid $color-border;
}
</style>
