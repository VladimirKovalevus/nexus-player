/** Media-query matching state shared by the `window.matchMedia` stub. */
type Listener = (event: MediaQueryListEvent) => void;

interface Entry {
  query: string;
  listeners: Set<Listener>;
}

let matching = false;
const entries = new Set<Entry>();

/** Make every media query match (true) or none (false); notifies listeners. */
export function setMatchMedia(matches: boolean): void {
  matching = matches;
  for (const entry of entries) {
    for (const listener of entry.listeners) {
      listener({ matches, media: entry.query } as MediaQueryListEvent);
    }
  }
}

export function resetMatchMedia(): void {
  matching = false;
}

export function matchMediaStub(query: string): MediaQueryList {
  const listeners = new Set<Listener>();
  const list = {
    media: query,
    onchange: null,
    get matches() {
      return matching;
    },
    addEventListener: (_type: string, cb: Listener) => listeners.add(cb),
    removeEventListener: (_type: string, cb: Listener) => listeners.delete(cb),
    addListener: (cb: Listener) => listeners.add(cb),
    removeListener: (cb: Listener) => listeners.delete(cb),
    dispatchEvent: () => true,
  } as unknown as MediaQueryList;
  entries.add({ query, listeners });
  return list;
}

/** In-memory Storage. Newer Node versions ship a global `localStorage` that shadows jsdom's. */
export function createMemoryStorage(): Storage {
  const data = new Map<string, string>();
  return {
    get length() {
      return data.size;
    },
    clear: () => data.clear(),
    getItem: (key: string) => (data.has(key) ? (data.get(key) as string) : null),
    key: (index: number) => [...data.keys()][index] ?? null,
    removeItem: (key: string) => {
      data.delete(key);
    },
    setItem: (key: string, value: string) => {
      data.set(key, String(value));
    },
  };
}
