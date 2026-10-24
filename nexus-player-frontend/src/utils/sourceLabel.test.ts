import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { i18n } from '@/i18n';
import MediaRow from '@/components/common/MediaRow.vue';
import historySource from '@/views/HistoryView.vue?raw';
import mediaRowSource from '@/components/common/MediaRow.vue?raw';
import playerSource from '@/components/PlayerControl.vue?raw';
import settingsSource from '@/views/SettingsView.vue?raw';
import { sourceLabel } from './sourceLabel';

vi.mock('@/utils/providerAuth', () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

type Loc = 'ru' | 'en';
const tFor = (loc: Loc) => {
  i18n.global.locale.value = loc;
  return (key: string) => i18n.global.t(key);
};

describe('sourceLabel helper', () => {
  it.each(['ru', 'en'] as const)('keeps brand names in %s', (loc) => {
    const t = tFor(loc);
    expect(sourceLabel('soundcloud', t)).toBe('SoundCloud');
    expect(sourceLabel('bandcamp', t)).toBe('Bandcamp');
    expect(sourceLabel('SoundCloud', t)).toBe('SoundCloud');
    expect(sourceLabel('BANDCAMP', t)).toBe('Bandcamp');
  });

  it.each(['ru', 'en'] as const)('translates local and unknown in %s, never the raw id', (loc) => {
    const t = tFor(loc);
    for (const raw of ['local', 'zzz', '', null, undefined]) {
      const label = sourceLabel(raw, t);
      expect(label).not.toBe('');
      expect(label).not.toBe(raw);
      expect(label).not.toMatch(/^[a-z]+(\.[a-zA-Z]+)+$/); // not a bare key
    }
    expect(sourceLabel('local', t)).not.toBe(sourceLabel('zzz', t));
  });

  it('differs between ru and en for local and unknown', () => {
    const ru = tFor('ru');
    const ruLocal = sourceLabel('local', ru);
    const ruUnknown = sourceLabel('zzz', ru);
    const en = tFor('en');
    expect(sourceLabel('local', en)).not.toBe(ruLocal);
    expect(sourceLabel('zzz', en)).not.toBe(ruUnknown);
  });
});

function mountRow(props: Record<string, unknown>) {
  return mount(MediaRow, {
    props: { layout: 'table', ...props },
    global: {
      plugins: [i18n],
      stubs: { ActionMenu: true, PlaylistPicker: true, AppImage: true },
    },
  });
}

const track = (system?: string) => ({
  title: 'T',
  liked: false,
  ...(system ? { source: { system, id: '1' } } : {}),
});

describe('MediaRow source text', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('shows the brand for soundcloud', () => {
    i18n.global.locale.value = 'en';
    const w = mountRow({ type: 'track', track: track('soundcloud') });
    expect(w.find('.track-col-source .source-tag').text()).toBe('SoundCloud');
  });

  it.each(['ru', 'en'] as const)('shows the translated unknown label for zzz in %s', (loc) => {
    const t = tFor(loc);
    const w = mountRow({ type: 'track', track: track('zzz') });
    const text = w.find('.track-col-source .source-tag').text();
    expect(text).not.toBe('zzz');
    expect(text).toBe(sourceLabel('zzz', t));
  });

  it('keeps the local label for tracks without a system', () => {
    i18n.global.locale.value = 'en';
    const w = mountRow({ type: 'track', track: track() });
    expect(w.find('.track-col-source .source-tag').text()).toBe('Local');
  });

  it('keeps playlist rows without a source system empty', () => {
    const t = tFor('en');
    const w = mountRow({ type: 'playlist', playlist: { id: 1, name: 'P', trackCount: 0 } });
    const tag = w.find('.track-col-source .source-tag');
    expect(tag.exists() ? tag.text() : '').toBe('');
    expect(w.find('.track-col-source').text()).not.toContain(sourceLabel('zzz', t));
  });
});

describe('call sites', () => {
  it('HistoryView no longer renders source.system directly', () => {
    expect(historySource).not.toMatch(/\{\{[^}]*source\.system[^}]*\}\}/);
  });

  it.each([
    ['HistoryView', historySource],
    ['MediaRow', mediaRowSource],
    ['PlayerControl', playerSource],
    ['SettingsView', settingsSource],
  ])('%s uses the shared helper', (_name, src) => {
    expect(src).toMatch(/from ['"]@\/utils\/sourceLabel['"]/);
    expect(src).toContain('sourceLabel(');
  });

  it('SettingsView has no literal Bandcamp text', () => {
    expect(settingsSource).not.toMatch(/>\s*Bandcamp\s*</);
  });
});
