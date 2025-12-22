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
      <a href="#" :class="[$style.navItem, $style.active]">
        <span :class="$style.icon">🏠</span>
        <span>Главная</span>
      </a>
      <a href="#" :class="$style.navItem">
        <span :class="$style.icon">🔍</span>
        <span>Поиск</span>
      </a>
      <a href="#" :class="$style.navItem">
        <span :class="$style.icon">🎵</span>
        <span>Музыка</span>
      </a>
      <a href="#" :class="$style.navItem">
        <span :class="$style.icon">📋</span>
        <span>Плейлисты</span>
      </a>
    </nav>

    <div :class="$style.library">
      <h3 :class="$style.libraryTitle">Моя библиотека</h3>
    </div>
  </aside>
</template>

<script setup lang="ts">
// Navigation logic will be added here
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.sidebar {
  width: $sidebar-width;
  height: 100vh;
  background-color: $color-bg-sidebar;
  border-right: 1px solid $color-border;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
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
  background: linear-gradient(135deg, $color-accent-primary 0%, #764ba2 100%);
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
    background-color: rgba(255, 255, 255, 0.1);
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
</style>
