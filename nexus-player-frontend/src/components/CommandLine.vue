<script setup lang="ts">
import { ref, inject, nextTick, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useAudioStore } from '../stores/audio';
import { useStreamingStore } from '../stores/streaming';

const audioStore = useAudioStore();
const streamingStore = useStreamingStore();
const changeView = inject<(view: string) => void>('changeView');
const toggleQueue = inject<() => void>('toggleQueue');
const openCreatePlaylistModal = inject<() => void>('openCreatePlaylistModal');
const clearQueue = inject<() => void>('clearQueue');

const ALL_COMMANDS = ['next', 'previous', 'play', 'stop', 'switch', 'queue', 'search'];
const ALL_VIEWS = ['home', 'search', 'vk', 'history', 'downloads'];
const ALL_QUEUE_SUBCOMMANDS = ['toggle', 'playlist', 'clear'];
const ALL_PROVIDERS = ['bandcamp', 'soundcloud', 'vk'];
const ALL_SEARCH_SUBCOMMANDS = ['play', ...ALL_PROVIDERS];
const ALL_CONTENT_TYPES = ['all', 'track', 'playlist', 'album', 'channel'];

const isOpen = ref(false);
const input = ref('');
const feedback = ref('');
const isError = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);
const tabMatches = ref<string[]>([]);
const tabIndex = ref(-1);
let feedbackTimer: ReturnType<typeof setTimeout> | null = null;

const isTauriEnv = () => !!(window as any).__TAURI_INTERNALS__;

const resetTab = () => {
  tabMatches.value = [];
  tabIndex.value = -1;
};

const open = async () => {
  isOpen.value = true;
  input.value = '';
  feedback.value = '';
  isError.value = false;
  resetTab();
  await nextTick();
  inputRef.value?.focus();
};

const close = () => {
  isOpen.value = false;
  input.value = '';
  feedback.value = '';
  isError.value = false;
  resetTab();
  if (feedbackTimer) {
    clearTimeout(feedbackTimer);
    feedbackTimer = null;
  }
};

const handleTab = () => {
  const raw = input.value.toLowerCase();
  const parts = raw.trimStart().split(/\s+/);

  // Completing a switch argument: "switch <partial>"
  if (parts[0] === 'switch' && (parts.length > 1 || raw.endsWith(' '))) {
    const argPrefix = parts[1] ?? '';
    if (tabMatches.value.length === 0 || tabIndex.value === -1) {
      const matches = argPrefix
        ? ALL_VIEWS.filter(v => v.startsWith(argPrefix))
        : ALL_VIEWS;
      if (matches.length === 0) return;
      tabMatches.value = matches;
      tabIndex.value = 0;
    } else {
      tabIndex.value = (tabIndex.value + 1) % tabMatches.value.length;
    }
    input.value = `switch ${tabMatches.value[tabIndex.value]}`;
    return;
  }

  // Completing search subcommand/provider (pos 1) or content type (pos 2)
  if (parts[0] === 'search' && (parts.length > 1 || raw.endsWith(' '))) {
    const firstArg = raw.endsWith(' ') ? '' : (parts[1] ?? '');
    const firstArgResolved = parts.length >= 2 ? parts[1] : '';
    const atPos2 = (parts.length > 2) || (parts.length === 2 && raw.endsWith(' ') && ALL_PROVIDERS.includes(firstArgResolved));

    // Content type completion only for real providers (not 'play')
    if (atPos2 && ALL_PROVIDERS.includes(firstArgResolved)) {
      const argPrefix = raw.endsWith(' ') ? '' : (parts[2] ?? '');
      if (tabMatches.value.length === 0 || tabIndex.value === -1) {
        const matches = argPrefix
          ? ALL_CONTENT_TYPES.filter(t => t.startsWith(argPrefix))
          : ALL_CONTENT_TYPES;
        if (matches.length === 0) return;
        tabMatches.value = matches;
        tabIndex.value = 0;
      } else {
        tabIndex.value = (tabIndex.value + 1) % tabMatches.value.length;
      }
      input.value = `search ${firstArgResolved} ${tabMatches.value[tabIndex.value]}`;
      return;
    }

    // Provider / 'play' completion at pos 1
    if (!atPos2) {
      if (tabMatches.value.length === 0 || tabIndex.value === -1) {
        const matches = firstArg
          ? ALL_SEARCH_SUBCOMMANDS.filter(p => p.startsWith(firstArg))
          : ALL_SEARCH_SUBCOMMANDS;
        if (matches.length === 0) return;
        tabMatches.value = matches;
        tabIndex.value = 0;
      } else {
        tabIndex.value = (tabIndex.value + 1) % tabMatches.value.length;
      }
      input.value = `search ${tabMatches.value[tabIndex.value]}`;
      return;
    }
  }

  // Completing a queue subcommand: "queue <partial>"
  if (parts[0] === 'queue' && (parts.length > 1 || raw.endsWith(' '))) {
    const argPrefix = parts[1] ?? '';
    if (tabMatches.value.length === 0 || tabIndex.value === -1) {
      const matches = argPrefix
        ? ALL_QUEUE_SUBCOMMANDS.filter(s => s.startsWith(argPrefix))
        : ALL_QUEUE_SUBCOMMANDS;
      if (matches.length === 0) return;
      tabMatches.value = matches;
      tabIndex.value = 0;
    } else {
      tabIndex.value = (tabIndex.value + 1) % tabMatches.value.length;
    }
    input.value = `queue ${tabMatches.value[tabIndex.value]}`;
    return;
  }

  // Completing a command name
  if (tabMatches.value.length === 0 || tabIndex.value === -1) {
    const matches = parts[0]
      ? ALL_COMMANDS.filter(c => c.startsWith(parts[0]))
      : ALL_COMMANDS;
    if (matches.length === 0) return;
    tabMatches.value = matches;
    tabIndex.value = 0;
  } else {
    tabIndex.value = (tabIndex.value + 1) % tabMatches.value.length;
  }

  input.value = tabMatches.value[tabIndex.value];
};

