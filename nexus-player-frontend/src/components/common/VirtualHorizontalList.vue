<template>
  <div
    ref="listRef"
    :class="$style.container"
    :style="{ height: height ? `${height}px` : 'auto' }"
  >
    <div :style="spacerStyle">
      <div ref="contentRef" :style="contentStyle">
        <div
          v-for="item in visibleItems"
          :key="item.index"
          :class="$style.item"
          :style="{ width: `${itemWidth}px` }"
        >
          <slot :item="item.data" :index="item.index"></slot>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue';

const props = defineProps<{
  items: any[];
  itemWidth: number;
  gap?: number;
  height?: number;
  overscan?: number;
}>();

const listRef = ref<HTMLElement | null>(null);
const contentRef = ref<HTMLElement | null>(null);
const containerWidth = ref(0);

const gap = computed(() => props.gap ?? 0);
const overscan = computed(() => props.overscan ?? 2);
const stride = computed(() => props.itemWidth + gap.value);

const totalWidth = computed(() =>
  props.items.length === 0
    ? 0
    : props.items.length * props.itemWidth + (props.items.length - 1) * gap.value
);

// Plain refs — Vue re-renders only when the virtual window actually shifts,
// NOT on every scroll frame.
const startIndex = ref(0);
const endIndex = ref(0);

let rafId: number | null = null;
let resizeObserver: ResizeObserver | null = null;

const calcStart = (sl: number) =>
  Math.max(0, Math.floor(sl / stride.value) - overscan.value);

const calcEnd = (sl: number) =>
  Math.min(
    props.items.length,
    Math.ceil((sl + containerWidth.value) / stride.value) + overscan.value
  );

const onScroll = () => {
  if (rafId !== null) return;
  rafId = requestAnimationFrame(() => {
    if (!listRef.value) { rafId = null; return; }

    const sl = listRef.value.scrollLeft;
    const newStart = calcStart(sl);
    const newEnd = calcEnd(sl);

    // Direct DOM write — bypasses Vue reactivity, no virtual DOM diff every frame
    if (contentRef.value) {
      contentRef.value.style.transform = `translateX(${newStart * stride.value}px)`;
    }

    // Vue re-render only when items actually enter/leave the window
    if (newStart !== startIndex.value || newEnd !== endIndex.value) {
      startIndex.value = newStart;
      endIndex.value = newEnd;
    }

    rafId = null;
  });
};

const onWheel = (e: WheelEvent) => {
  if (!listRef.value) return;
  // Native horizontal scroll (trackpad) — don't interfere
  if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;

  const el = listRef.value;
  const atStart = el.scrollLeft <= 0;
  const atEnd = el.scrollLeft >= el.scrollWidth - el.clientWidth - 1;

  // At edges — let parent scroll vertically
  if ((e.deltaY < 0 && atStart) || (e.deltaY > 0 && atEnd)) return;

  e.preventDefault();
  el.scrollLeft += e.deltaY;
};

const updateDimensions = () => {
  if (!listRef.value) return;
  containerWidth.value = listRef.value.clientWidth;
  const sl = listRef.value.scrollLeft;
  const newEnd = calcEnd(sl);
  if (newEnd !== endIndex.value) endIndex.value = newEnd;
};

onMounted(async () => {
  await nextTick();
  if (!listRef.value) return;

  updateDimensions();

  if (contentRef.value) {
    contentRef.value.style.transform = 'translateX(0px)';
  }

  listRef.value.addEventListener('scroll', onScroll, { passive: true });
  listRef.value.addEventListener('wheel', onWheel, { passive: false });

  resizeObserver = new ResizeObserver(updateDimensions);
  resizeObserver.observe(listRef.value);
});

onUnmounted(() => {
  listRef.value?.removeEventListener('scroll', onScroll);
  listRef.value?.removeEventListener('wheel', onWheel);
  resizeObserver?.disconnect();
  if (rafId !== null) cancelAnimationFrame(rafId);
});

const visibleItems = computed(() =>
  props.items.slice(startIndex.value, endIndex.value).map((data, idx) => ({
    data,
    index: startIndex.value + idx,
  }))
);

const spacerStyle = computed(() => ({
  width: `${totalWidth.value}px`,
  height: '100%',
  position: 'relative' as const,
}));

// No transform here — updated imperatively via contentRef.style.transform
const contentStyle = computed(() => ({
  position: 'absolute' as const,
  top: 0,
  left: 0,
  bottom: 0,
  display: 'flex',
  gap: `${gap.value}px`,
  willChange: 'transform',
}));
</script>

<style module lang="scss">
.container {
  width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  position: relative;
  touch-action: pan-x;
  overscroll-behavior-x: contain;

  &::-webkit-scrollbar {
    height: 4px;
  }

  &::-webkit-scrollbar-thumb {
    background: var(--color-border-light);
    border-radius: 2px;
  }
}

.item {
  flex-shrink: 0;
  // Isolate layout recalculations: changes inside don't invalidate outside
  contain: layout style;
}
</style>
