<template>
  <div 
    ref="containerRef" 
    :class="[$style.marqueeContainer, { [$style.isActive]: active || isHovered }]"
    @mouseenter="isHovered = true"
    @mouseleave="isHovered = false"
  >
    <div 
      ref="contentRef" 
      :class="$style.marqueeContent" 
      :style="scrollStyle"
    >
      <slot></slot>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, onUnmounted, watch } from 'vue';

const props = defineProps<{
  active?: boolean;
  durationPerPx?: number; // Seconds per pixel for consistent speed
}>();

const containerRef = ref<HTMLElement | null>(null);
const contentRef = ref<HTMLElement | null>(null);
const isHovered = ref(false);
const overflowAmount = ref(0);
let resizeObserver: ResizeObserver | null = null;

const checkOverflow = () => {
  if (containerRef.value && contentRef.value) {
    // We measure the difference between scrollWidth and clientWidth
    const amount = contentRef.value.scrollWidth - containerRef.value.clientWidth;
    overflowAmount.value = Math.max(0, amount);
  }
};

const scrollStyle = computed(() => {
  if (overflowAmount.value <= 0) return {};
  
  // Dynamic duration based on length if desired, or fixed
  const duration = props.durationPerPx 
    ? Math.max(3, overflowAmount.value * props.durationPerPx) 
    : 5;

  return {
    '--overflow-shift': `-${overflowAmount.value}px`,
    '--animation-duration': `${duration}s`
  };
});

onMounted(() => {
  checkOverflow();
  
  // Watch for size changes
  resizeObserver = new ResizeObserver(() => {
    checkOverflow();
  });
  
  if (containerRef.value) resizeObserver.observe(containerRef.value);
  if (contentRef.value) resizeObserver.observe(contentRef.value);
});

onUnmounted(() => {
  resizeObserver?.disconnect();
});

// Also check when content might change (slot updates)
watch(() => props.active, () => {
  checkOverflow();
});
</script>

<style module lang="scss">
.marqueeContainer {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  width: 100%;
  display: block;
  min-width: 0;
}

.isActive {
  text-overflow: clip;
  
  .marqueeContent {
    animation: marquee-dynamic-reveal var(--animation-duration, 5s) linear infinite alternate;
  }
}

.marqueeContent {
  display: inline-block;
  vertical-align: top;
  will-change: transform;
  max-width: 100%; // Default state
}

.isActive .marqueeContent {
  max-width: none; // Allow it to be as wide as content when animating
}

@keyframes marquee-dynamic-reveal {
  0%, 20% { transform: translateX(0); }
  80%, 100% { transform: translateX(var(--overflow-shift, 0)); }
}
</style>
