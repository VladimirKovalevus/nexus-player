<template>
  <div :class="$style.tabs">
    <button
      v-for="item in NAV_ITEMS"
      :key="item.view"
      :class="[$style.tab, { [$style.active]: activeView === item.view }]"
      @click="$emit('changeView', item.view)"
    >
      <AppIcon :name="item.icon" :size="15" />
      <span :class="$style.label">{{ $t(`nav.${item.view}`) }}</span>
    </button>
    <button :class="$style.tab" @click="$emit('openSettings')">
      <AppIcon name="settings" :size="15" />
      <span :class="$style.label">{{ $t('nav.settings') }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
import AppIcon from '@/components/common/AppIcon.vue';

defineProps<{ activeView: string }>();
defineEmits<{ changeView: [view: string]; openSettings: [] }>();

const NAV_ITEMS = [
  { view: 'home', icon: 'home' },
  { view: 'search', icon: 'search' },
  { view: 'library', icon: 'music-note' },
  { view: 'radio', icon: 'radio' },
  { view: 'history', icon: 'history' },
  { view: 'cache', icon: 'album' },
  { view: 'downloads', icon: 'download-tray' },
];
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.tabs {
  display: flex;
  gap: 1px;
  background: var(--line);
  border-bottom: 1px solid var(--line);
}

.tab {
  flex: 1;
  min-width: 0;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  background: var(--bg-panel);
  color: var(--muted);
  font: 600 var(--font-size-xs) $font-family-mono;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  white-space: nowrap;

  &:hover {
    color: var(--text);
  }
}

.active {
  background: var(--bg-table);
  color: var(--accent);
  box-shadow: inset 0 2px 0 var(--accent);
}

// Eight labels stop fitting; the icons stay and the title of each tab is the icon's meaning.
@media (max-width: 860px) {
  .tab {
    gap: 5px;
    font-size: 0.625rem;
    letter-spacing: 0;
  }
}
</style>
