import { beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { i18n } from '@/i18n';
import { confirmRequest, settleConfirm } from '@/composables/useConfirm';
import MediaRow from './MediaRow.vue';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@/utils/providerAuth', () => ({ invoke: invokeMock }));

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockClear();
  settleConfirm(false);
  i18n.global.locale.value = 'en';
});

function mountTrack(liked: boolean) {
  return mount(MediaRow, {
    props: { type: 'track', track: { id: 1, title: 'Song', liked }, layout: 'table' },
    global: { plugins: [i18n], stubs: { ActionMenu: true, PlaylistPicker: true, AppImage: true } },
  });
}

it.each([false, true])('requires confirmation to remove a like (confirm=%s)', async (confirmed) => {
  const w = mountTrack(true);
  await w.get('button[aria-label="Remove from favorites"]').trigger('click');
  expect(confirmRequest.value).not.toBeNull();
  expect(invokeMock).not.toHaveBeenCalled();
  expect(w.props('track')?.liked).toBe(true);
  settleConfirm(confirmed);
  await flushPromises();
  expect(w.props('track')?.liked).toBe(!confirmed);
  expect(invokeMock).toHaveBeenCalledTimes(confirmed ? 1 : 0);
});

it('adds a like without confirmation', async () => {
  const w = mountTrack(false);
  await w.get('button[aria-label="Add to favorites"]').trigger('click');
  await flushPromises();
  expect(confirmRequest.value).toBeNull();
  expect(w.props('track')?.liked).toBe(true);
  expect(invokeMock).toHaveBeenCalledWith('toggle_liked', expect.any(Object));
});
