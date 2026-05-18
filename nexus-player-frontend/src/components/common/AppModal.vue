<template>
  <Teleport to="body">
    <div v-if="open" :class="$style.overlay" @click.self="$emit('close')">
      <div
        ref="dialogRef"
        :class="[$style.dialog, { [$style.wide]: wide }]"
        role="dialog"
        aria-modal="true"
        :aria-label="title"
        @keydown.esc.stop="$emit('close')"
      >
        <header :class="$style.header">
          <h2 class="view-title">{{ title }}</h2>
          <button :class="$style.close" :title="$t('common.close')" :aria-label="$t('common.close')" @click="$emit('close')">
            <AppIcon name="close" :size="14" />
          </button>
        </header>
        <div ref="bodyRef" :class="$style.body">
          <slot />
        </div>
        <footer v-if="$slots.footer" :class="$style.footer">
          <slot name="footer" />
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import AppIcon from './AppIcon.vue';

const props = defineProps<{ open: boolean; title: string; wide?: boolean }>();
defineEmits<{ close: [] }>();

const dialogRef = ref<HTMLElement | null>(null);
const bodyRef = ref<HTMLElement | null>(null);

// Focus lands inside so Esc works at once; the first field is what the user fills next.
watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    await nextTick();
    const firstField = bodyRef.value?.querySelector<HTMLElement>('input, textarea, select');
    (firstField ?? dialogRef.value?.querySelector<HTMLElement>('button'))?.focus();
  },
  { immediate: true },
);
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.overlay {
  @include flex-center;
  position: fixed;
  inset: 0;
  padding: 16px;
  background: var(--overlay);
  z-index: $z-modal;
}

.dialog {
  display: flex;
  flex-direction: column;
  width: min(460px, 100%);
  max-height: 100%;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-xl);
}

.wide {
  width: min(600px, 100%);
}

.header {
  @include flex-between;
  gap: 8px;
  min-height: 45px;
  padding: 8px 8px 8px 14px;
  border-bottom: 1px solid var(--line);
}

.close {
  @include flex-center;
  flex: none;
  width: 28px;
  height: 28px;
  color: var(--muted);

  &:hover {
    color: var(--text);
  }
}

.body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  padding: 14px;
  overflow-y: auto;
}

.footer {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  padding: 10px 14px;
  border-top: 1px solid var(--line);
}

@media (max-width: $mobile-breakpoint) {
  .overlay {
    padding: 8px;
  }

  .close {
    width: 44px;
    height: 44px;
  }

  .footer > * {
    flex: 1;
    height: 44px;
  }
}
</style>