const showFeedback = (msg: string, error = false, autoCloseMs = 800) => {
  feedback.value = msg;
  isError.value = error;
  if (feedbackTimer) clearTimeout(feedbackTimer);
  feedbackTimer = setTimeout(close, autoCloseMs);
};

const COMMANDS: Record<string, string> = {
  next: 'play_next',
  previous: 'play_previous',
  prev: 'play_previous',
};

const resolveCommand = (token: string): string | null => {
  if (ALL_COMMANDS.includes(token)) return token;
  const matches = ALL_COMMANDS.filter(c => c.startsWith(token));
  return matches.length === 1 ? matches[0] : null;
};

const resolveView = (token: string): string | null => {
  if (ALL_VIEWS.includes(token)) return token;
  const matches = ALL_VIEWS.filter(v => v.startsWith(token));
  return matches.length === 1 ? matches[0] : null;
};

const resolveQueueSubcommand = (token: string): string | null => {
  if (ALL_QUEUE_SUBCOMMANDS.includes(token)) return token;
  const matches = ALL_QUEUE_SUBCOMMANDS.filter(s => s.startsWith(token));
  return matches.length === 1 ? matches[0] : null;
};

const resolveProvider = (token: string): string | null => {
  if (ALL_PROVIDERS.includes(token)) return token;
  const matches = ALL_PROVIDERS.filter(p => p.startsWith(token));
  return matches.length === 1 ? matches[0] : null;
};

const resolveContentType = (token: string): string | null => {
  if (ALL_CONTENT_TYPES.includes(token)) return token;
  const matches = ALL_CONTENT_TYPES.filter(t => t.startsWith(token));
  return matches.length === 1 ? matches[0] : null;
};

