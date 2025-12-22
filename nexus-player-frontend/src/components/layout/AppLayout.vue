<template>
  <div :class="[$style.appLayout, { [$style.hideQueue]: !showQueue }]">
    <header :class="$style.topBar">
      <slot name="topBar">
        <TopBar />
      </slot>
    </header>

    <aside :class="$style.leftSidebar">
      <slot name="leftSidebar">
        <Sidebar />
      </slot>
    </aside>

    <main :class="$style.mainContent">
      <slot />
    </main>

    <aside v-if="showQueue" :class="$style.rightSidebar">
      <slot name="rightSidebar"> </slot>
    </aside>

    <footer :class="$style.bottomPlayer">
      <slot name="bottomPlayer" />
    </footer>
  </div>
</template>

<script setup lang="ts">
import Sidebar from './Sidebar.vue';
import TopBar from './TopBar.vue';

interface Props {
  showQueue?: boolean;
}

withDefaults(defineProps<Props>(), {
  showQueue: true,
});
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.appLayout {
  display: grid;
  grid-template-areas:
    'sidebar topbar topbar'
    'sidebar main queue'
    'player player player';
  grid-template-columns: $sidebar-width 1fr 400px;
  grid-template-rows: $topbar-height 1fr $player-height;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: var(--color-bg-primary);
  color: var(--color-text-primary);
  transition: grid-template-columns 0.3s ease;
}

.hideQueue {
  grid-template-areas:
    'sidebar topbar topbar'
    'sidebar main main'
    'player player player';
  grid-template-columns: $sidebar-width 1fr 0px;
}

.topBar {
  grid-area: topbar;
  background-color: var(--color-bg-primary);
  border-bottom: 1px solid var(--color-border);
  z-index: $z-topbar;
}

.leftSidebar {
  grid-area: sidebar;
  background-color: var(--color-bg-sidebar);
  border-right: 1px solid var(--color-border);
  overflow-y: auto;
  z-index: $z-sidebar;
}

.mainContent {
  grid-area: main;
  background-color: var(--color-bg-primary);
  overflow-y: auto;
  overflow-x: hidden;
}

.rightSidebar {
  grid-area: queue;
  background-color: var(--color-bg-secondary);
  border-left: 1px solid var(--color-border);
  overflow-y: auto;
}

.bottomPlayer {
  grid-area: player;
  background-color: var(--color-bg-player);
  border-top: 1px solid var(--color-border);
  z-index: $z-player;
}
</style>
