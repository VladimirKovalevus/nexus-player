import { computed, reactive, type ComputedRef, type MaybeRefOrGetter, toValue } from 'vue';
import { convertFileSrc } from '@tauri-apps/api/core';
import { invoke } from '@/utils/providerAuth';
import { listen } from '@tauri-apps/api/event';

export type ArtworkVariant = 'thumb' | 'large';

interface ResolvedArtworkDto {
  url: string;
  path: string | null;
}

/**
 * Remote cover URL -> local asset URL, once the backend has downloaded and
 * re-encoded it. Module-level, so every card showing the same cover shares one
 * lookup and one resolution.
 */
const resolved = reactive(new Map<string, string>());

/** Keys already asked about, so scrolling back over a row costs nothing. */
const requested = new Set<string>();

/** Queued for the next flush. */
let batch: { url: string; variant: ArtworkVariant }[] = [];
let flushHandle: number | undefined;

/**
 * Coalescing window. A virtualized list rebuilds every visible row on each
 * scroll pass, so resolving per row would fire dozens of IPC calls per frame —
 * one stall traded for another. One frame's worth of requests goes out together.
 */
const FLUSH_MS = 16;

function cacheKey(url: string, variant: ArtworkVariant): string {
  return `${variant}:${url}`;
}

async function flush(): Promise<void> {
  flushHandle = undefined;

  const pending = batch;
  batch = [];
  if (pending.length === 0) return;

  // One call per variant: the command resolves a single variant at a time.
  const byVariant = new Map<ArtworkVariant, string[]>();
  for (const { url, variant } of pending) {
    const urls = byVariant.get(variant);
    if (urls) urls.push(url);
    else byVariant.set(variant, [url]);
  }

  await Promise.all(
    [...byVariant].map(async ([variant, urls]) => {
      try {
        const results = await invoke<ResolvedArtworkDto[]>('resolve_artwork', { urls, variant });
        for (const { url, path } of results) {
          if (path) resolved.set(cacheKey(url, variant), convertFileSrc(path));
        }
      } catch (e) {
        // A failed resolve just means the remote URL keeps being used.
        console.warn('resolve_artwork failed', e);
        for (const url of urls) requested.delete(cacheKey(url, variant));
      }
    }),
  );
}

function request(url: string, variant: ArtworkVariant): void {
  const key = cacheKey(url, variant);
  if (resolved.has(key) || requested.has(key)) return;

  requested.add(key);
  batch.push({ url, variant });

  if (flushHandle === undefined) {
    flushHandle = window.setTimeout(flush, FLUSH_MS);
  }
}

// The backend announces each cover as it finishes downloading, which swaps it in
// wherever it is on screen without anyone re-querying.
void listen<[string, string, string]>('artwork-cached', ({ payload }) => {
  const [url, variant, path] = payload;
  resolved.set(cacheKey(url, variant as ArtworkVariant), convertFileSrc(path));
});

/**
 * Local cached URL for `src`, or `undefined` while it is still being fetched.
 *
 * Callers fall back to the remote URL in the meantime, so the first view of a
 * cover looks exactly as it did before and silently upgrades to the local file.
 */
export function useArtwork(
  src: MaybeRefOrGetter<string | undefined | null>,
  variant: MaybeRefOrGetter<ArtworkVariant> = 'thumb',
): ComputedRef<string | undefined> {
  return computed(() => {
    const url = toValue(src);
    if (!url) return undefined;

    const v = toValue(variant);
    // Already a local URL (a cover picked from disk) — nothing to resolve.
    if (url.startsWith('asset:') || url.startsWith('data:') || url.startsWith('blob:')) {
      return url;
    }

    const hit = resolved.get(cacheKey(url, v));
    if (hit) return hit;

    request(url, v);
    return undefined;
  });
}

/** Drop the on-disk cache and every in-memory resolution. */
export async function clearArtworkCache(): Promise<void> {
  await invoke('clear_artwork_cache');
  resolved.clear();
  requested.clear();
}
