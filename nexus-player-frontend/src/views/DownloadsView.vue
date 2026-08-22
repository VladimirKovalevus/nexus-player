<template>
  <div :class="$style.downloads">
    <div class="view-header">
      <h1 class="view-title">{{ $t('nav.downloads') }}</h1>
      <span v-if="tasks.length > 0" class="view-count">{{ tasks.length }} · {{ formattedTotalSize }}</span>
    </div>

    <p v-if="tasks.length === 0" :class="$style.empty">{{ $t('downloads.empty') }}</p>

    <div v-else>
      <div v-for="task in sortedTasks" :key="task.trackId" :class="$style.row">
        <AppImage :src="task.meta?.thumbnail" :alt="task.meta?.title ?? ''" :class="$style.art">
          <template #placeholder>
            <span :class="$style.placeholder"><AppIcon name="music-note" :size="14" /></span>
          </template>
        </AppImage>

        <div :class="$style.info">
          <div :class="$style.title" :title="task.meta?.title">
            {{ task.meta?.title || $t('downloads.trackFallback', { id: task.trackId }) }}
          </div>
          <div :class="$style.subtitle" :title="task.meta?.artist ?? task.url">
            {{ task.meta?.artist || task.url }}
          </div>
        </div>

        <span
          :class="[$style.state, $style[stateKind(task.state)]]"
          :title="stateDetail(task.state)"
        >{{ $t(`downloads.state.${stateKind(task.state)}`) }}</span>
        <span :class="$style.size">{{ task.sizeHuman }}</span>

        <div :class="$style.progress">
          <div :class="$style.bar">
            <div :class="$style.fill" :style="{ width: `${task.progress * 100}%` }" />
          </div>
          <span :class="$style.percent">{{ Math.round(task.progress * 100) }}%</span>
        </div>

        <button
          :class="$style.remove"
          :title="$t('downloads.remove')"
          :aria-label="$t('downloads.remove')"
          @click="removeTask(task)"
        >
          <AppIcon name="close" :size="14" />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { errorMessage } from '@/utils/errorMessage';
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@/utils/providerAuth';
import { useNotificationStore } from '@/stores/notifications';
import type { TaskInfoDto } from '@/types/types';
import AppIcon from '@/components/common/AppIcon.vue';
import AppImage from '@/components/common/AppImage.vue';

const { t } = useI18n();
const notifications = useNotificationStore();

const tasks = ref<TaskInfoDto[]>([]);

const formattedTotalSize = computed(() =>
  formatBytes(tasks.value.reduce((total, task) => total + (task.byteSize || 0), 0)),
);

const sortedTasks = computed(() => [...tasks.value].sort((a, b) => b.trackId - a.trackId));

const fetchTasks = async () => {
  try {
    tasks.value = await invoke<TaskInfoDto[]>('list_download_tasks');
  } catch (e) {
    console.error('Failed to fetch download tasks:', e);
  }
};

const removeTask = async (task: TaskInfoDto) => {
  try {
    await invoke('remove_download_task', { trackId: task.trackId });
    tasks.value = tasks.value.filter((other) => other.trackId !== task.trackId);
  } catch (e) {
    console.error('Failed to remove task:', e);
    notifications.addNotification(t('common.error', { error: errorMessage(e) }), 'error');
  }
};

type StateKind = 'pending' | 'downloading' | 'complete' | 'cancelled' | 'failed';

const stateKind = (state: string): StateKind => {
  if (state.startsWith('Failed')) return 'failed';
  return state.toLowerCase() as StateKind;
};

/** Failed states carry the reason after a colon; it is shown as a tooltip. */
const stateDetail = (state: string) =>
  state.startsWith('Failed:') ? state.slice('Failed:'.length).trim() : undefined;

