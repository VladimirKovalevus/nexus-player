<template>
  <button
    ref="triggerRef"
    type="button"
    :class="triggerClass"
    :title="label"
    :aria-label="label"
    :aria-expanded="isOpen"
    @click.stop="toggle"
  >
    <slot name="trigger"><AppIcon name="more-horizontal" :size="16" /></slot>
  </button>

  <Teleport to="body">
    <div
      v-if="isOpen"
      ref="panelRef"
      :class="$style.panel"
      :style="panelStyle"
      role="menu"
      @click.stop
      @wheel.stop
    >
      <slot :close="close" />
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref } from 'vue';
import AppIcon from './AppIcon.vue';

defineProps<{ label: string; triggerClass?: string }>();
const emit = defineEmits<{ close: [] }>();

const VIEWPORT_MARGIN = 8;
const TRIGGER_GAP = 4;

const isOpen = ref(false);
const triggerRef = ref<HTMLElement | null>(null);
const panelRef = ref<HTMLElement | null>(null);
const panelStyle = ref<Record<string, string>>({});

let panelObserver: ResizeObserver | null = null;

// The panel is fixed to the viewport, so it must be placed from the trigger's position,
// and placed again when its content changes (a submenu has a different height).
const place = () => {
  if (!triggerRef.value || !panelRef.value) return;
  const trigger = triggerRef.value.getBoundingClientRect();
  const height = panelRef.value.offsetHeight;
  const width = panelRef.value.offsetWidth;
  const fitsBelow = trigger.bottom + TRIGGER_GAP + height <= window.innerHeight - VIEWPORT_MARGIN;
  const top = fitsBelow ? trigger.bottom + TRIGGER_GAP : trigger.top - height - TRIGGER_GAP;
  panelStyle.value = {
    top: `${Math.max(VIEWPORT_MARGIN, top)}px`,
    right: `${Math.max(VIEWPORT_MARGIN, Math.min(window.innerWidth - trigger.right, window.innerWidth - width - VIEWPORT_MARGIN))}px`,
  };
};

const handleOutside = (e: Event) => {
  const target = e.target as Node;
  if (panelRef.value?.contains(target) || triggerRef.value?.contains(target)) return;
  close();
};

const handleKey = (e: KeyboardEvent) => {
  if (e.key === 'Escape') close();
};

const removeListeners = () => {
  document.removeEventListener('mousedown', handleOutside);
  document.removeEventListener('scroll', handleOutside, true);
  document.removeEventListener('keydown', handleKey);
  window.removeEventListener('resize', place);
  panelObserver?.disconnect();
  panelObserver = null;
};

function close() {
  if (!isOpen.value) return;
  isOpen.value = false;
  removeListeners();
  emit('close');
}

const toggle = async () => {
  if (isOpen.value) {
    close();
    return;
  }
  isOpen.value = true;
  await nextTick();
  place();
  if (panelRef.value) {
    panelObserver = new ResizeObserver(place);
    panelObserver.observe(panelRef.value);
  }
  document.addEventListener('mousedown', handleOutside);
  document.addEventListener('scroll', handleOutside, true);
  document.addEventListener('keydown', handleKey);
  // Not close: the on-screen keyboard resizes the window when the filter gets focus.
  window.addEventListener('resize', place);
};

onBeforeUnmount(removeListeners);

defineExpose({ close });
</script>

<style module lang="scss">
@import '@/styles/variables.scss';

.panel {
  position: fixed;
  width: 230px;
  max-width: calc(100vw - 16px);
  max-height: min(340px, calc(100dvh - 16px));
  overflow-y: auto;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  z-index: $z-modal - 50;
}
</style>
