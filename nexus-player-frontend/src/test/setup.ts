import { afterEach, beforeEach, vi } from 'vitest';
import { createMemoryStorage, matchMediaStub, resetMatchMedia } from './helpers';

const storage = createMemoryStorage();
for (const target of [globalThis, window]) {
  Object.defineProperty(target, 'localStorage', { value: storage, configurable: true, writable: true });
}

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  once: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => path,
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

class ResizeObserverStub {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

beforeEach(() => {
  window.matchMedia = matchMediaStub;
  window.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
  globalThis.ResizeObserver = window.ResizeObserver;
});

afterEach(() => {
  localStorage.clear();
  resetMatchMedia();
});
