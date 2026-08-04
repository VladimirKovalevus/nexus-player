import { createI18n } from 'vue-i18n';
import ru from './locales/ru.json';
import en from './locales/en.json';

export const LOCALES = ['ru', 'en'] as const;
export type Locale = (typeof LOCALES)[number];

const STORAGE_KEY = 'locale';

export function pickInitialLocale(stored: string | null, language: string | undefined): Locale {
  if (stored && (LOCALES as readonly string[]).includes(stored)) return stored as Locale;
  return language?.toLowerCase().startsWith('ru') ? 'ru' : 'en';
}

const initial = pickInitialLocale(localStorage.getItem(STORAGE_KEY), navigator.language);

export const i18n = createI18n({
  legacy: false,
  globalInjection: true,
  locale: initial,
  fallbackLocale: 'en',
  messages: { ru, en },
});

document.documentElement.lang = initial;

export function setLocale(locale: Locale) {
  i18n.global.locale.value = locale;
  document.documentElement.lang = locale;
  localStorage.setItem(STORAGE_KEY, locale);
}
