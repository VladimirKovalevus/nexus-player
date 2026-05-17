<template>
  <button type="button" role="menuitem" :class="[$style.item, { [$style.danger]: danger }]">
    <AppIcon v-if="icon" :name="icon" :size="15" />
    <span :class="$style.label"><slot /></span>
    <AppIcon v-if="submenu" name="chevron-right" :size="13" />
  </button>
</template>

<script setup lang="ts">
import AppIcon from './AppIcon.vue';

defineProps<{ icon?: string; danger?: boolean; submenu?: boolean }>();
</script>

<style module lang="scss">
@import '@/styles/mixins.scss';

.item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 9px;
  min-height: 34px;
  padding: 6px 11px;
  text-align: left;
  font-size: var(--font-size-sm);

  &:hover,
  &:focus-visible {
    background: var(--bg-selected);
  }
}

.label {
  flex: 1;
  min-width: 0;
  @include text-ellipsis;
}

.danger {
  color: var(--danger);
}

@media (max-width: 640px) {
  .item {
    min-height: 44px;
  }
}
</style>
