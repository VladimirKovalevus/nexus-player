import { beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { i18n } from '@/i18n';
import { invoke } from '@/utils/providerAuth';
import { useStreamingStore } from '@/stores/streaming';
import { useAudioStore } from '@/stores/audio';
import { useNotificationStore } from '@/stores/notifications';
import type { SearchResult } from '@/types/types';
import StreamingSearch from './StreamingSearch.vue';

vi.mock('@/utils/providerAuth', () => ({
  invoke: vi.fn(),
  requestProviderAuth: vi.fn(),
}));

const url = 'https://artist.bandcamp.com/track/song';
const track: SearchResult = {
  type: 'track', id: 1, title: 'Song', artist: 'Artist',
  liked: false,
  source: { system: 'bandcamp', url },
};
const results: SearchResult[] = [
  track,
  { type: 'album', id: 'album', title: 'Album', artist: 'Artist', trackCount: 2, url, system: 'bandcamp' },
  { type: 'playlist', id: 'playlist', title: 'Playlist', creator: 'Artist', trackCount: 2, url, system: 'soundcloud' },
];

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === 'direct_link_resolve') return track;
    if (command === 'handle_search') throw new Error('Failed to load search result at index 0');
  });
});

function mountSearch() {
  return mount(StreamingSearch, {
    global: {
      plugins: [i18n],
      stubs: {
        MediaRow: { emits: ['play', 'click'], template: '<button class="result" @click="$emit(\'play\')">Play</button>' },
        PlaylistCreateSimpleModal: true,
      },
    },
  });
}

it.each(results)('plays a resolved HTTPS $type without searching for its URL and index again', async (result) => {
  vi.mocked(invoke).mockResolvedValue(result);
  const wrapper = mountSearch();
  await wrapper.get('input').setValue(url);
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  expect(useStreamingStore().searchResults).toHaveLength(1);
  await wrapper.get('.result').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('handle_streaming_track', {
    searchResult: result, strategy: 'cache', play: true,
  });
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'handle_search')).toBe(false);
  expect(wrapper.emitted('refetch')).toHaveLength(1);
});

it('uses search positions again after switching from a link to a text query', async () => {
  const wrapper = mountSearch();
  await wrapper.get('input').setValue(url);
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  vi.mocked(invoke).mockImplementation(async (command) => command === 'search_streaming' ? [track] : undefined);
  await wrapper.get('input').setValue('Artist');
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  await wrapper.get('.result').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('handle_search', expect.objectContaining({ query: 'Artist', selectedIndex: 0 }));
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'handle_streaming_track')).toBe(false);
});

it('reports direct playback errors and restores the loading state', async () => {
  const wrapper = mountSearch();
  await wrapper.get('input').setValue(url);
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  vi.mocked(invoke).mockRejectedValue(new Error('Playback failed'));
  const audio = useAudioStore();
  const wasLoading = audio.isLoading;
  await wrapper.get('.result').trigger('click');
  await flushPromises();
  expect(audio.isLoading).toBe(wasLoading);
  expect(useNotificationStore().notifications).toHaveLength(1);
  expect(useNotificationStore().notifications[0].message).toContain('Playback failed');
  expect(wrapper.emitted('refetch')).toBeUndefined();
});
