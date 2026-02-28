<template>
  <div
    ref="listRef"
    :class="[$style.container, { [$style.isManaged]: managed }]"
  >
    <div :style="spacerStyle" :class="$style.spacer">
      <div :style="contentStyle" :class="$style.content">
        <div
          v-for="item in visibleItems"
          :key="item.index"
          :style="{ height: `${getItemHeight(item.index)}px` }"
        >
          <slot :item="item.data" :index="item.index"></slot>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue';

const props = defineProps<{
  items: any[];
  itemHeight: number | ((index: number) => number);
  overscan?: number;
  containerSelector?: string;
}>();

const listRef = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const containerHeight = ref(window.innerHeight);
const overscan = props.overscan ?? 5;
const managed = computed(() => !!props.containerSelector);

let scrollContainer: HTMLElement | Window | null = null;
let resizeObserver: ResizeObserver | null = null;

const getItemHeight = (index: number) => {
  if (typeof props.itemHeight === 'function') {
    return props.itemHeight(index);
  }
  return props.itemHeight;
};

const offsets = computed(() => {
  const result = [0];
  for (let i = 0; i < props.items.length; i++) {
    result.push(result[i] + getItemHeight(i));
  }
  return result;
});

const totalHeight = computed(() => offsets.value[offsets.value.length - 1]);

const onScroll = () => {
  if (scrollContainer instanceof Window) {
    scrollTop.value = window.scrollY;
  } else if (scrollContainer) {
    scrollTop.value = scrollContainer.scrollTop;
  }
};

const updateDimensions = () => {
  if (scrollContainer instanceof Window) {
    containerHeight.value = window.innerHeight;
  } else if (scrollContainer) {
    containerHeight.value = (scrollContainer as HTMLElement).clientHeight;
  }
};

onMounted(async () => {
  await nextTick();
  
  if (props.containerSelector) {
    scrollContainer = document.querySelector(props.containerSelector);
    if (!scrollContainer) {
        scrollContainer = window;
    }
  } else {
    scrollContainer = listRef.value;
  }

  updateDimensions();
  
  if (scrollContainer) {
    scrollContainer.addEventListener('scroll', onScroll, { passive: true });
    if (scrollContainer === window) {
      window.addEventListener('resize', updateDimensions);
    } else {
      resizeObserver = new ResizeObserver(updateDimensions);
      resizeObserver.observe(scrollContainer as HTMLElement);
    }
  }
});

onUnmounted(() => {
  if (scrollContainer) {
    scrollContainer.removeEventListener('scroll', onScroll);
    if (scrollContainer === window) {
      window.removeEventListener('resize', updateDimensions);
    } else if (resizeObserver) {
      resizeObserver.disconnect();
    }
  }
});

const getListOffset = () => {
    if (!listRef.value || !scrollContainer) return 0;
    if (scrollContainer === window) {
        return listRef.value.getBoundingClientRect().top + window.scrollY;
    }
    const containerRect = (scrollContainer as HTMLElement).getBoundingClientRect();
    const listRect = listRef.value.getBoundingClientRect();
    return listRect.top - containerRect.top + (scrollContainer as HTMLElement).scrollTop;
};

const startIndex = computed(() => {
  const offset = getListOffset();
  const adjustedScroll = Math.max(0, scrollTop.value - offset);
  
  let low = 0;
  let high = offsets.value.length - 1;
  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    if (offsets.value[mid] <= adjustedScroll) {
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  return Math.max(0, low - 1 - overscan);
});

const endIndex = computed(() => {
  const offset = getListOffset();
  const adjustedScroll = Math.max(0, scrollTop.value - offset) + containerHeight.value;
  
  let low = 0;
  let high = offsets.value.length - 1;
  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    if (offsets.value[mid] <= adjustedScroll) {
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  return Math.min(props.items.length, low + overscan);
});

const visibleItems = computed(() => {
  return props.items.slice(startIndex.value, endIndex.value).map((data, idx) => ({
    data,
    index: startIndex.value + idx,
  }));
});

const spacerStyle = computed(() => ({
  height: `${totalHeight.value}px`,
  position: 'relative' as const,
}));

const contentStyle = computed(() => ({
  position: 'absolute' as const,
  top: 0,
  left: 0,
  right: 0,
  transform: `translateY(${offsets.value[startIndex.value]}px)`,
}));
</script>

<style module lang="scss">
.container {
  height: 100%;
  overflow-y: auto;
  position: relative;
  will-change: transform;

  &.isManaged {
    overflow-y: visible;
    height: auto;
  }
}

.spacer {
  width: 100%;
}

.content {
  width: 100%;
}
</style>
