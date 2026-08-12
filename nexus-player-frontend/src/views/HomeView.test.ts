import { beforeEach, describe, expect, it, vi } from 'vitest';
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

async function mountHome(playlists: PlaylistDto[], mobile: boolean) {
  setMatchMedia(mobile);
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'get_all_playlists') return Promise.resolve(playlists);
    if (cmd === 'get_all_tracks') return Promise.resolve([]);
    return Promise.resolve(undefined);
  });
  const wrapper = mount(HomeView, {
    global: {
      plugins: [i18n],
      stubs: { VirtualList: true },
    },
  });
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockReset();
  i18n.global.locale.value = 'en';
});

describe('HomeView mobile Playlists row (R11, R13, R14)', () => {
  it('R13: 0 playlists hides the block and keeps the "+ playlist" button', async () => {
    const w = await mountHome([], true);
    expect(w.find('[data-testid="home-playlists"]').exists()).toBe(false);
    const header = w.find('.view-header');
    expect(header.text()).toContain(i18n.global.t('home.addPlaylist'));
    expect(header.find('button.deck-btn').exists()).toBe(true);
  });

  it('R11: 5 playlists show 5 tiles, no All tile, and a link with 5', async () => {
    const w = await mountHome(makePlaylists(5), true);
    const block = w.find('[data-testid="home-playlists"]');
    expect(block.exists()).toBe(true);
    expect(w.findAll('[data-testid="home-playlist-tile"]')).toHaveLength(5);
    expect(w.find('[data-testid="home-playlists-all-tile"]').exists()).toBe(false);
    const link = w.find('[data-testid="home-playlists-all-link"]');
    expect(link.exists()).toBe(true);
    expect(link.text()).toContain('5');
  });

  it('R11: 30 playlists show the first 12 in order plus an All tile with 30', async () => {
    const all = makePlaylists(30);
    const w = await mountHome(all, true);
    const tiles = w.findAll('[data-testid="home-playlist-tile"]');
    expect(tiles).toHaveLength(12);
    tiles.forEach((tile, i) => expect(tile.text()).toContain(all[i].title));
    expect(w.text()).not.toContain(all[12].title);
    const allTile = w.find('[data-testid="home-playlists-all-tile"]');
    expect(allTile.exists()).toBe(true);
    expect(allTile.text()).toContain('30');
    expect(w.find('[data-testid="home-playlists-all-link"]').text()).toContain('30');
  });

  it('R11: exactly 12 playlists show all of them with no All tile', async () => {
    const w = await mountHome(makePlaylists(12), true);
    expect(w.findAll('[data-testid="home-playlist-tile"]')).toHaveLength(12);
    expect(w.find('[data-testid="home-playlists-all-tile"]').exists()).toBe(false);
  });

  it('R14: desktop renders no Playlists block even with 30 playlists', async () => {
    const w = await mountHome(makePlaylists(30), false);
    expect(w.find('[data-testid="home-playlists"]').exists()).toBe(false);
    expect(w.find('[data-testid="home-playlist-tile"]').exists()).toBe(false);
  });
});
