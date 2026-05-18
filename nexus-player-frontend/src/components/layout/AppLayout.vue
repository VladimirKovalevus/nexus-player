<template>
  <div :class="$style.appLayout">
    <header :class="$style.deck">
      <slot name="deck" />
    </header>

    <nav :class="$style.tabs">
      <slot name="tabs" />
    </nav>

    <div :class="$style.body">
      <aside :class="$style.tree">
        <slot name="tree" />
      </aside>

      <main :class="$style.main">
        <slot />
      </main>

      <Transition name="queue-backdrop">
        <div v-if="showQueue" :class="$style.backdrop" @click="$emit('closeQueue')" />
      </Transition>

      <!-- v-show keeps the queue mounted: commands reach it while it is hidden. -->
      <Transition name="queue-panel">
        <aside v-show="showQueue" :class="$style.queue">
          <slot name="queue" />
        </aside>
      </Transition>
    </div>

    <nav :class="$style.mobileNav">
      <slot name="mobileNav" />
    </nav>
  </div>
</template>

<script setup lang="ts">
defineProps<{ showQueue: boolean }>();
defineEmits<{ closeQueue: [] }>();
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.appLayout {
  display: flex;
  flex-direction: column;
  height: 100dvh;
  width: 100%;
  overflow: hidden;
  background-color: var(--bg-body);
  color: var(--text);
  // The first tab and the playlist tree share this width so their edges line up.
  // Below 1280px the tree keeps a usable size and the first tab grows with it.
  --tree-width: max(160px, 12.5%);
  padding-left: env(safe-area-inset-left);
  padding-right: env(safe-area-inset-right);
}

.deck {
  flex: none;
  padding-top: env(safe-area-inset-top);
  background: var(--bg-panel);
  border-bottom: 1px solid var(--line);
  position: relative;
  z-index: $z-player;
}

.tabs {
  flex: none;
}

.body {
  flex: 1;
  min-height: 0;
  display: grid;
  // The extra pixel holds the tree's right border, in line with the 1px gap after the first tab.
  grid-template-columns: calc(var(--tree-width) + 1px) minmax(0, 1fr) auto;
  position: relative;
}

.tree {
  min-height: 0;
  overflow: auto;
  background: var(--bg-panel);
  border-right: 1px solid var(--line);
}

.main {
  min-width: 0;
  min-height: 0;
  overflow: auto;
  background: var(--bg-table);
}

.backdrop {
  display: none;
}

.queue {
  width: 312px;
  min-height: 0;
  overflow: hidden;
  background: var(--bg-panel);
  border-left: 1px solid var(--line);
}

.mobileNav {
  display: none;
}

// Too narrow to dock the queue next to the table: it floats over the main area.
@media (max-width: 1100px) {
  .body {
    grid-template-columns: calc(var(--tree-width) + 1px) minmax(0, 1fr);
  }

  .backdrop {
    display: block;
    position: absolute;
    inset: 0;
    background: var(--overlay);
    z-index: $z-queue-backdrop;
  }

  .queue {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: $z-queue-overlay;
    box-shadow: var(--shadow-lg);
  }
}

@media (max-width: $mobile-breakpoint) {
  .tabs,
  .tree {
    display: none;
  }

  .body {
    grid-template-columns: minmax(0, 1fr);
  }

  .backdrop {
    position: fixed;
  }

  .queue {
    position: fixed;
    top: auto;
    left: 0;
    width: 100%;
    height: 70dvh;
    border-left: 0;
    border-top: 2px solid var(--accent);
    padding-bottom: env(safe-area-inset-bottom);
  }

  .mobileNav {
    display: block;
    flex: none;
    padding-bottom: env(safe-area-inset-bottom);
    background: var(--bg-panel);
    border-top: 1px solid var(--line);
  }
}
</style>

<style lang="scss">
@media (max-width: 640px) {
  .queue-panel-enter-active,
  .queue-panel-leave-active {
    transition: transform var(--transition-base);
  }

  .queue-panel-enter-from,
  .queue-panel-leave-to {
    transform: translateY(100%);
  }
}

@media (max-width: 1100px) {
  .queue-backdrop-enter-active,
  .queue-backdrop-leave-active {
    transition: opacity var(--transition-base);
  }

  .queue-backdrop-enter-from,
  .queue-backdrop-leave-to {
    opacity: 0;
  }
}
</style>
