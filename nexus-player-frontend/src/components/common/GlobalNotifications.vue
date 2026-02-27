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
  top: 20px;
  right: 20px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 10px;
  pointer-events: none;
}

.notification {
  pointer-events: auto;
  padding: 12px 20px;
  border-radius: 8px;
  color: white;
  min-width: 200px;
  max-width: 300px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.3);
  cursor: pointer;
  font-size: 14px;
  word-break: break-word;

  &.error { background-color: #f44336; }
  &.info { background-color: #2196f3; }
  &.success { background-color: #4caf50; }
}

.list-enter-active,
.list-leave-active {
  transition: all 0.5s ease;
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
