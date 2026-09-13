<script setup lang="ts">
import { errorMessage } from '@/utils/errorMessage';
import type { SearchResult } from '@/types/types';
import { ref, inject, nextTick, onMounted, onUnmounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@/utils/providerAuth';
import { useAudioStore } from '../stores/audio';
import { useStreamingStore } from '../stores/streaming';
import { fuzzyScore, fuzzyFilter } from '@/utils/fuzzy';

// ════════════════════════════════════════════════════════════════════
// Trie
// ════════════════════════════════════════════════════════════════════

class TrieNode { ch = new Map<string, TrieNode>(); end = false; }

class Trie {
  private root = new TrieNode();
  private words: string[] = [];

  insert(w: string) {
    this.words.push(w);
    let n = this.root;
    for (const c of w.toLowerCase()) {
      if (!n.ch.has(c)) n.ch.set(c, new TrieNode());
      n = n.ch.get(c)!;
    }
    n.end = true;
  }

  all() { return [...this.words]; }
  fuzzy(q: string) { return fuzzyFilter(q, this.words); }
}

const mkTrie = (...ws: string[]): Trie => {
  const t = new Trie(); ws.forEach(w => t.insert(w)); return t;
};

// ════════════════════════════════════════════════════════════════════
// Input Parsing
// ════════════════════════════════════════════════════════════════════

interface Tokenized {
  complete: string[];   // fully-typed tokens (lowercased)
  orig: string[];       // same, original case
  partial: string;      // token being typed right now (lowercased)
  partialOrig: string;  // same, original case
  inQ: boolean;         // cursor is inside single quotes
  trailing: boolean;    // ends with space (ready for next token)
}

function tokenize(s: string): Tokenized {
  const complete: string[] = [], orig: string[] = [];
  let cur = '', curO = '', inQ = false;
  for (const ch of s.trimStart()) {
    if (ch === "'") { inQ = !inQ; continue; }
    if (ch === ' ' && !inQ) {
      if (cur) { complete.push(cur.toLowerCase()); orig.push(curO); cur = ''; curO = ''; }
      continue;
    }
    cur += ch.toLowerCase(); curO += ch;
  }
  return {
    complete, orig,
    partial: cur.toLowerCase(), partialOrig: curO,
    inQ,
    trailing: !inQ && !cur && s.trimStart().length > 0,
  };
}

function splitQuery(raw: string): { args: string; query: string | null } {
  const i = raw.search(/ :/);
  return i >= 0
    ? { args: raw.slice(0, i), query: raw.slice(i + 2).trim() || null }
    : { args: raw, query: null };
}

// Extract original-case quoted or plain arg from start of string
function shiftArg(s: string): { val: string; rest: string } | null {
  s = s.trimStart();
  if (!s) return null;
  if (s[0] === "'") {
    const e = s.indexOf("'", 1);
    return e < 0 ? { val: s.slice(1), rest: '' } : { val: s.slice(1, e), rest: s.slice(e + 1).trimStart() };
  }
  const sp = s.indexOf(' ');
  return sp < 0 ? { val: s, rest: '' } : { val: s.slice(0, sp), rest: s.slice(sp + 1).trimStart() };
}

// ════════════════════════════════════════════════════════════════════
// Command Registry Types
// ════════════════════════════════════════════════════════════════════

type Completer = () => Promise<string[]> | string[];
// tokens = positional args (lowercased) after the command path
// raw    = full original input line
// query  = inline query after ' :' separator (original case)
type ExecFn = (tokens: string[], raw: string, query: string | null) => Promise<void>;

interface ArgDef {
  name: string;
  complete: Completer;
  quoted?: boolean;  // wrap completions in single quotes
}

interface CmdSpec {
  path: string[];      // routing path (e.g. ['play', 'playlist'])
  args?: ArgDef[];     // positional arg specs after path
  exec: ExecFn;
}

// ════════════════════════════════════════════════════════════════════
// UI State  (declared before exec fns so they can close over them)
// ════════════════════════════════════════════════════════════════════

const { t } = useI18n();

const isOpen      = ref(false);
const input       = ref('');
const feedback    = ref('');
const isError     = ref(false);
const inputRef    = ref<HTMLInputElement | null>(null);
const tabMatches  = ref<string[]>([]);
const tabIndex    = ref(-1);
const tabPrefix   = ref('');
const tabQuoted   = ref(false);
let feedbackTimer: ReturnType<typeof setTimeout> | null = null;

const showFeedback = (msg: string, error = false, ms = 800) => {
  feedback.value = msg; isError.value = error;
  if (feedbackTimer) clearTimeout(feedbackTimer);
  feedbackTimer = setTimeout(close, ms);
};

// ════════════════════════════════════════════════════════════════════
// Stores + Inject
// ════════════════════════════════════════════════════════════════════

const audioStore        = useAudioStore();
const streamingStore    = useStreamingStore();
const changeView        = inject<(v: string) => void>('changeView');
const toggleQueue       = inject<() => void>('toggleQueue');
const openQueuePlaylist = inject<() => void>('openCreatePlaylistModal');
const clearQueue        = inject<() => void>('clearQueue');

// ════════════════════════════════════════════════════════════════════
// Data Fetchers
// ════════════════════════════════════════════════════════════════════

interface PlaylistMeta { id: number; title: string; }
const cachedPlaylists = ref<PlaylistMeta[]>([]);

const fetchPlaylists = async (): Promise<PlaylistMeta[]> => {
  if (cachedPlaylists.value.length) return cachedPlaylists.value;
  const list = await invoke<PlaylistMeta[]>('get_all_playlists');
  cachedPlaylists.value = list;
  return list;
};

const playlistTitleCompleter: Completer = async () =>
  (await fetchPlaylists()).map(p => p.title);

// ════════════════════════════════════════════════════════════════════
// Static Completion Sets
// ════════════════════════════════════════════════════════════════════

const VIEWS     = ['home', 'search', 'library', 'radio', 'history', 'cache', 'downloads', 'settings'];
const PROVIDERS = ['bandcamp', 'soundcloud'];
const CTYPES    = ['all', 'track', 'playlist', 'album', 'channel'];

// ════════════════════════════════════════════════════════════════════
// Execute Functions
// ════════════════════════════════════════════════════════════════════

const execNext: ExecFn = async ([countStr]) => {
  const n = countStr ? parseInt(countStr) : 1;
  if (isNaN(n) || n < 1) { showFeedback(t('cmd.badCount', { cmd: 'next' }), true, 1500); return; }
  if (n === 1) { await invoke('play_next'); showFeedback(t('cmd.nextTrack')); }
  else {
    const idx = audioStore.currentIndex;
    if (idx === null) { showFeedback(t('cmd.noActiveTrack'), true); return; }
    await invoke('play_at_index', { index: idx + n });
    showFeedback(t('cmd.skippedForward', { n }));
  }
};

const execPrev: ExecFn = async ([countStr]) => {
  const n = countStr ? parseInt(countStr) : 1;
  if (isNaN(n) || n < 1) { showFeedback(t('cmd.badCount', { cmd: 'previous' }), true, 1500); return; }
  if (n === 1) { await invoke('play_previous'); showFeedback(t('cmd.previousTrack')); }
  else {
    const idx = audioStore.currentIndex;
    if (idx === null) { showFeedback(t('cmd.noActiveTrack'), true); return; }
    await invoke('play_at_index', { index: Math.max(0, idx - n) });
    showFeedback(t('cmd.skippedBack', { n }));
  }
};

const execPlay: ExecFn = async ([idxStr]) => {
  if (idxStr) {
    const n = parseInt(idxStr);
    if (isNaN(n) || n < 1) { showFeedback(t('cmd.badIndex', { cmd: 'play' }), true, 1500); return; }
    await invoke('play_at_index', { index: n - 1 });
    showFeedback(t('cmd.playingIndex', { n }));
  } else if (audioStore.isPaused) {
    await invoke('resume'); showFeedback(t('cmd.playing'));
  } else {
    await invoke('play_at_index', { index: 0 }); showFeedback(t('cmd.playing'));
  }
};

const execPlayPlaylist: ExecFn = async (_, raw) => {
  const afterPath = raw.replace(/^play\s+playlist\s+/i, '');
  const titleArg = shiftArg(afterPath);
  if (!titleArg?.val) { showFeedback(t('cmd.playlistTitleRequired'), true, 1500); return; }
  const startN = titleArg.rest ? (parseInt(titleArg.rest) || 1) : 1;
  const playlists = await fetchPlaylists();
  const tq = titleArg.val.toLowerCase();
  const match = playlists.find(p => p.title.toLowerCase() === tq)
    ?? fuzzyFilter(tq, playlists.map(p => p.title)).map(t => playlists.find(p => p.title === t)!)[0];
  if (!match) { showFeedback(t('cmd.playlistNotFound', { title: titleArg.val }), true, 1500); return; }
  await invoke('load_playlist_and_play', { playlistId: match.id, startIndex: startN - 1 });
  showFeedback(`▶ ${match.title}${startN > 1 ? ` #${startN}` : ''}`);
};

const execStop: ExecFn = async () => {
  await invoke('pause'); showFeedback(t('cmd.stopped'));
};

const execSwitch: ExecFn = async ([viewArg]) => {
  if (!viewArg) { showFeedback(t('cmd.viewRequired'), true, 1500); return; }
  const view = fuzzyFilter(viewArg, VIEWS)[0];
  if (!view) { showFeedback(t('cmd.unknownView', { view: viewArg }), true, 1500); return; }
  changeView?.(view); showFeedback(`→ ${view}`);
};

const execQueueToggle:   ExecFn = async () => { toggleQueue?.();       showFeedback(t('cmd.queueToggled')); };
const execQueuePlaylist: ExecFn = async () => { openQueuePlaylist?.(); showFeedback(t('cmd.queuePlaylist')); };
const execQueueClear:    ExecFn = async () => { clearQueue?.();        showFeedback(t('cmd.queueCleared')); };

const execSearchPlay: ExecFn = async ([idxStr]) => {
  const n = idxStr ? parseInt(idxStr) : 1;
  if (isNaN(n) || n < 1) { showFeedback(t('cmd.badIndex', { cmd: 'search play' }), true, 1500); return; }
  const results = streamingStore.searchResults;
  const result  = results[n - 1];
  if (!result) { showFeedback(t('cmd.noResult', { n, total: results.length }), true, 1500); return; }
  if (result.type === 'channel') { showFeedback(t('search.channelUnsupported'), true, 1500); return; }
  const ok = await audioStore.searchAndPlay({
    system: streamingStore.activeProvider,
    contentType: streamingStore.activeContentType?.toLowerCase() ?? null,
    query: streamingStore.activeQuery,
    limit: streamingStore.limit,
    offset: streamingStore.offset,
    selectedIndex: n - 1,
    resultId: result.type === 'track' ? result.source?.url : result.id,
  });
  if (!ok) return;
  showFeedback(t('cmd.queued', { title: result.title }));
};

const execSearch: ExecFn = async ([provArg, typeArg, limitArg, pageArg], _, query) => {
  if (!provArg) { showFeedback(t('cmd.providerRequired'), true, 1800); return; }
  const provider = fuzzyFilter(provArg, PROVIDERS)[0];
  if (!provider) { showFeedback(t('cmd.unknownProvider', { value: provArg }), true, 1500); return; }
  const ctRaw    = typeArg ? fuzzyFilter(typeArg, CTYPES)[0] : null;
  if (typeArg && !ctRaw) { showFeedback(t('cmd.unknownType', { value: typeArg }), true, 1500); return; }
  const limit = limitArg ? parseInt(limitArg) : 20;
  const page  = pageArg  ? parseInt(pageArg)  : 0;
  if (limitArg && (isNaN(limit) || limit < 1)) { showFeedback(t('cmd.badLimit'), true, 1500); return; }
  if (pageArg  && (isNaN(page)  || page  < 0)) { showFeedback(t('cmd.badPage'), true, 1500); return; }
  const norm = ctRaw === 'all' ? null : ctRaw;
  Object.assign(streamingStore, {
    selectedProvider: provider, activeProvider: provider,
    selectedContentType: norm, activeContentType: norm,
    limit, offset: page * limit,
  });
  changeView?.('search');
  if (query) {
    streamingStore.query = query; streamingStore.activeQuery = query; streamingStore.searchResults = [];
    const results = await invoke<SearchResult[]>('search_streaming', {
      provider, contentType: norm?.toLowerCase() ?? null, query, limit, offset: page * limit,
    });
    streamingStore.searchResults = results;
    showFeedback(`"${query}" → ${provider}`);
  } else {
    showFeedback(`${provider}${norm ? ` · ${norm}` : ''} · ${t('cmd.limitPage', { limit, page: page + 1 })}`);
  }
};

// ════════════════════════════════════════════════════════════════════
// Command Specs  (the single source of truth)
// ════════════════════════════════════════════════════════════════════

const SPECS: CmdSpec[] = [
  { path: ['next'],             args: [{ name: 'count',    complete: () => []        }], exec: execNext          },
  { path: ['previous'],         args: [{ name: 'count',    complete: () => []        }], exec: execPrev          },
  { path: ['play'],             args: [{ name: 'index',    complete: () => []        }], exec: execPlay          },
  { path: ['play', 'playlist'], args: [
      { name: 'title', complete: playlistTitleCompleter, quoted: true },
      { name: 'index', complete: () => []                             },
    ],                                                                                    exec: execPlayPlaylist  },
  { path: ['stop'],                                                                       exec: execStop          },
  { path: ['switch'],           args: [{ name: 'view',     complete: () => VIEWS     }], exec: execSwitch        },
  { path: ['queue', 'toggle'],                                                            exec: execQueueToggle   },
  { path: ['queue', 'playlist'],                                                          exec: execQueuePlaylist },
  { path: ['queue', 'clear'],                                                             exec: execQueueClear    },
  { path: ['search', 'play'],   args: [{ name: 'index',    complete: () => []        }], exec: execSearchPlay    },
  { path: ['search'],           args: [
      { name: 'provider', complete: () => PROVIDERS },
      { name: 'type',     complete: () => CTYPES    },
      { name: 'limit',    complete: () => []        },
      { name: 'page',     complete: () => []        },
    ],                                                                                    exec: execSearch        },
];

const ROOT_TRIE = mkTrie(...new Set(SPECS.map(s => s.path[0])));

// ════════════════════════════════════════════════════════════════════
// Completion Engine
// ════════════════════════════════════════════════════════════════════

// How many leading tokens of 'complete' match this spec's path (greedily)
function pathMatchLen(spec: CmdSpec, complete: string[]): number {
  let n = 0;
  for (let i = 0; i < Math.min(spec.path.length, complete.length); i++) {
    if (fuzzyScore(complete[i], spec.path[i]) !== null) n++;
    else break;
  }
  return n;
}

async function getCompletions(inputStr: string): Promise<{
  prefix: string; candidates: string[]; quoted: boolean;
}> {
  const { args: argsStr } = splitQuery(inputStr);
  const { complete, orig, partial } = tokenize(argsStr);

  // ── No confirmed tokens: completing command name ──
  if (complete.length === 0) {
    return { prefix: '', candidates: ROOT_TRIE.fuzzy(partial), quoted: false };
  }

  // ── Find all specs and how deep their path matches ──
  const specDepths = SPECS.map(s => ({ s, d: pathMatchLen(s, complete) }));

  // Specs with path fully consumed by 'complete' (d == path.length <= complete.length)
  const fullMatch = specDepths.filter(x => x.d === x.s.path.length && x.s.path.length <= complete.length);

  // Specs whose path is only partially consumed (d == complete.length < path.length)
  const partialMatch = specDepths.filter(x => x.d === complete.length && complete.length < x.s.path.length);

  // ── Next path token candidates (from partial-match specs) ──
  const nextPathTokens = [...new Set(partialMatch.map(x => x.s.path[complete.length]))];

  // ── Arg candidates (from the best full-match spec) ──
  // Pick the spec with longest path (most specific)
  const bestFull = fullMatch.sort((a, b) => b.s.path.length - a.s.path.length)[0];

  let argCandidates: string[] = [];
  let argPrefix    = '';
  let argQuoted    = false;

  if (bestFull) {
    const argTokens = complete.slice(bestFull.s.path.length); // confirmed arg tokens
    const origArgs  = orig.slice(bestFull.s.path.length);
    const argPos    = argTokens.length; // next arg to complete (partial or trailing)
    const argDef    = bestFull.s.args?.[argPos];

    if (argDef) {
      argCandidates = await argDef.complete();
      argQuoted     = !!argDef.quoted;

      // Build prefix: path + confirmed args (quoting where needed)
      let ap = '';
      for (let i = 0; i < argTokens.length; i++) {
        const d = bestFull.s.args?.[i];
        ap += ' ' + (d?.quoted ? `'${origArgs[i]}'` : origArgs[i]);
      }
      argPrefix = bestFull.s.path.join(' ') + ap + ' ';
    }
  }

  // ── Combine path-routing tokens and arg-value candidates ──
  const allCandidates = [...nextPathTokens, ...argCandidates];

  if (!allCandidates.length) return { prefix: inputStr, candidates: [], quoted: false };

  // Prefixes are equal in the mixed case (both resolve to same confirmed tokens + space)
  const prefix = argPrefix || (complete.join(' ') + ' ');
  const trie   = mkTrie(...allCandidates);

  return {
    prefix,
    candidates: trie.fuzzy(partial),
    // Only wrap in quotes if completing a quoted arg and no path tokens mixed in
    quoted: nextPathTokens.length === 0 && argQuoted,
  };
}

// ════════════════════════════════════════════════════════════════════
// Execute Engine
// ════════════════════════════════════════════════════════════════════

function resolveSpec(complete: string[]): { spec: CmdSpec; argTokens: string[] } | null {
  // Find spec whose full path is matched by the leading tokens of complete
  const candidates = SPECS
    .map(s => ({ s, d: pathMatchLen(s, complete) }))
    .filter(x => x.d === x.s.path.length && x.s.path.length <= complete.length)
    .sort((a, b) => b.s.path.length - a.s.path.length); // prefer most specific

  if (!candidates.length) return null;
  const { s } = candidates[0];
  return { spec: s, argTokens: complete.slice(s.path.length) };
}

// ════════════════════════════════════════════════════════════════════
// Open / Close / Reset
// ════════════════════════════════════════════════════════════════════

const isTauriEnv = () => !!(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;

const resetTab = () => { tabMatches.value = []; tabIndex.value = -1; tabPrefix.value = ''; tabQuoted.value = false; };

// ════════════════════════════════════════════════════════════════════
// History
// ════════════════════════════════════════════════════════════════════

const history     = ref<string[]>([]);
const historyIdx  = ref(-1);   // -1 = not browsing
const historySaved = ref('');  // input saved before browsing started

const pushHistory = (cmd: string) => {
  const trimmed = cmd.trim();
  if (!trimmed) return;
  // Deduplicate: remove previous occurrence of same command
  const existing = history.value.indexOf(trimmed);
  if (existing !== -1) history.value.splice(existing, 1);
  history.value.push(trimmed);
};

const resetHistory = () => { historyIdx.value = -1; historySaved.value = ''; };

const navigateHistory = (dir: 'up' | 'down') => {
  const prefix = historyIdx.value === -1 ? input.value : historySaved.value;
  const filtered = history.value.filter(h => h.startsWith(prefix));

  if (!filtered.length) return;

  if (dir === 'up') {
    if (historyIdx.value === -1) {
      // Start browsing: save current input, go to most recent match
      historySaved.value = input.value;
      historyIdx.value   = filtered.length - 1;
    } else {
      historyIdx.value = Math.max(0, historyIdx.value - 1);
    }
    input.value = filtered[historyIdx.value];
  } else {
    if (historyIdx.value === -1) return;
    if (historyIdx.value >= filtered.length - 1) {
      // Past the end: restore saved input
      input.value = historySaved.value;
      resetHistory();
    } else {
      historyIdx.value++;
      input.value = filtered[historyIdx.value];
    }
  }
};

const open = async () => {
  isOpen.value = true; input.value = ''; feedback.value = ''; isError.value = false;
  cachedPlaylists.value = [];
  resetTab();
  resetHistory();
  await nextTick();
  inputRef.value?.focus();
};

const close = () => {
  isOpen.value = false; input.value = ''; feedback.value = ''; isError.value = false;
  resetTab();
  resetHistory();
  if (feedbackTimer) { clearTimeout(feedbackTimer); feedbackTimer = null; }
};

// ════════════════════════════════════════════════════════════════════
// Tab Handler
// ════════════════════════════════════════════════════════════════════

const handleTab = async (reverse = false) => {
  if (tabMatches.value.length === 0) {
    // First tab press: compute completions
    const { prefix, candidates, quoted } = await getCompletions(input.value);
    if (!candidates.length) return;
    tabMatches.value  = candidates;
    tabPrefix.value   = prefix;
    tabQuoted.value   = quoted;
    tabIndex.value    = reverse ? candidates.length - 1 : 0;
  } else {
    // Subsequent presses: cycle
    tabIndex.value = (tabIndex.value + (reverse ? -1 : 1) + tabMatches.value.length) % tabMatches.value.length;
  }

  const choice = tabMatches.value[tabIndex.value];
  input.value  = tabPrefix.value + (tabQuoted.value ? `'${choice}'` : choice);
};

// ════════════════════════════════════════════════════════════════════
// Execute Handler
// ════════════════════════════════════════════════════════════════════

const execute = async () => {
  const raw = input.value.trim();
  if (!raw) { close(); return; }

  const { args: argsStr, query } = splitQuery(raw);
  const { complete, partial } = tokenize(argsStr);
  // Include partial as the last token (Enter confirms it even without trailing space)
  const allTokens = partial ? [...complete, partial] : complete;

  if (!allTokens.length) { close(); return; }

  const resolved = resolveSpec(allTokens);
  if (!resolved) {
    showFeedback(t('cmd.unknownCommand', { cmd: allTokens[0] }), true, 1500);
    return;
  }

  try {
    await resolved.spec.exec(resolved.argTokens, raw, query);
    pushHistory(raw);
  } catch (e) {
    showFeedback(t('common.error', { error: errorMessage(e) }), true, 2000);
  }
};

// ════════════════════════════════════════════════════════════════════
// Keyboard Handler
// ════════════════════════════════════════════════════════════════════

const onGlobalKeydown = async (e: KeyboardEvent) => {
  if (!isTauriEnv()) return;

  if (!isOpen.value) {
    const t = e.target as HTMLElement;
    if (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable) return;
    if (e.key === ':' && !e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault(); await open();
    }
    return;
  }

  if (e.key === 'Escape') {
    e.preventDefault(); close();
  } else if (e.key === 'Tab') {
    e.preventDefault(); await handleTab(e.shiftKey);
  } else if (e.key === 'Enter') {
    e.preventDefault(); await execute();
  } else if (e.key === 'ArrowUp') {
    e.preventDefault(); resetTab(); navigateHistory('up');
  } else if (e.key === 'ArrowDown') {
    e.preventDefault(); resetTab(); navigateHistory('down');
  } else if (e.key.length === 1) {
    resetTab();
    resetHistory();
  }
};

onMounted(() => window.addEventListener('keydown', onGlobalKeydown));
onUnmounted(() => { window.removeEventListener('keydown', onGlobalKeydown); if (feedbackTimer) clearTimeout(feedbackTimer); });
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
          :placeholder="$t('cmd.placeholder')"
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

.cmdline-bar,
.cmdline-completions {
  pointer-events: auto;
  width: 100%;
  background: var(--screen);
  font-family: $font-family-mono;
}

.cmdline-completions {
  display: flex;
  flex-wrap: wrap;
  gap: 3px;
  max-height: 30vh;
  overflow-y: auto;
  padding: 5px 12px;
  border-top: 1px solid var(--screen-track);
}

.cmdline-completion {
  padding: 2px 7px;
  font-size: var(--font-size-xs);
  color: var(--screen-muted);
  border-radius: var(--radius-sm);

  &.active {
    background: var(--led);
    color: var(--on-accent);
    font-weight: 600;
  }
}

.cmdline-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 38px;
  padding: 0 12px calc(env(safe-area-inset-bottom));
  border-top: 2px solid var(--led);
  font-size: var(--font-size-base);
  box-shadow: var(--shadow-lg);
}

.cmdline-prompt {
  flex: none;
  color: var(--led);
  font-weight: 700;
  user-select: none;
}

.cmdline-input {
  flex: 1;
  min-width: 0;
  padding: 0;
  background: transparent;
  border: 0;
  border-radius: 0;
  outline: none;
  color: var(--led);
  font-family: inherit;
  font-size: inherit;
  caret-color: var(--led);

  &::placeholder {
    color: var(--screen-muted);
  }
}

.cmdline-feedback {
  flex: none;
  max-width: 60%;
  overflow: hidden;
  padding-left: 8px;
  border-left: 1px solid var(--screen-track);
  font-size: var(--font-size-xs);
  color: var(--screen-text);
  text-overflow: ellipsis;
  white-space: nowrap;

  &.error {
    color: var(--led-hot);
  }
}

.cmdline-enter-active,
.cmdline-leave-active,
.completions-enter-active,
.completions-leave-active {
  transition: opacity var(--transition-fast), transform var(--transition-fast);
}

.cmdline-enter-from,
.cmdline-leave-to {
  opacity: 0;
  transform: translateY(8px);
}

.completions-enter-from,
.completions-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.feedback-enter-active,
.feedback-leave-active {
  transition: opacity var(--transition-base);
}

.feedback-enter-from,
.feedback-leave-to {
  opacity: 0;
}
</style>
