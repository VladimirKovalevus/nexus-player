import { beforeEach, describe, expect, it, vi } from 'vitest';
import { defineComponent } from 'vue';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { i18n } from '@/i18n';
import { useAudioStore } from '@/stores/audio';
import type { QueueEntryDto, QueueState, TrackDto } from '@/types/types';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@/utils/providerAuth', () => ({
  invoke: invokeMock,
  requestProviderAuth: vi.fn(() => Promise.resolve()),
}));

import PlaybackQueue from './PlaybackQueue.vue';

// Renders every item, so row content does not depend on layout measurements.
const FlatList = defineComponent({
  props: { items: { type: Array, required: true } },
  template:
    '<div><div v-for="(item, index) in items" :key="index"><slot :item="item" :index="index" /></div></div>',
});

const track = (id: number, title: string): TrackDto => ({
  id,
  title,
  artist: 'Artist',
  album: null,
  durationSecs: 100,
  artworkUrl: null,
  playCount: 0,
  liked: false,
  state: { type: 'streaming' },
  source: null,
  unavailable: false,
});

const trackEntry = (id: number, title: string): QueueEntryDto => ({
  entryType: 'track',
  track: track(id, title),
});

const containerEntry = (
  id: string,
  title: string,
  tracks: TrackDto[] | null = null,
): QueueEntryDto => ({
  entryType: 'container',
  meta: {
    kind: 'album',
    id,
    system: 'bandcamp',
    title,
    artist: 'Band',
    trackCount: 3,
    thumbnail: null,
  },
  tracks,
  playIndex: 0,
});

const queueState = (entries: QueueEntryDto[]): QueueState => ({
  entries,
  currentPosition: { itemIndex: 0, trackWithinContainer: null },
  repeatMode: 'off',
  shuffleEnabled: false,
  searchContext: { system: 'bandcamp', query: 'band' },
  radioMode: null,
});

async function mountQueue(initial: QueueState) {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'get_queue_state') return Promise.resolve(initial);
    if (cmd === 'expand_queue_container') {
      const entries = initial.entries.map((e) =>
        e.entryType === 'container' && !e.tracks
          ? { ...e, tracks: [track(11, 'First'), track(12, 'Second')] }
          : e,
      );
      return Promise.resolve({ ...initial, entries });
    }
    return Promise.resolve(undefined);
  });
  const wrapper = mount(PlaybackQueue, {
    global: { plugins: [i18n], stubs: { VirtualList: FlatList } },
  });
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  invokeMock.mockReset();
  i18n.global.locale.value = 'en';
});

describe('PlaybackQueue container rows', () => {
  it('shows the title of a container that is not expanded yet', async () => {
    const w = await mountQueue(
      queueState([trackEntry(1, 'Opener'), containerEntry('a1', 'Album A')]),
    );
    expect(w.text()).toContain('Opener');
    expect(w.text()).toContain('Album A');
  });

  it('expanding a container asks the backend for the row index of that container', async () => {
    const w = await mountQueue(
      queueState([
        trackEntry(1, 'Opener'),
        trackEntry(2, 'Second'),
        containerEntry('a1', 'Album A'),
      ]),
    );

    await w.get(`button[aria-label="${i18n.global.t('media.expand')}"]`).trigger('click');
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('expand_queue_container', { itemIndex: 2 });
    expect(w.text()).toContain('First');
  });

  it('keeps no row marked as expanded once the container is replaced by track rows', async () => {
    const w = await mountQueue(
      queueState([trackEntry(1, 'Opener'), containerEntry('a1', 'Album A')]),
    );
    await w.get(`button[aria-label="${i18n.global.t('media.expand')}"]`).trigger('click');
    await flushPromises();
    expect(w.find(`button[aria-label="${i18n.global.t('media.collapse')}"]`).exists()).toBe(true);

    // The album was played: its row became track rows and another container took index 1.
    const audioStore = useAudioStore();
    audioStore.updateQueueState(
      queueState([
        trackEntry(1, 'Opener'),
        containerEntry('b1', 'Album B'),
        trackEntry(11, 'First'),
        trackEntry(12, 'Second'),
      ]),
    );
    await flushPromises();

    expect(w.text()).toContain('Album B');
    expect(w.find(`button[aria-label="${i18n.global.t('media.collapse')}"]`).exists()).toBe(false);
    expect(w.find(`button[aria-label="${i18n.global.t('media.expand')}"]`).exists()).toBe(true);
  });
});