const execute = async () => {
  const original = input.value.trim();
  if (!original) {
    close();
    return;
  }

  // Split on first ' :' to separate args from inline query
  const colonQueryIdx = original.search(/\s:/);
  const argsRaw = (colonQueryIdx >= 0 ? original.slice(0, colonQueryIdx) : original).toLowerCase();
  const inlineQuery = colonQueryIdx >= 0 ? original.slice(colonQueryIdx).replace(/^\s*:/, '').trim() : null;

  const parts = argsRaw.split(/\s+/);
  const cmd = resolveCommand(parts[0]);

  if (!cmd) {
    showFeedback(`неизвестная команда: ${parts[0]}`, true, 1500);
    return;
  }

  try {
    if (cmd === 'play') {
      if (audioStore.isPaused) {
        await invoke('resume');
      } else if (!audioStore.isPlaying) {
        await invoke('play_at_index', { index: 0 });
      }
      showFeedback('воспроизведение');
    } else if (cmd === 'stop') {
      await invoke('pause');
      showFeedback('остановлено');
    } else if (cmd === 'switch') {
      const viewArg = parts[1];
      if (!viewArg) {
        showFeedback('switch: укажи название страницы', true, 1500);
        return;
      }
      const view = resolveView(viewArg);
      if (!view) {
        showFeedback(`неизвестная страница: ${viewArg}`, true, 1500);
        return;
      }
      changeView?.(view);
      showFeedback(`→ ${view}`);
    } else if (cmd === 'queue') {
      const subArg = parts[1];
      if (!subArg) {
        showFeedback('queue: toggle | playlist | clear', true, 1500);
        return;
      }
      const sub = resolveQueueSubcommand(subArg);
      if (!sub) {
        showFeedback(`queue: неизвестная подкоманда: ${subArg}`, true, 1500);
        return;
      }
      if (sub === 'toggle') {
        toggleQueue?.();
        showFeedback('очередь переключена');
      } else if (sub === 'playlist') {
        openCreatePlaylistModal?.();
        showFeedback('создание плейлиста...');
      } else if (sub === 'clear') {
        clearQueue?.();
        showFeedback('очередь очищена');
      }
    } else if (cmd === 'search') {
      const providerArg = parts[1];
      if (!providerArg) {
        showFeedback('search: укажи провайдера (bandcamp | soundcloud | vk)', true, 1800);
        return;
      }

      // :search play [index] — add result by index to queue
      if (providerArg === 'play') {
        const idxArg = parts[2];
        const idx = idxArg !== undefined ? parseInt(idxArg, 10) : null;
        if (idx === null || isNaN(idx) || idx < 0) {
          showFeedback('search play: укажи индекс результата', true, 1500);
          return;
        }
        const result = streamingStore.searchResults[idx];
        if (!result) {
          showFeedback(`search play: нет результата с индексом ${idx}`, true, 1500);
          return;
        }
        try {
          await invoke('handle_search', {
            system: streamingStore.activeProvider,
            contentType: streamingStore.activeContentType?.toLowerCase() ?? null,
            query: streamingStore.activeQuery,
            limit: streamingStore.limit,
            offset: streamingStore.offset,
            selectedIndex: idx,
          });
        } catch {
          await invoke('handle_streaming_track', {
            searchResult: result,
            strategy: 'cache',
            query: streamingStore.activeQuery,
            searchType: streamingStore.activeContentType,
          });
        }
        showFeedback(`→ очередь: ${result.title ?? idx}`);
        return;
      }

      const provider = resolveProvider(providerArg);
      if (!provider) {
        showFeedback(`search: неизвестный провайдер: ${providerArg}`, true, 1500);
        return;
      }

      const typeArg = parts[2];
      const limitArg = parts[3];
      const pageArg = parts[4];

      const contentType = typeArg ? resolveContentType(typeArg) : null;
      if (typeArg && contentType === null) {
        showFeedback(`search: неизвестный тип: ${typeArg}`, true, 1500);
        return;
      }

      const limit = limitArg ? parseInt(limitArg, 10) : 20;
      const page = pageArg ? parseInt(pageArg, 10) : 0;

      if (isNaN(limit) || limit < 1) {
        showFeedback(`search: некорректный limit: ${limitArg}`, true, 1500);
        return;
      }
      if (isNaN(page) || page < 0) {
        showFeedback(`search: некорректная страница: ${pageArg}`, true, 1500);
        return;
      }

      const normalizedType = contentType === 'all' ? null : contentType;
      streamingStore.selectedProvider = provider;
      streamingStore.activeProvider = provider;
      streamingStore.selectedContentType = normalizedType;
      streamingStore.activeContentType = normalizedType;
      streamingStore.limit = limit;
      streamingStore.offset = page * limit;

      changeView?.('search');

      if (inlineQuery) {
        streamingStore.query = inlineQuery;
        streamingStore.activeQuery = inlineQuery;
        streamingStore.searchResults = [];
        await invoke('search_streaming', {
          provider,
          contentType: normalizedType?.toLowerCase() ?? null,
          query: inlineQuery,
          limit,
          offset: page * limit,
        }).then((results: any) => {
          streamingStore.searchResults = results;
        });
        showFeedback(`"${inlineQuery}" → ${provider}`);
      } else {
        const typePart = contentType && contentType !== 'all' ? ` · ${contentType}` : '';
        showFeedback(`${provider}${typePart} · limit ${limit} · стр. ${page + 1}`);
      }
    } else if (COMMANDS[cmd]) {
      await invoke(COMMANDS[cmd]);
      showFeedback(cmd === 'next' ? 'следующий трек' : 'предыдущий трек');
    }
  } catch (e) {
    showFeedback(`ошибка: ${e}`, true, 2000);
  }
};

