<template>
  <div class="notifications-container">
    <TransitionGroup name="list">
      <div 
        v-for="n in store.notifications" 
        :key="n.id" 
        :class="['notification', n.type]"
        @click="store.removeNotification(n.id)"
      >
        {{ n.message }}
      </div>
    </TransitionGroup>
  </div>
</template>

<script setup lang="ts">
import { useNotificationStore } from '@/stores/notifications';
const store = useNotificationStore();
</script>

<style scoped lang="scss">
.notifications-container {
  position: fixed;
  top: calc(12px + env(safe-area-inset-top));
  right: 12px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-width: min(360px, calc(100vw - 24px));
  pointer-events: none;
}

.notification {
  pointer-events: auto;
  padding: 10px 12px;
  background: var(--bg-panel);
  color: var(--text);
  border: 1px solid var(--line);
  border-left: 3px solid var(--muted);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  cursor: pointer;
  font-size: var(--font-size-base);
  overflow-wrap: anywhere;

  &.error { border-left-color: var(--danger); }
  &.warning { border-left-color: var(--warning); }
  &.success { border-left-color: var(--success); }
  &.info { border-left-color: var(--accent); }
}

.list-enter-active,
.list-leave-active {
  transition: opacity var(--transition-base), transform var(--transition-base);
}
.list-enter-from {
  opacity: 0;
  transform: translateX(30px);
}
.list-leave-to {
  opacity: 0;
  transform: translateX(30px);
}
</style>
