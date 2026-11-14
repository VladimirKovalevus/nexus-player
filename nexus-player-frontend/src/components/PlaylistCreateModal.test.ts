import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { i18n } from '@/i18n';
import PlaylistCreateModal from './PlaylistCreateModal.vue';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@/utils/providerAuth', () => ({ invoke: invokeMock }));
const track = { id: 1, title: 'Track', artist: 'Artist', durationSecs: 120 };
const albums = [{ id: 10, title: 'Album A', trackCount: 3 }, { id: 20, title: 'Album B', trackCount: 4 }];

beforeEach(() => {
  i18n.global.locale.value = 'en';
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_all_tracks') return [track];
    if (cmd === 'get_all_playlists') return albums;
    if (cmd === 'create_playlist_with_tracks') return 99;
  });
});

async function openModal() {
  const wrapper = mount(PlaylistCreateModal, {
    props: { isOpen: true },
    global: { plugins: [i18n], stubs: { AppModal: { template: '<div><slot /><slot name="footer" /></div>' } } },
  });
  await flushPromises();
  await wrapper.get('#new-playlist-title').setValue('My playlist');
  return wrapper;
}
type Wrapper = Awaited<ReturnType<typeof openModal>>;
async function click(wrapper: Wrapper, text: string) {
  const button = wrapper.findAll('button').find(button => button.text().startsWith(text));
  expect(button).toBeDefined();
  await button!.trigger('click');
}

describe('playlist creation selection', () => {
  it('preserves selections across views and saves tracks and whole albums', async () => {
    const w = await openModal();
    await w.get('input[type="checkbox"]').setValue(true);
    await click(w, 'Albums');
    for (const box of w.findAll('input[type="checkbox"]')) await box.setValue(true);
    await click(w, 'Tracks');
    expect((w.get('input[type="checkbox"]').element as HTMLInputElement).checked).toBe(true);
    await click(w, 'Albums');
    expect(w.findAll('input[type="checkbox"]').every(input => (input.element as HTMLInputElement).checked)).toBe(true);
    await click(w, 'Save');
    await flushPromises();
    expect(invokeMock).toHaveBeenCalledWith('create_playlist_with_tracks', {
      request: { title: 'My playlist', description: null, tracks: [track] },
    });
    for (const childPlaylistId of [10, 20]) {
      expect(invokeMock).toHaveBeenCalledWith('add_playlist_to_playlist', { parentPlaylistId: 99, childPlaylistId });
    }
    expect(w.emitted('created')).toHaveLength(1);
    expect(w.emitted('close')).toHaveLength(1);
    w.unmount();
  });

  it('excludes deselected albums and saves only albums', async () => {
    const w = await openModal();
    await click(w, 'Albums');
    const boxes = w.findAll('input[type="checkbox"]');
    await boxes[0].setValue(true);
    await boxes[1].setValue(true);
    await boxes[0].setValue(false);
    await click(w, 'Save');
    await flushPromises();
    expect(invokeMock).toHaveBeenCalledWith('create_playlist_with_tracks', {
      request: { title: 'My playlist', description: null, tracks: [] },
    });
    expect(invokeMock.mock.calls.filter(([cmd]) => cmd === 'add_playlist_to_playlist')).toEqual([
      ['add_playlist_to_playlist', { parentPlaylistId: 99, childPlaylistId: 20 }],
    ]);
    w.unmount();
  });

  it('clears selections on cancel and reloads the library on reopen', async () => {
    const w = await openModal();
    await w.get('input[type="checkbox"]').setValue(true);
    await click(w, 'Albums');
    await w.get('input[type="checkbox"]').setValue(true);
    await click(w, 'Cancel');
    await w.setProps({ isOpen: false });
    await w.setProps({ isOpen: true });
    await flushPromises();
    expect(w.text()).toContain('Tracks, selected: 0');
    expect(w.text()).toContain('Albums, selected: 0');
    expect(invokeMock.mock.calls.filter(([cmd]) => cmd === 'get_all_playlists')).toHaveLength(2);
    w.unmount();
  });

  it('removes an incomplete playlist on failure and retains selections for retry', async () => {
    const w = await openModal();
    await click(w, 'Albums');
    await w.get('input[type="checkbox"]').setValue(true);
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'create_playlist_with_tracks') return 99;
      if (cmd === 'add_playlist_to_playlist') throw new Error('Album failed');
    });
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    await click(w, 'Save');
    await flushPromises();
    expect(invokeMock).toHaveBeenCalledWith('delete_playlist', { playlistId: 99 });
    expect(w.emitted('created')).toBeUndefined();
    expect(w.emitted('close')).toBeUndefined();
    expect(w.text()).toContain('Album failed');
    expect((w.get('input[type="checkbox"]').element as HTMLInputElement).checked).toBe(true);
    consoleSpy.mockRestore();
    w.unmount();
  });
});
