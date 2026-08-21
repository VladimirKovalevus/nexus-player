import { describe, expect, it } from 'vitest';
import { errorMessage, isCommandError } from './errorMessage';

describe('errorMessage', () => {
  it('returns the message of an internal command error', () => {
    expect(errorMessage({ code: 'internal', message: 'boom' })).toBe('boom');
  });

  it('returns the message of a challenge_required command error', () => {
    expect(
      errorMessage({
        code: 'challenge_required',
        message: 'solve the captcha',
        provider: 'vk',
        url: 'https://example.com/challenge',
      }),
    ).toBe('solve the captcha');
  });

  it('returns the message of an Error', () => {
    expect(errorMessage(new Error('plain failure'))).toBe('plain failure');
  });

  it('returns a string as is', () => {
    expect(errorMessage('just text')).toBe('just text');
  });

  it.each([null, undefined, 42, {}, { foo: 'bar' }])('falls back to readable text for %j', (value) => {
    const text = errorMessage(value);
    expect(typeof text).toBe('string');
    expect(text.length).toBeGreaterThan(0);
    expect(text).not.toContain('[object Object]');
  });
});

describe('isCommandError', () => {
  it('accepts objects shaped like a command error', () => {
    expect(isCommandError({ code: 'internal', message: 'x' })).toBe(true);
    expect(
      isCommandError({ code: 'challenge_required', message: 'x', provider: 'vk', url: 'u' }),
    ).toBe(true);
  });

  it('rejects everything else', () => {
    expect(isCommandError(null)).toBe(false);
    expect(isCommandError(undefined)).toBe(false);
    expect(isCommandError('internal')).toBe(false);
    expect(isCommandError(new Error('x'))).toBe(false);
    expect(isCommandError({ message: 'no code' })).toBe(false);
    expect(isCommandError({ code: 'internal' })).toBe(false);
    expect(isCommandError({ code: 1, message: 'x' })).toBe(false);
  });
});

describe('UI error text conversion', () => {
  const sources = import.meta.glob('/src/**/*.{ts,vue}', {
    query: '?raw',
    import: 'default',
    eager: true,
  }) as Record<string, string>;

  const pattern = /String\((e|err|error)\)|\$\{(e|err|error)\}/;

  it('never stringifies a caught error directly into UI text', () => {
    const offenders: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      if (/\.test\.ts$/.test(path) || path.endsWith('/utils/errorMessage.ts')) continue;
      text.split('\n').forEach((line, i) => {
        if (/console\./.test(line)) return;
        if (pattern.test(line)) offenders.push(`${path}:${i + 1}: ${line.trim()}`);
      });
    }
    expect(offenders).toEqual([]);
  });
});
