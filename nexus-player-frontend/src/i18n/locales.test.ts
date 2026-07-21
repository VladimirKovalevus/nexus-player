import { describe, expect, it } from 'vitest';
import ru from './locales/ru.json';
import en from './locales/en.json';

function flatten(obj: unknown, prefix = ''): Map<string, unknown> {
  const out = new Map<string, unknown>();
  if (obj !== null && typeof obj === 'object' && !Array.isArray(obj)) {
    for (const [k, v] of Object.entries(obj)) {
      const key = prefix ? `${prefix}.${k}` : k;
      if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
        for (const [ik, iv] of flatten(v, key)) out.set(ik, iv);
      } else {
        out.set(key, v);
      }
    }
  }
  return out;
}

function emptyKeys(map: Map<string, unknown>): string[] {
  return [...map].filter(([, v]) => typeof v !== 'string' || v.trim() === '').map(([k]) => k);
}

describe('locale files (R5)', () => {
  const ruKeys = flatten(ru);
  const enKeys = flatten(en);

  it('have identical key sets', () => {
    const missingInEn = [...ruKeys.keys()].filter((k) => !enKeys.has(k));
    const missingInRu = [...enKeys.keys()].filter((k) => !ruKeys.has(k));
    expect({ missingInEn, missingInRu }).toEqual({ missingInEn: [], missingInRu: [] });
  });

  it('ru has no empty values', () => {
    expect(emptyKeys(ruKeys)).toEqual([]);
  });

  it('en has no empty values', () => {
    expect(emptyKeys(enKeys)).toEqual([]);
  });
});
