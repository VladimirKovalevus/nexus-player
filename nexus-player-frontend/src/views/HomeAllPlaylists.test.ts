import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { i18n } from '@/i18n';
import { setMatchMedia } from '@/test/helpers';
import type { PlaylistDto } from '@/types/types';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@/utils/providerAuth', () => ({
  invoke: invokeMock,
  requestProviderAuth: vi.fn(() => Promise.resolve()),
}));

import HomeView from './HomeView.vue';

const makePlaylists = (n: number): PlaylistDto[] =>
  Array.from({ length: n }, (_, i) => ({
    id: i + 1,
    title: `PL-${String(i + 1).padStart(2, '0')}`,
    description: null,
    creator: null,
    thumbnail: null,
    trackCount: 3,
    createdAt: 0,
    updatedAt: 0,
  }));

async function mountHome() {
  setMatchMedia(true);
  const playlists = makePlaylists(30);
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'get_all_playlists') return Promise.resolve(playlists);
    if (cmd === 'get_all_tracks') return Promise.resolve([]);
    return Promise.resolve(undefined);
  });
  const wrapper = mount(HomeView, {
    global: { plugins: [i18n], stubs: { VirtualList: true } },
  });
  await flushPromises();
  return wrapper;
}

const GRID = '[data-testid="home-playlists-grid"]';
const ROW = '[data-testid="home-playlists"]';

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockReset();
  i18n.global.locale.value = 'en';
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('HomeView All playlists grid', () => {
  it('the All tile opens a grid with all 30 playlists and hides the row', async () => {
    const w = await mountHome();
    await w.find('[data-testid="home-playlists-all-tile"]').trigger('click');
    await flushPromises();
    const grid = w.find(GRID);
    expect(grid.exists()).toBe(true);
    expect(grid.findAll('[title^="PL-"]')).toHaveLength(30);
    expect(w.find(ROW).exists()).toBe(false);
    expect(w.find('[data-testid="home-playlists-back"]').exists()).toBe(true);
    expect(grid.text()).toContain('30');
    w.unmount();
  });

  it('opening pushes exactly one history entry', async () => {
    const push = vi.spyOn(history, 'pushState');
    const w = await mountHome();
    await w.find('[data-testid="home-playlists-all-link"]').trigger('click');
    await flushPromises();
    expect(push).toHaveBeenCalledTimes(1);
    w.unmount();
  });

  it('Back button returns to the row, via history.back, with no navigate event', async () => {
    const back = vi.spyOn(history, 'back');
    const w = await mountHome();
    await w.find('[data-testid="home-playlists-all-tile"]').trigger('click');
    await flushPromises();
    await w.find('[data-testid="home-playlists-back"]').trigger('click');
    expect(back).toHaveBeenCalled();
    await vi.waitFor(() => {
      expect(w.find(GRID).exists()).toBe(false);
    });
    await flushPromises();
    expect(w.find(ROW).exists()).toBe(true);
    expect(w.emitted('navigate-playlist')).toBeUndefined();
    w.unmount();
  });

  it('a popstate closes the grid', async () => {
    const w = await mountHome();
    await w.find('[data-testid="home-playlists-all-link"]').trigger('click');
    await flushPromises();
    expect(w.find(GRID).exists()).toBe(true);
    window.dispatchEvent(new PopStateEvent('popstate'));
    await flushPromises();
    expect(w.find(GRID).exists()).toBe(false);
    expect(w.find(ROW).exists()).toBe(true);
    expect(w.emitted('navigate-playlist')).toBeUndefined();
    w.unmount();
  });

  it('.home-scroll scrollTop is restored after Back', async () => {
    const w = await mountHome();
    const scroller = w.find('.home-scroll').element as HTMLElement;
    let top = 0;
    Object.defineProperty(scroller, 'scrollTop', {
      configurable: true,
      get: () => top,
      set: (v: number) => {
        top = v;
      },
    });
    top = 500;
    await w.find('[data-testid="home-playlists-all-tile"]').trigger('click');
    await flushPromises();
    top = 0; // browser clamping once the tracks are hidden
    await w.find('[data-testid="home-playlists-back"]').trigger('click');
    await vi.waitFor(() => {
      expect(w.find(GRID).exists()).toBe(false);
    });
    await vi.waitFor(() => {
      expect(top).toBe(500);
    });
    w.unmount();
  });

  it('unmounting with the grid open steps back once and leaves no listener', async () => {
    const back = vi.spyOn(history, 'back');
    const w = await mountHome();
    await w.find('[data-testid="home-playlists-all-tile"]').trigger('click');
    await flushPromises();
    expect(back).not.toHaveBeenCalled();
    w.unmount();
    expect(back).toHaveBeenCalledTimes(1);
    await flushPromises();
    expect(() => window.dispatchEvent(new PopStateEvent('popstate'))).not.toThrow();
    await flushPromises();
    expect(back).toHaveBeenCalledTimes(1);
  });

  it('unmounting with the grid closed does not touch history', async () => {
    const back = vi.spyOn(history, 'back');
    const w = await mountHome();
    w.unmount();
    expect(back).not.toHaveBeenCalled();
  });

  it('becoming desktop closes the grid', async () => {
    const w = await mountHome();
    await w.find('[data-testid="home-playlists-all-tile"]').trigger('click');
    await flushPromises();
    setMatchMedia(false);
    await flushPromises();
    expect(w.find(GRID).exists()).toBe(false);
    w.unmount();
  });

  it('the grid text renders in both locales', async () => {
    for (const locale of ['en', 'ru'] as const) {
      i18n.global.locale.value = locale;
      const w = await mountHome();
      await w.find('[data-testid="home-playlists-all-tile"]').trigger('click');
      await flushPromises();
      const text = w.find(GRID).text();
      expect(text).not.toMatch(/home\.|common\./);
      expect(text).toContain('30');
      w.unmount();
    }
  });
});
