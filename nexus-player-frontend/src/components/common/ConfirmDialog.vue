<template>
  <Teleport to="body">
    <div v-if="confirmRequest" :class="$style.overlay" @click.self="settleConfirm(false)">
      <div
        :class="$style.dialog"
        role="alertdialog"
        aria-modal="true"
        :aria-label="confirmRequest.title"
        @keydown.esc.stop="settleConfirm(false)"
      >
        <h2 :class="$style.title">{{ confirmRequest.title }}</h2>
        <p v-if="confirmRequest.message" :class="$style.message">{{ confirmRequest.message }}</p>
        <div :class="$style.actions">
          <button ref="cancelRef" :class="$style.button" @click="settleConfirm(false)">
            {{ $t('common.cancel') }}
          </button>
          <button :class="[$style.button, $style.danger]" @click="settleConfirm(true)">
            {{ confirmRequest.confirmLabel }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { confirmRequest, settleConfirm } from '@/composables/useConfirm';

const cancelRef = ref<HTMLButtonElement | null>(null);

// Focus the safe choice, so a stray Enter or Space does not confirm.
watch(confirmRequest, async (request) => {
  if (!request) return;
  await nextTick();
  cancelRef.value?.focus();
});
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
  z-index: $z-modal + 100;
}

.dialog {
  width: min(400px, 100%);
  padding: 16px;
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-xl);
}

.title {
  font-size: var(--font-size-lg);
  font-weight: 600;
  overflow-wrap: anywhere;
}

.message {
  margin-top: 6px;
  color: var(--muted);
  font-size: var(--font-size-sm);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 16px;
}

.button {
  height: 34px;
  padding: 0 14px;
  background: var(--key);
  border-radius: var(--radius-sm);
  box-shadow: inset 0 1px 0 var(--key-highlight), 0 2px 0 var(--key-edge);
  font: 600 var(--font-size-xs) $font-family-mono;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.danger {
  color: var(--danger);
}

@media (max-width: $mobile-breakpoint) {
  .button {
    height: 44px;
    flex: 1;
  }
}
</style>
