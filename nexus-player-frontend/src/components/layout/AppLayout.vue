<template>
  <div ref="appLayoutRef" :class="[$style.appLayout, { [$style.hideQueue]: !showQueue }]">

    <aside :class="$style.leftSidebar">
      <slot name="leftSidebar">
        <Sidebar />
      </slot>
    </aside>

    <main :class="$style.mainContent">
      <slot />
    </main>

    <!-- Mobile backdrop -->
    <Transition name="fade">
      <div
        v-if="showQueue"
        :class="$style.mobileBackdrop"
        @click="$emit('closeQueue')"
      />
    </Transition>

    <Transition name="slide-up">
      <aside v-if="showQueue" :class="$style.rightSidebar">
        <slot name="rightSidebar"> </slot>
      </aside>
    </Transition>

    <footer ref="bottomPlayerRef" :class="$style.bottomPlayer">
      <slot name="bottomPlayer" />
    </footer>

    <nav ref="mobileNavRef" :class="$style.mobileNav">
      <slot name="mobileNav" />
    </nav>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import Sidebar from './Sidebar.vue';
import TopBar from './TopBar.vue';

interface Props {
  showQueue?: boolean;
}

withDefaults(defineProps<Props>(), {
  showQueue: true,
});

defineEmits<{ closeQueue: [] }>();

const appLayoutRef = ref<HTMLElement | null>(null);
const bottomPlayerRef = ref<HTMLElement | null>(null);
const mobileNavRef = ref<HTMLElement | null>(null);

let resizeObserver: ResizeObserver | null = null;

const updateQueueBottom = () => {
  const playerH = bottomPlayerRef.value?.offsetHeight ?? 0;
  const navH = mobileNavRef.value?.offsetHeight ?? 0;
  appLayoutRef.value?.style.setProperty('--queue-bottom', `${playerH + navH}px`);
};

onMounted(() => {
  resizeObserver = new ResizeObserver(updateQueueBottom);
  if (bottomPlayerRef.value) resizeObserver.observe(bottomPlayerRef.value);
  if (mobileNavRef.value) resizeObserver.observe(mobileNavRef.value);
  updateQueueBottom();
});

onUnmounted(() => resizeObserver?.disconnect());
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.appLayout {
  display: grid;
  grid-template-areas:
    'sidebar topbar topbar'
    'sidebar main queue'
    'player player player';
  grid-template-columns: $sidebar-width 1fr minmax(0, 400px);
  grid-template-rows: $topbar-height 1fr $player-height;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: var(--color-bg-primary);
  color: var(--color-text-primary);
  transition: grid-template-columns 0.3s ease;
  padding-left: env(safe-area-inset-left);
  padding-right: env(safe-area-inset-right);
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
  padding-top: env(safe-area-inset-top);
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
  z-index: $z-queue-overlay + 10;
}

.mobileNav {
  display: none;
  grid-area: mobilenav;
  background-color: var(--color-bg-sidebar);
  border-top: 1px solid var(--color-border);
  z-index: $z-player;
  padding-bottom: env(safe-area-inset-bottom);
}

.mobileBackdrop {
  display: none;
}

// Mobile: floating queue overlay
@media (max-width: $mobile-breakpoint) {
  .appLayout {
    grid-template-areas:
      'topbar'
      'main'
      'player'
      'mobilenav';
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr $player-height auto;
  }

  .hideQueue {
    grid-template-areas:
      'topbar'
      'main'
      'player'
      'mobilenav';
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr $player-height auto;
  }

  .leftSidebar {
    display: none;
  }

  .mobileNav {
    display: flex;
  }

  .rightSidebar {
    grid-area: unset;
    position: fixed;
    bottom: var(--queue-bottom, #{$player-height + $mobile-nav-height});
    left: 0;
    right: 0;
    height: 66.666vh;
    z-index: $z-queue-overlay;
    background-color: var(--color-bg-secondary);
    border: none;
    border-top: 1px solid var(--color-border);
    border-radius: 16px 16px 0 0;
    box-shadow: 0 -8px 32px rgba(0, 0, 0, 0.5);
    overflow-y: auto;
  }

  .mobileBackdrop {
    display: block;
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: var(--queue-bottom, #{$player-height + $mobile-nav-height});
    background: rgba(0, 0, 0, 0.5);
    z-index: $z-queue-backdrop;
  }
}
</style>

<style>
/* Transition: backdrop fade */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Transition: queue slides up */
.slide-up-enter-active,
.slide-up-leave-active {
  transition: transform 0.3s ease;
}
.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100%);
}

/* Only apply slide transition on mobile */
@media (min-width: 480px) {
  .slide-up-enter-from,
  .slide-up-leave-to {
    transform: none;
  }
  .slide-up-enter-active,
  .slide-up-leave-active {
    transition: none;
  }
}
</style>
