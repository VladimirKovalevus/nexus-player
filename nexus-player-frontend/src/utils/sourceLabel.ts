const BRANDS: Record<string, string> = {
  soundcloud: 'SoundCloud',
  bandcamp: 'Bandcamp',
};

/** Display name for a provider id: brands stay as brands, the rest goes through i18n. */
export function sourceLabel(system: string | null | undefined, t: (key: string) => string): string {
  const id = (system ?? '').trim().toLowerCase();
  if (id in BRANDS) return BRANDS[id];
  if (id === 'local') return t('track.source.local');
  return t('track.source.unknown');
}
