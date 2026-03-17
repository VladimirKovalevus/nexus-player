<template>
  <div :class="$style.downloadsView">
    <header :class="$style.header">
      <h1 :class="$style.title">
        Загрузки
        <span :class="$style.totalSize" v-if="tasks.length > 0">
          ({{ formattedTotalSize }})
        </span>
      </h1>
    </header>

    <div v-if="tasks.length === 0" :class="$style.empty">
      Нет активных загрузок
    </div>

    <div v-else :class="$style.taskList">
      <div v-for="task in sortedTasks" :key="task.trackId" :class="$style.taskCard">
        <div :class="$style.taskInfo">
          <div :class="$style.taskTitle">{{ task.meta?.title || `Трек #${task.trackId}` }}</div>
          <div :class="$style.taskArtist" v-if="task.meta?.artist">{{ task.meta.artist }}</div>
          <div :class="$style.taskUrl" v-else>{{ task.url }}</div>
          <div :class="$style.taskMeta">
            <span :class="[$style.state, $style[`state-${task.state.toLowerCase()}`]]">
              {{ formatState(task.state) }}
            </span>
            <span :class="$style.size">{{ task.sizeHuman }}</span>
            <span :class="$style.duration" v-if="task.meta?.durationSecs">
              {{ formatDuration(task.meta.durationSecs) }}
            </span>
          </div>
        </div>

        <div :class="$style.progressContainer">
          <div :class="$style.progressBar">
            <div 
              :class="$style.progressFill" 
              :style="{ width: `${task.progress * 100}%` }"
            ></div>
          </div>
          <div :class="$style.progressText">{{ Math.round(task.progress * 100) }}%</div>
        </div>

        <button :class="$style.removeBtn" @click="removeTask(task)" title="Удалить">
          ✕
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { TaskInfoDto } from '@/types/types';

const tasks = ref<TaskInfoDto[]>([]);

const totalSize = computed(() => {
  return tasks.value.reduce((acc, task) => acc + (task.byteSize || 0), 0);
});

const formattedTotalSize = computed(() => {
  return formatBytes(totalSize.value);
});

const sortedTasks = computed(() => {
  return [...tasks.value].sort((a, b) => b.trackId - a.trackId);
});

const fetchTasks = async () => {
  try {
    tasks.value = await invoke<TaskInfoDto[]>('list_download_tasks');
  } catch (e) {
    console.error('Failed to fetch download tasks:', e);
  }
};

const removeTask = async (track: TaskInfoDto) => {
  try {
    await invoke('remove_download_task', { trackId:track.meta.id });
    tasks.value = tasks.value.filter(t => t?.meta?.id !== track.meta.id);
  } catch (e) {
    console.error('Failed to remove task:', e);
  }
};

const formatState = (state: string) => {
  if (state.startsWith('Failed:')) return 'Ошибка';
  switch (state) {
    case 'Pending': return 'В очереди';
    case 'Downloading': return 'Загрузка';
    case 'Complete': return 'Готово';
    case 'Cancelled': return 'Отменено';
    default: return state;
  }
};

const formatDuration = (secs: number) => {
  const mins = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${mins}:${s.toString().padStart(2, '0')}`;
};

const formatBytes = (bytes: number) => {
  const kb = 1024;
  const mb = kb * 1024;
  const gb = mb * 1024;

  if (bytes >= gb) return `${(bytes / gb).toFixed(2)} GB`;
  if (bytes >= mb) return `${(bytes / mb).toFixed(2)} MB`;
  if (bytes >= kb) return `${(bytes / kb).toFixed(2)} KB`;
  return `${bytes} B`;
};

let unlistenStart: any;
let unlistenProgress: any;
let unlistenFinish: any;

onMounted(async () => {
  await fetchTasks();

  unlistenStart = await listen<[number, string]>('download-start', () => {
    fetchTasks(); // Refresh to get all meta
  });

  unlistenProgress = await listen<[number, number, number]>('download-progress', (event) => {
    const [trackId, progress, byteSize] = event.payload;
    const task = tasks.value.find(t => t.trackId === trackId);
    if (task) {
      task.progress = progress;
      task.byteSize = byteSize;
      task.sizeHuman = formatBytes(byteSize);
      task.state = 'Downloading';
    }
  });

  unlistenFinish = await listen<[number, boolean, string]>('download-finish', (event) => {
    const [trackId, success] = event.payload;
    const task = tasks.value.find(t => t.trackId === trackId);
    if (task) {
      task.state = success ? 'Complete' : 'Failed';
      task.progress = success ? 1.0 : task.progress;
    }
  });
});

onUnmounted(() => {
  if (unlistenStart) unlistenStart();
  if (unlistenProgress) unlistenProgress();
  if (unlistenFinish) unlistenFinish();
});
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.downloadsView {
  padding: $spacing-xl;
  height: 100%;
  overflow-y: auto;
}

.header {
  margin-bottom: $spacing-2xl;
}

.title {
  font-size: $font-size-3xl;
  font-weight: $font-weight-bold;
  color: var(--color-text-primary);
  display: flex;
  align-items: baseline;
  gap: $spacing-md;
}

.totalSize {
  font-size: $font-size-lg;
  font-weight: $font-weight-normal;
  color: var(--color-text-secondary);
}

.empty {
  text-align: center;
  padding: $spacing-3xl;
  color: var(--color-text-secondary);
  font-size: $font-size-lg;
}

.taskList {
  display: flex;
  flex-direction: column;
  gap: $spacing-md;
}

.taskCard {
  background: var(--color-bg-secondary);
  border-radius: $radius-lg;
  padding: $spacing-lg;
  display: grid;
  grid-template-columns: 1fr 200px auto;
  align-items: center;
  gap: $spacing-xl;
  border: 1px solid var(--color-border);
  position: relative;
}

.taskInfo {
  min-width: 0;
}

.taskTitle {
  font-weight: $font-weight-bold;
  margin-bottom: 2px;
}

.taskArtist {
  font-size: $font-size-sm;
  color: var(--color-text-secondary);
  margin-bottom: 4px;
}

.taskUrl {
  font-size: $font-size-xs;
  color: var(--color-text-secondary);
  @include text-ellipsis;
}

.taskMeta {
  display: flex;
  gap: $spacing-md;
  margin-top: $spacing-sm;
  font-size: $font-size-xs;
}

.state {
  font-weight: $font-weight-semibold;

  &-pending { color: #f1c40f; }
  &-downloading { color: var(--color-accent-primary); }
  &-complete { color: #2ecc71; }
  &-failed { color: #e74c3c; }
}

.duration {
  color: var(--color-text-secondary);
}

.progressContainer {
  display: flex;
  align-items: center;
  gap: $spacing-md;
}

.progressBar {
  flex: 1;
  height: 6px;
  background: var(--color-bg-hover);
  border-radius: 3px;
  overflow: hidden;
}

.progressFill {
  height: 100%;
  background: var(--color-accent-primary);
  transition: width 0.3s ease;
}

.progressText {
  font-size: $font-size-xs;
  min-width: 35px;
  text-align: right;
}

.removeBtn {
  background: none;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: $spacing-sm;
  font-size: 18px;
  line-height: 1;
  
  &:hover {
    color: #e74c3c;
  }
}
</style>