const onGlobalKeydown = async (e: KeyboardEvent) => {
  if (!isTauriEnv()) return;

  if (!isOpen.value) {
    const target = e.target as HTMLElement;
    const tag = target.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable)
      return;
    if (e.key === ':' && !e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault();
      await open();
    }
    return;
  }

  if (e.key === 'Escape') {
    e.preventDefault();
    close();
  } else if (e.key === 'Tab') {
    e.preventDefault();
    handleTab();
  } else if (e.key === 'Enter') {
    e.preventDefault();
    await execute();
  } else if (e.key.length === 1) {
    // User typed a character — reset tab cycling
    resetTab();
  }
};

onMounted(() => {
  window.addEventListener('keydown', onGlobalKeydown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onGlobalKeydown);
  if (feedbackTimer) clearTimeout(feedbackTimer);
});
</script>

<template>
  <Transition name="cmdline">
    <div v-if="isOpen" class="cmdline-overlay" @click.self="close">
      <div class="cmdline-bar">
        <span class="cmdline-prompt">:</span>
        <input
          ref="inputRef"
          v-model="input"
          class="cmdline-input"
          type="text"
          autocomplete="off"
          autocorrect="off"
          autocapitalize="off"
          spellcheck="false"
          placeholder="команда..."
        />
        <Transition name="feedback">
          <span
            v-if="feedback"
            :class="['cmdline-feedback', { error: isError }]"
          >
            {{ feedback }}
          </span>
        </Transition>
      </div>
      <Transition name="completions">
        <div v-if="tabMatches.length > 1" class="cmdline-completions">
          <span
            v-for="(cmd, i) in tabMatches"
            :key="cmd"
            :class="['cmdline-completion', { active: i === tabIndex }]"
          >{{ cmd }}</span>
        </div>
      </Transition>
    </div>
  </Transition>
</template>

<style scoped lang="scss">
@import '@/styles/variables.scss';

.cmdline-overlay {
  position: fixed;
  inset: 0;
  z-index: $z-modal + 10;
  pointer-events: none;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
}

.cmdline-completions {
  pointer-events: auto;
  width: 100%;
  display: flex;
  gap: $spacing-xs;
  padding: $spacing-xs $spacing-md;
  background: $color-bg-secondary;
  border-bottom: 1px solid $color-border;
}

.cmdline-completion {
  padding: $spacing-xxs $spacing-sm;
  font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', $font-family-primary;
  font-size: $font-size-xs;
  color: $color-text-muted;
  border-radius: $radius-sm;

  &.active {
    background: $color-accent-primary;
    color: $color-accent-hover;
    font-weight: $font-weight-semibold;
  }
}

.cmdline-bar {
  pointer-events: auto;
  width: 100%;
  display: flex;
  align-items: center;
  gap: $spacing-xs;
  padding: $spacing-sm $spacing-md;
  background: $color-bg-player;
  border-top: 1px solid $color-accent-primary;
  font-family:
    'JetBrains Mono', 'Fira Code', 'Cascadia Code', $font-family-primary;
  font-size: $font-size-sm;
  box-shadow: $shadow-xl;
}

.cmdline-prompt {
  color: $color-accent-primary;
  font-weight: $font-weight-bold;
  font-size: $font-size-base;
  line-height: 1;
  user-select: none;
  flex-shrink: 0;
}

.cmdline-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: $color-accent-hover;
  font-family: inherit;
  font-size: inherit;
  caret-color: $color-accent-primary;
  min-width: 0;

  &::placeholder {
    color: $color-text-muted;
  }
}

.cmdline-feedback {
  flex-shrink: 0;
  font-size: $font-size-xs;
  color: $color-text-secondary;
  padding-left: $spacing-sm;
  border-left: 1px solid $color-border;

  &.error {
    color: $color-accent-primary;
  }
}

.cmdline-enter-active,
.cmdline-leave-active {
  transition:
    opacity $transition-fast,
    transform $transition-fast;
}

.cmdline-enter-from,
.cmdline-leave-to {
  opacity: 0;
  transform: translateY($spacing-sm);
}

.feedback-enter-active,
.feedback-leave-active {
  transition: opacity $transition-base;
}

.feedback-enter-from,
.feedback-leave-to {
  opacity: 0;
}

.completions-enter-active,
.completions-leave-active {
  transition: opacity $transition-fast, transform $transition-fast;
}

.completions-enter-from,
.completions-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
