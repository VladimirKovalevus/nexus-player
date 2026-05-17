<template>
  <div :class="$style.tile" @click="$emit('click')">
    <div :class="$style.art">
      <AppImage :src="thumbnail" :alt="title" :class="$style.image">
        <template #placeholder>
          <span :class="$style.placeholder"><AppIcon :name="placeholderIcon" :size="20" /></span>
        </template>
      </AppImage>
      <button :class="$style.play" :title="$t('player.play')" :aria-label="$t('player.play')" @click.stop="$emit('play')">
        <AppIcon name="play" :size="14" />
      </button>
    </div>
    <div :class="$style.text">
      <div :class="$style.title" :title="title">{{ title }}</div>
      <div :class="$style.subtitle">{{ subtitle }}</div>
    </div>
    <ActionMenu v-if="$slots.menu" :label="$t('common.more')" :trigger-class="$style.more">
      <template #default="menuProps">
        <slot name="menu" v-bind="menuProps" />
      </template>
    </ActionMenu>
  </div>
</template>

<script setup lang="ts">
import AppIcon from './AppIcon.vue';
import AppImage from './AppImage.vue';
import ActionMenu from './ActionMenu.vue';

defineProps<{ title: string; subtitle: string; thumbnail?: string | null; placeholderIcon: string }>();
defineEmits<{ click: []; play: [] }>();
</script>

<style module lang="scss">
@import '@/styles/mixins.scss';

.tile {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr) auto;
  gap: 9px;
  align-items: center;
  padding: 7px 8px 7px 10px;
  background: var(--bg-table);
  cursor: pointer;

  &:hover {
    background: var(--bg-selected);
  }
}

.art {
  position: relative;
  width: 48px;
  height: 48px;
  overflow: hidden;
  background: var(--key);

  &:hover .play,
  .play:focus-visible {
    opacity: 1;
  }
}

.image {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.placeholder {
  @include flex-center;
  width: 100%;
  height: 100%;
  color: var(--muted);
}

.play {
  @include flex-center;
  position: absolute;
  inset: 0;
  background: var(--overlay);
  color: var(--screen-text);
  opacity: 0;
}

.text {
  min-width: 0;
}

.title {
  font-weight: 600;
  @include text-ellipsis;
}

.subtitle {
  font: 500 var(--font-size-xs) var(--font-mono);
  color: var(--muted);
  @include text-ellipsis;
}

.more {
  @include flex-center;
  width: 28px;
  height: 28px;
  color: var(--muted);

  &:hover {
    color: var(--text);
  }
}

@media (hover: none) {
  .play {
    opacity: 1;
    background: none;
    align-items: flex-end;
    justify-content: flex-start;
    padding: 3px;
  }
}
</style>
