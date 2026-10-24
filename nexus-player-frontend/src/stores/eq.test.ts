import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { invoke } from '@/utils/providerAuth';
import { i18n } from '@/i18n';
import ru from '@/i18n/locales/ru.json';
import en from '@/i18n/locales/en.json';
import eqStoreSource from './eq.ts?raw';
import eqPanelSource from '@/components/EqPanel.vue?raw';
import { presetLabelKey, useEqStore } from './eq';

vi.mock('@/utils/providerAuth', () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

const IDS = ['flat', 'rock', 'pop', 'jazz', 'classical', 'bass_boost', 'vocal'];
const GAINS = [1, 2, 3, 4, 5, -1, -2, -3, -4, -5];

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(invoke).mockClear();
});

describe('eq store ids', () => {
  it('defaults to the flat preset id', () => {
    expect(useEqStore().settings.presetName).toBe('flat');
  });

  it('applyPreset stores the id and sends it to the backend', async () => {
    const store = useEqStore();
    await store.applyPreset({ name: 'rock', gains: GAINS });
    expect(store.settings.presetName).toBe('rock');
    expect(invoke).toHaveBeenCalledWith(
      'set_eq_settings',
      expect.objectContaining({
        settings: expect.objectContaining({ presetName: 'rock', gains: GAINS }),
      }),
    );
  });

  it('setting a band gain clears the preset', async () => {
    const store = useEqStore();
    await store.applyPreset({ name: 'pop', gains: GAINS });
    await store.setBandGain(0, 3);
    expect(store.settings.presetName).toBeNull();
  });

  it('presetLabelKey maps an id to eq.presets.<id>', () => {
    for (const id of IDS) expect(presetLabelKey(id)).toBe(`eq.presets.${id}`);
  });

  it.each([
    ['ru', ru],
    ['en', en],
  ] as const)('labels resolve in %s', (locale, messages) => {
    i18n.global.locale.value = locale;
    const presets = (messages as unknown as { eq: { presets: Record<string, string> } }).eq.presets;
    for (const id of IDS) {
      const label = i18n.global.t(presetLabelKey(id));
      expect(label).not.toBe(presetLabelKey(id));
      expect(label).toBe(presets[id]);
    }
  });

  it('has no Cyrillic in stores/eq.ts or EqPanel.vue', () => {
    expect(eqStoreSource).not.toMatch(/[\u0400-\u04FF]/);
    expect(eqPanelSource).not.toMatch(/[\u0400-\u04FF]/);
  });
});
