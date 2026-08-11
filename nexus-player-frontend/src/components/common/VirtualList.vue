<template>
  <div
    ref="listRef"
    :class="[$style.container, { [$style.isManaged]: managed }]"
  >
    <div :style="spacerStyle">
      <div ref="contentRef" :style="contentStyle">
        <div
          v-for="item in visibleItems"
          :key="item.index"
          :class="$style.item"
          :style="{ height: `${getItemHeight(item.index)}px` }"
        >
          <slot :item="item.data" :index="item.index"></slot>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts" generic="T">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue';

const props = defineProps<{
  items: T[];
  itemHeight: number | ((index: number) => number);
  overscan?: number;
  containerSelector?: string;
}>();

const listRef = ref<HTMLElement | null>(null);
const contentRef = ref<HTMLElement | null>(null);
const containerHeight = ref(0);
// Cached offset of list top relative to scroll container — updated on mount/resize,
// NOT on every scroll event. Avoids getBoundingClientRect() causing forced reflow each frame.
const listOffset = ref(0);

const managed = computed(() => !!props.containerSelector);
const overscan = props.overscan ?? 3;

let scrollContainer: HTMLElement | Window | null = null;
let resizeObserver: ResizeObserver | null = null;
let rafId: number | null = null;

// Plain refs — Vue re-renders only when the virtual window changes, not every scroll frame
const startIndex = ref(0);
const endIndex = ref(0);

const getItemHeight = (index: number) =>
  typeof props.itemHeight === 'function' ? props.itemHeight(index) : props.itemHeight;

const offsets = computed(() => {
  const result = [0];
  for (let i = 0; i < props.items.length; i++) {
    result.push(result[i] + getItemHeight(i));
  }
  return result;
});

const totalHeight = computed(() => offsets.value[offsets.value.length - 1] ?? 0);

const getCurrentScrollTop = (): number => {
  if (scrollContainer instanceof Window) return window.scrollY;
  return (scrollContainer as HTMLElement)?.scrollTop ?? 0;
};

// Called rarely (mount, resize) — safe to use getBoundingClientRect here
const updateListOffset = () => {
  if (!listRef.value || !scrollContainer) return;
  if (scrollContainer instanceof Window) {
    listOffset.value = listRef.value.getBoundingClientRect().top + window.scrollY;
  } else {
    const containerEl = scrollContainer as HTMLElement;
    const containerRect = containerEl.getBoundingClientRect();
    const listRect = listRef.value.getBoundingClientRect();
    listOffset.value = listRect.top - containerRect.top + containerEl.scrollTop;
  }
};

const updateContainerHeight = () => {
  if (scrollContainer instanceof Window) {
    containerHeight.value = window.innerHeight;
  } else if (scrollContainer) {
    containerHeight.value = (scrollContainer as HTMLElement).clientHeight;
  }
};

const bsearch = (arr: number[], target: number): number => {
  let lo = 0, hi = arr.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (arr[mid] <= target) lo = mid + 1;
    else hi = mid - 1;
  }
  return lo;
};

const applyFrame = (scrollTop: number) => {
  const offs = offsets.value;
  const adjusted = Math.max(0, scrollTop - listOffset.value);
  const newStart = Math.max(0, bsearch(offs, adjusted) - 1 - overscan);
  const newEnd = Math.min(props.items.length, bsearch(offs, adjusted + containerHeight.value) + overscan);

  // Direct DOM write — no Vue reactive overhead every frame
  if (contentRef.value) {
    contentRef.value.style.transform = `translateY(${offs[newStart] ?? 0}px)`;
  }

  // Vue re-render only when the virtual window actually changes
  if (newStart !== startIndex.value || newEnd !== endIndex.value) {
    startIndex.value = newStart;
    endIndex.value = newEnd;
  }
};

const onScroll = () => {
  if (rafId !== null) return;
  rafId = requestAnimationFrame(() => {
    applyFrame(getCurrentScrollTop());
    rafId = null;
  });
};

const updateDimensions = () => {
  updateContainerHeight();
  updateListOffset();
  applyFrame(getCurrentScrollTop());
};

onMounted(async () => {
  await nextTick();

  if (props.containerSelector) {
    scrollContainer = document.querySelector<HTMLElement>(props.containerSelector) ?? window;
  } else {
    scrollContainer = listRef.value;
  }

  updateContainerHeight();
  updateListOffset();

  const sl = getCurrentScrollTop();
  applyFrame(sl);

  scrollContainer?.addEventListener('scroll', onScroll, { passive: true });

  if (scrollContainer === window) {
    window.addEventListener('resize', updateDimensions);
  } else {
    resizeObserver = new ResizeObserver(updateDimensions);
    resizeObserver.observe(scrollContainer as HTMLElement);
    // Content above a managed list can grow after mount, which moves the list down.
    const content = (scrollContainer as HTMLElement).firstElementChild;
    if (managed.value && content) resizeObserver.observe(content);
  }
});

onUnmounted(() => {
  scrollContainer?.removeEventListener('scroll', onScroll);
  if (scrollContainer === window) {
    window.removeEventListener('resize', updateDimensions);
  }
  resizeObserver?.disconnect();
  if (rafId !== null) cancelAnimationFrame(rafId);
});

// Recalculate when items list changes
watch(
  () => props.items,
  () => {
    nextTick(() => {
      updateListOffset();
      applyFrame(getCurrentScrollTop());
    });
  },
  { flush: 'post' }
);

const visibleItems = computed(() =>
  props.items.slice(startIndex.value, endIndex.value).map((data, idx) => ({
    data,
    index: startIndex.value + idx,
  }))
);

const spacerStyle = computed(() => ({
  height: `${totalHeight.value}px`,
  position: 'relative' as const,
}));

// No transform — set imperatively via contentRef.style.transform
const contentStyle = {
  position: 'absolute' as const,
  top: 0,
  left: 0,
  right: 0,
  willChange: 'transform',
};
</script>

<style module lang="scss">
.container {
  height: 100%;
  overflow-y: auto;
  position: relative;

  &.isManaged {
    overflow-y: visible;
    height: auto;
  }
}

.item {
  contain: layout style;
}
</style>
