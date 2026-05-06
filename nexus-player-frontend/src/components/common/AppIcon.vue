<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(defineProps<{
  name: string;
  size?: number | string;
}>(), {
  size: 20,
});

const url = computed(() => `url("/icons/${props.name}.svg")`);
const sizeValue = computed(() => (typeof props.size === 'number' ? `${props.size}px` : props.size));
</script>

<template>
  <span
    :class="$style.icon"
    :style="{
      width: sizeValue,
      height: sizeValue,
      maskImage: url,
      WebkitMaskImage: url,
    }"
    aria-hidden="true"
  />
</template>

<style module lang="scss">
.icon {
  display: inline-block;
  flex-shrink: 0;
  background-color: currentColor;
  mask-repeat: no-repeat;
  mask-position: center;
  mask-size: contain;
  -webkit-mask-repeat: no-repeat;
  -webkit-mask-position: center;
  -webkit-mask-size: contain;
}
</style>
