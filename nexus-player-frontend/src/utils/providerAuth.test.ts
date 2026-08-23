import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import providerAuthSource from './providerAuth.ts?raw';

const rawInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => rawInvoke(...a) }));
vi.mock('@tauri-apps/api/webviewWindow', () => ({ WebviewWindow: { getByLabel: vi.fn() } }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@/utils/platform', () => ({ isAndroid: true }));

import { invoke } from './providerAuth';

const challenge = {
  code: 'challenge_required',
  message: 'solve it',
  provider: 'soundcloud',
  url: 'https://soundcloud.com/x',
};

function authCalls() {
  return rawInvoke.mock.calls.filter(
    (c) => c[0] === 'plugin:soundCloudChallenge|open' || c[0] === 'open_challenge_window',
  );
}

beforeEach(() => {
  setActivePinia(createPinia());
  rawInvoke.mockReset();
});

describe('invoke with provider challenge', () => {
  it('runs the auth flow once and resolves with the retried result', async () => {
    let attempts = 0;
    rawInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'play_track') {
        attempts += 1;
        if (attempts === 1) throw challenge;
        return 'played';
      }
      if (cmd === 'plugin:soundCloudChallenge|open') return { clientId: 'abc', label: 'x' };
      return undefined;
    });
    await expect(invoke('play_track', { id: 1 })).resolves.toBe('played');
    expect(attempts).toBe(2);
    expect(authCalls()).toHaveLength(1);
    expect(rawInvoke).toHaveBeenCalledWith('update_provider_auth', {
      provider: 'soundcloud',
      authData: 'abc',
    });
  });

  it('rethrows an internal command error unchanged without auth', async () => {
    const err = { code: 'internal', message: 'boom' };
    rawInvoke.mockRejectedValue(err);
    await expect(invoke('play_track')).rejects.toBe(err);
    expect(rawInvoke).toHaveBeenCalledTimes(1);
    expect(authCalls()).toHaveLength(0);
  });

  it('rethrows a plain string rejection unchanged without auth', async () => {
    rawInvoke.mockRejectedValue('Challenge required for soundcloud: https://soundcloud.com/x');
    await expect(invoke('play_track')).rejects.toBe(
      'Challenge required for soundcloud: https://soundcloud.com/x',
    );
    expect(rawInvoke).toHaveBeenCalledTimes(1);
    expect(authCalls()).toHaveLength(0);
  });

  it('does not parse error text with a regex', () => {
    expect(providerAuthSource).not.toContain('CHALLENGE_RE');
  });
});
