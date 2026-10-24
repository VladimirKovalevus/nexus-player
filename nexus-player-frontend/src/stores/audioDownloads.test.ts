import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { i18n } from '@/i18n';
import ru from '@/i18n/locales/ru.json';
import en from '@/i18n/locales/en.json';
import appSource from '@/App.vue?raw';
import audioSource from './audio.ts?raw';
import { useAudioStore } from './audio';
import { useNotificationStore } from './notifications';

vi.mock('@/utils/providerAuth', () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
  requestProviderAuth: vi.fn(() => Promise.resolve()),
}));

type Messages = { downloads: Record<string, string> };
const LOCALES = [
  ['ru', ru as unknown as Messages],
  ['en', en as unknown as Messages],
] as const;

const fill = (tpl: string, title: string) => tpl.replace('{title}', title);

beforeEach(() => {
  vi.useFakeTimers();
  setActivePinia(createPinia());
});

afterEach(() => {
  i18n.global.locale.value = 'en';
  vi.useRealTimers();
});

describe.each(LOCALES)('cache download toasts in %s', (locale, messages) => {
  beforeEach(() => {
    i18n.global.locale.value = locale;
  });

  it('DownloadStarted adds a success notification', () => {
    const audio = useAudioStore();
    const notes = useNotificationStore();
    audio.handleAudioEvent({ type: 'DownloadStarted', payload: { title: 'Song' } } as never);
    expect(notes.notifications).toHaveLength(1);
    expect(notes.notifications[0].type).toBe('success');
    expect(notes.notifications[0].message).toBe(fill(messages.downloads.cacheStarted, 'Song'));
    expect(notes.notifications[0].message).toContain('Song');
  });

  it('DownloadStarted without a title adds nothing', () => {
    const notes = useNotificationStore();
    useAudioStore().handleAudioEvent({ type: 'DownloadStarted', payload: {} } as never);
    expect(notes.notifications).toHaveLength(0);
  });

  it('DownloadFinished success notifies and records lastCachedTrack', () => {
    const audio = useAudioStore();
    const notes = useNotificationStore();
    audio.handleAudioEvent({
      type: 'DownloadFinished',
      payload: { trackId: 7, title: 'Song', success: true },
    } as never);
    expect(notes.notifications).toHaveLength(1);
    expect(notes.notifications[0].type).toBe('success');
    expect(notes.notifications[0].message).toBe(fill(messages.downloads.cacheFinished, 'Song'));
    expect(audio.lastCachedTrack).toEqual({ trackId: 7, success: true });
  });

  it('DownloadFinished failure adds an error notification', () => {
    const audio = useAudioStore();
    const notes = useNotificationStore();
    audio.handleAudioEvent({
      type: 'DownloadFinished',
      payload: { trackId: 7, title: 'Song', success: false },
    } as never);
    expect(notes.notifications).toHaveLength(1);
    expect(notes.notifications[0].type).toBe('error');
    expect(notes.notifications[0].message).toBe(fill(messages.downloads.cacheFailed, 'Song'));
    expect(audio.lastCachedTrack).toEqual({ trackId: 7, success: false });
  });
});

describe('ru and en toasts differ', () => {
  it.each(['cacheStarted', 'cacheFinished', 'cacheFailed'])('%s', (key) => {
    expect((ru as unknown as Messages).downloads[key]).not.toBe(
      (en as unknown as Messages).downloads[key],
    );
  });
});

describe('dead listeners and keys removed', () => {
  it.each(['download-started', 'download-finished', 'download-start', 'download-finish'])(
    'App.vue has no %s',
    (name) => {
      expect(appSource).not.toContain(name);
    },
  );

  it.each(['Download started', 'Download finished', 'Download failed'])(
    'audio.ts has no %s literal',
    (literal) => {
      expect(audioSource).not.toContain(literal);
    },
  );

  it.each(LOCALES)('%s locale keys', (_l, messages) => {
    for (const key of ['started', 'finished', 'failed']) {
      expect(messages.downloads).not.toHaveProperty(key);
    }
    for (const key of ['cacheStarted', 'cacheFinished', 'cacheFailed']) {
      expect(messages.downloads[key]).toContain('{title}');
    }
  });
});
