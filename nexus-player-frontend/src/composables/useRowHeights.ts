import { computed } from 'vue';
import { useMediaQuery } from '@vueuse/core';

// VirtualList positions rows by these numbers, so they must match --row-height and
// --row-height-compact in styles/themes.scss.
const ROW_HEIGHT = 34;
const ROW_HEIGHT_COMPACT = 42;
const ROW_HEIGHT_MOBILE = 54;

export function useRowHeights() {
  const isMobile = useMediaQuery('(max-width: 640px)');
  return {
    table: computed(() => (isMobile.value ? ROW_HEIGHT_MOBILE : ROW_HEIGHT)),
    compact: computed(() => (isMobile.value ? ROW_HEIGHT_MOBILE : ROW_HEIGHT_COMPACT)),
  };
}
