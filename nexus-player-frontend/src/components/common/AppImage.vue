<template>
  <img
    v-if="displaySrc"
    v-bind="$attrs"
    :src="displaySrc"
    :alt="alt"
    decoding="async"
    loading="lazy"
    @error="onError"
  />
  <slot v-else name="placeholder" />
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useArtwork, type ArtworkVariant } from '@/composables/useArtwork';

interface Props {
  /** Remote cover URL, as stored in the library. */
  src?: string | null;
  alt?: string;
  /** `large` for the player and visualizer, where 256px would look soft. */
  variant?: ArtworkVariant;
}

const props = withDefaults(defineProps<Props>(), {
  src: undefined,
  alt: '',
  variant: 'thumb',
});

// Styling stays with whichever card is using this, so `class` and the rest of the
// attributes go onto the <img> rather than the component root.
defineOptions({ inheritAttrs: false });

const cached = useArtwork(() => props.src, () => props.variant);

const broken = ref(new Set<string>());
watch(
  () => props.src,
  () => {
    broken.value = new Set();
  },
);

/**
 * Prefer the local file, fall back to the remote URL until it has been cached —
 * so a cover seen for the first time still appears immediately — and fall through
 * to the placeholder once every candidate has failed to load.
 */
const displaySrc = computed(() => {
  const candidates = [cached.value, props.src].filter((s): s is string => !!s);
  return candidates.find((s) => !broken.value.has(s));
});

function onError() {
  const current = displaySrc.value;
  if (!current) return;
  // A fresh Set, so the computed above re-evaluates and tries the next candidate.
  broken.value = new Set(broken.value).add(current);
}
</script>
