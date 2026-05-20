<template>
  <div :class="$style.tabs">
    <button
      v-for="item in NAV_ITEMS"
      :key="item.view"
      :class="[$style.tab, { [$style.active]: activeView === item.view }]"
      :title="$t(`nav.${item.view}`)"
      @click="$emit('changeView', item.view)"
    >
      <AppIcon :name="item.icon" :size="15" />
      <span :class="$style.label">{{ $t(`nav.${item.view}`) }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
import AppIcon from '@/components/common/AppIcon.vue';

defineProps<{ activeView: string }>();
defineEmits<{ changeView: [view: string] }>();

const NAV_ITEMS = [
  { view: 'home', icon: 'home' },
  { view: 'search', icon: 'search' },
  { view: 'library', icon: 'music-note' },
  { view: 'radio', icon: 'radio' },
  { view: 'history', icon: 'history' },
  { view: 'cache', icon: 'album' },
  { view: 'downloads', icon: 'download-tray' },
  { view: 'settings', icon: 'settings' },
];
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.tabs {
  display: grid;
  grid-template-columns: var(--tree-width) repeat(7, minmax(0, 1fr));
  gap: 1px;
  background: var(--line);
  border-bottom: 1px solid var(--line);
}

.tab {
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

@media (max-width: 860px) {
  .tab {
    gap: 5px;
    font-size: 0.625rem;
    letter-spacing: 0;
  }
}

// Eight labels stop fitting; the icons stay and each tab's title names it.
@media (max-width: 720px) {
  .label {
    display: none;
  }
}
</style>