const formatBytes = (bytes: number) => {
  const kb = 1024;
  const mb = kb * 1024;
  const gb = mb * 1024;

  if (bytes >= gb) return `${(bytes / gb).toFixed(2)} GB`;
  if (bytes >= mb) return `${(bytes / mb).toFixed(2)} MB`;
  if (bytes >= kb) return `${(bytes / kb).toFixed(2)} KB`;
  return `${bytes} B`;
};

const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
  await fetchTasks();

  unlisteners.push(
    await listen<[number, string]>('download-start', () => {
      fetchTasks();
    }),
    await listen<[number, number, number]>('download-progress', (event) => {
      const [trackId, progress, byteSize] = event.payload;
      const task = tasks.value.find((other) => other.trackId === trackId);
      if (task) {
        task.progress = progress;
        task.byteSize = byteSize;
        task.sizeHuman = formatBytes(byteSize);
        task.state = 'Downloading';
      }
    }),
    await listen<[number, boolean, string]>('download-finish', (event) => {
      const [trackId, success, message] = event.payload;
      const task = tasks.value.find((other) => other.trackId === trackId);
      if (task) {
        task.state = success ? 'Complete' : `Failed: ${message}`;
        task.progress = success ? 1 : task.progress;
      }
    }),
  );
});

onUnmounted(() => {
  unlisteners.forEach((unlisten) => unlisten());
});
</script>

<style module lang="scss">
@import '@/styles/variables.scss';
@import '@/styles/mixins.scss';

.downloads {
  height: 100%;
  overflow-y: auto;
}

.empty {
  padding: 24px 12px;
  text-align: center;
  color: var(--muted);
}

.row {
  display: grid;
  grid-template-columns: 30px minmax(0, 1fr) 84px 72px minmax(120px, 200px) 28px;
  align-items: center;
  column-gap: 10px;
  min-height: var(--row-height-compact);
  padding: 4px 6px 4px 12px;
  border-bottom: 1px solid var(--line-soft);
}

.art,
.placeholder {
  display: block;
  width: 30px;
  height: 30px;
  object-fit: cover;
}

.placeholder {
  @include flex-center;
  background: var(--key);
  color: var(--muted);
}

.info {
  min-width: 0;
  line-height: 1.2;
}

.title {
  font-weight: 500;
  @include text-ellipsis;
}

.subtitle {
  font-size: var(--font-size-xs);
  color: var(--muted);
  @include text-ellipsis;
}

.state {
  font: 600 0.625rem $font-family-mono;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--muted);
}

.pending {
  color: var(--warning);
}

.downloading {
  color: var(--accent);
}

.complete {
  color: var(--success);
}

.failed {
  color: var(--danger);
}

.size,
.percent {
  font: 500 var(--font-size-xs) $font-family-mono;
  color: var(--muted);
  text-align: right;
  white-space: nowrap;
}

.progress {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 34px;
  align-items: center;
  gap: 6px;
}

.bar {
  height: 4px;
  overflow: hidden;
  background: var(--line);
}

.fill {
  height: 100%;
  background: var(--accent);
  transition: width var(--transition-base);
}

.remove {
  @include flex-center;
  width: 26px;
  height: 26px;
  color: var(--muted);

  &:hover {
    color: var(--danger);
  }
}

@media (max-width: $mobile-breakpoint) {
  .row {
    grid-template-columns: 40px minmax(0, 1fr) 76px 34px;
    row-gap: 4px;
    padding: 8px 4px 8px 10px;
  }

  .art,
  .placeholder {
    width: 40px;
    height: 40px;
  }

  .art {
    grid-row: span 2;
  }

  .info {
    grid-column: 2 / 3;
  }

  .state {
    grid-column: 3;
    grid-row: 1;
    text-align: right;
  }

  .remove {
    grid-column: 4;
    grid-row: 1 / 3;
    width: 34px;
    height: 44px;
  }

  .progress {
    grid-column: 2;
    grid-row: 2;
  }

  .size {
    grid-column: 3;
    grid-row: 2;
  }
}
</style>
