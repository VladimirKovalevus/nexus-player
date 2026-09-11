import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { invoke } from '@/utils/providerAuth';
import { useAudioStore } from './audio';
import { useNotificationStore } from './notifications';

vi.mock('@/utils/providerAuth', () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
  requestProviderAuth: vi.fn(() => Promise.resolve()),
}));

const args = {
  system: 'bandcamp',
  contentType: 'track',
  query: 'band',
  limit: 20,
  offset: 40,
  selectedIndex: 7,
  resultId: 't47',
};

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue(undefined);
});

describe('searchAndPlay store action', () => {
  it('shows one error notification and does not throw when handle_search rejects', async () => {
    vi.mocked(invoke).mockRejectedValue({ code: 'internal', message: 'search backend down' });
    const audio = useAudioStore();
    const notes = useNotificationStore();

    await expect(audio.searchAndPlay(args)).resolves.not.toThrow();

    expect(notes.notifications).toHaveLength(1);
    expect(notes.notifications[0].type).toBe('error');
    expect(notes.notifications[0].message).toContain('search backend down');
  });

  it('never falls back to handle_streaming_track after a failure', async () => {
    vi.mocked(invoke).mockRejectedValue({ code: 'internal', message: 'boom' });
    await useAudioStore().searchAndPlay(args);

    const commands = vi.mocked(invoke).mock.calls.map((c) => c[0]);
    expect(commands).toContain('handle_search');
    expect(commands).not.toContain('handle_streaming_track');
  });

  it('passes the chosen result to handle_search', async () => {
    await useAudioStore().searchAndPlay(args);
    expect(invoke).toHaveBeenCalledWith('handle_search', expect.objectContaining(args));
  });

  it('adds no notification on success', async () => {
    const notes = useNotificationStore();
    await useAudioStore().searchAndPlay(args);
    expect(notes.notifications).toHaveLength(0);
  });

  it('clears the loading flag after a failure', async () => {
    vi.mocked(invoke).mockRejectedValue({ code: 'internal', message: 'boom' });
    const audio = useAudioStore();
    const before = audio.isLoading;
    await audio.searchAndPlay(args);
    expect(audio.isLoading).toBe(before);
  });
});
