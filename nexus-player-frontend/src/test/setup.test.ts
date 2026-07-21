import { describe, expect, it } from 'vitest';
import { setMatchMedia } from './helpers';

describe('test setup', () => {
  it('imports useArtwork without Tauri internals', async () => {
    const mod = await import('@/composables/useArtwork');
    expect(mod).toBeTruthy();
  });

  it('matchMedia stub is controllable', () => {
    const mq = window.matchMedia('(max-width: 640px)');
    expect(mq.matches).toBe(false);
    setMatchMedia(true);
    expect(mq.matches).toBe(true);
  });

  it('ResizeObserver exists', () => {
    expect(() => new ResizeObserver(() => {}).observe(document.body)).not.toThrow();
  });

  it('localStorage starts empty', () => {
    expect(localStorage.length).toBe(0);
    localStorage.setItem('x', '1');
  });

  it('localStorage is cleared between tests', () => {
    expect(localStorage.getItem('x')).toBeNull();
  });
});
