<template>
  <div :class="$style.nav">
    <button
      v-for="item in PRIMARY_ITEMS"
      :key="item.view"
      :class="[$style.tab, { [$style.active]: activeView === item.view }]"
      @click="select(item.view)"
    >
      <AppIcon :name="item.icon" :size="18" />
      <span>{{ $t(`nav.${item.view}`) }}</span>
    </button>
    <button
      :class="[$style.tab, { [$style.active]: moreOpen || isMoreView }]"
      :aria-expanded="moreOpen"
      @click="moreOpen = !moreOpen"
    >
      <AppIcon name="more-horizontal" :size="18" />
      <span>{{ $t('nav.more') }}</span>
    </button>

    <template v-if="moreOpen">
      <div :class="$style.backdrop" @click="moreOpen = false" />
      <div :class="$style.menu">
        <button
          v-for="item in MORE_ITEMS"
          :key="item.view"
          :class="[$style.menuItem, { [$style.menuActive]: activeView === item.view }]"
          @click="select(item.view)"
        >
          <AppIcon :name="item.icon" :size="18" />
          {{ $t(`nav.${item.view}`) }}
        </button>
        <button :class="$style.menuItem" @click="openSettings">
          <AppIcon name="settings" :size="18" />
          {{ $t('nav.settings') }}
        </button>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import AppIcon from '@/components/common/AppIcon.vue';

const props = defineProps<{ activeView: string }>();
const emit = defineEmits<{ changeView: [view: string]; openSettings: [] }>();

const PRIMARY_ITEMS = [
  { view: 'home', icon: 'home' },
  { view: 'search', icon: 'search' },
  { view: 'library', icon: 'music-note' },
  { view: 'radio', icon: 'radio' },
];
const MORE_ITEMS = [
  { view: 'history', icon: 'history' },
  { view: 'cache', icon: 'album' },
  { view: 'downloads', icon: 'download-tray' },
];

const moreOpen = ref(false);
const isMoreView = computed(() => MORE_ITEMS.some((item) => item.view === props.activeView));

const select = (view: string) => {
  moreOpen.value = false;
  emit('changeView', view);
};

const openSettings = () => {
  moreOpen.value = false;
  emit('openSettings');
};
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.nav {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 1px;
  background: var(--line);
}

.tab {
  height: 54px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
  background: var(--bg-panel);
  color: var(--muted);
  font: 600 0.5938rem $font-family-mono;
  text-transform: uppercase;
}

.active {
  color: var(--accent);
  box-shadow: inset 0 2px 0 var(--accent);
}

.backdrop {
  position: fixed;
  inset: 0;
  background: var(--overlay);
  z-index: $z-mobile-nav;
}

.menu {
  position: fixed;
  right: 8px;
  bottom: calc(62px + env(safe-area-inset-bottom));
  min-width: 200px;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  z-index: $z-mobile-nav + 1;
}

.menuItem {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 12px;
  height: 48px;
  padding: 0 14px;
  font-weight: 600;
  text-align: left;

  & + & {
    border-top: 1px solid var(--line-soft);
  }
}

.menuActive {
  color: var(--accent);
}
</style>
