import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

async function load() {
  vi.resetModules();
  return await import('./index');
}

describe('pickInitialLocale', () => {
  it('stored supported locale wins over any language', async () => {
    const { pickInitialLocale } = await load();
    expect(pickInitialLocale('ru', 'en-US')).toBe('ru');
    expect(pickInitialLocale('en', 'ru-RU')).toBe('en');
  });

  it('ignores an invalid stored value', async () => {
    const { pickInitialLocale } = await load();
    expect(pickInitialLocale('fr', 'ru')).toBe('ru');
    expect(pickInitialLocale('fr', 'de')).toBe('en');
    expect(pickInitialLocale('', 'de')).toBe('en');
  });

  it('maps ru* languages to ru in any case', async () => {
    const { pickInitialLocale } = await load();
    for (const l of ['ru', 'ru-RU', 'RU']) {
      expect(pickInitialLocale(null, l)).toBe('ru');
    }
  });

  it('maps everything else or missing to en', async () => {
    const { pickInitialLocale } = await load();
    for (const l of ['en-US', 'de', undefined]) {
      expect(pickInitialLocale(null, l)).toBe('en');
    }
  });
});

describe('i18n module initial locale', () => {
  beforeEach(() => {
    localStorage.clear();
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    localStorage.clear();
  });

  it('uses navigator.language ru-RU on first start without writing storage', async () => {
    vi.stubGlobal('navigator', { language: 'ru-RU' });
    const { i18n } = await load();
    expect(i18n.global.locale.value).toBe('ru');
    expect(document.documentElement.lang).toBe('ru');
    expect(localStorage.getItem('locale')).toBeNull();
  });

  it('uses en for a non-Russian system language', async () => {
    vi.stubGlobal('navigator', { language: 'de-DE' });
    const { i18n } = await load();
    expect(i18n.global.locale.value).toBe('en');
    expect(document.documentElement.lang).toBe('en');
    expect(localStorage.getItem('locale')).toBeNull();
  });

  it('stored locale wins over the system language', async () => {
    localStorage.setItem('locale', 'en');
    vi.stubGlobal('navigator', { language: 'ru-RU' });
    const { i18n } = await load();
    expect(i18n.global.locale.value).toBe('en');
    expect(document.documentElement.lang).toBe('en');
  });

  it('fallback locale is en', async () => {
    vi.stubGlobal('navigator', { language: 'ru' });
    const { i18n } = await load();
    expect(i18n.global.fallbackLocale.value).toBe('en');
  });
});
