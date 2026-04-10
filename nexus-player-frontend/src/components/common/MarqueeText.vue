<template>
  <div
    v-if="MARQUEE_ENABLED"
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
  <!-- MARQUEE_ENABLED=false: bare span, zero JS overhead -->
  <span v-else :class="$style.marqueeContainer"><slot></slot></span>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, onUnmounted, watch } from 'vue';

// Toggle to re-enable animations
const MARQUEE_ENABLED = false;

const props = defineProps<{
  active?: boolean;
  durationPerPx?: number;
}>();

const containerRef = ref<HTMLElement | null>(null);
const contentRef = ref<HTMLElement | null>(null);
const isHovered = ref(false);
const overflowAmount = ref(0);
let resizeObserver: ResizeObserver | null = null;

const checkOverflow = () => {
  if (containerRef.value && contentRef.value) {
    const amount = contentRef.value.scrollWidth - containerRef.value.clientWidth;
    overflowAmount.value = Math.max(0, amount);
  }
};

const scrollStyle = computed(() => {
  if (overflowAmount.value <= 0) return {};
  const duration = props.durationPerPx
    ? Math.max(3, overflowAmount.value * props.durationPerPx)
    : 5;
  return {
    '--overflow-shift': `-${overflowAmount.value}px`,
    '--animation-duration': `${duration}s`,
  };
});

onMounted(() => {
  if (!MARQUEE_ENABLED) return;
  checkOverflow();
  resizeObserver = new ResizeObserver(() => { checkOverflow(); });
  if (containerRef.value) resizeObserver.observe(containerRef.value);
  if (contentRef.value) resizeObserver.observe(contentRef.value);
});

onUnmounted(() => {
  resizeObserver?.disconnect();
});

watch(() => props.active, () => {
  if (MARQUEE_ENABLED) checkOverflow();
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

// ANIMATIONS DISABLED — set MARQUEE_ENABLED=true and uncomment to re-enable
// .isActive {
//   text-overflow: clip;
//
//   .marqueeContent {
//     animation: marquee-dynamic-reveal var(--animation-duration, 5s) linear infinite alternate;
//   }
// }

.isActive {
  text-overflow: clip;
}

.marqueeContent {
  display: inline-block;
  vertical-align: top;
  will-change: transform;
  max-width: 100%;
}

.isActive .marqueeContent {
  max-width: none;
}

@keyframes marquee-dynamic-reveal {
  0%, 20% { transform: translateX(0); }
  80%, 100% { transform: translateX(var(--overflow-shift, 0)); }
}
</style>
