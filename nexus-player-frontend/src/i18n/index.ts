import { createI18n } from 'vue-i18n';
import ru from './locales/ru.json';
import en from './locales/en.json';

export const LOCALES = ['ru', 'en'] as const;
export type Locale = (typeof LOCALES)[number];

const STORAGE_KEY = 'locale';

const stored = localStorage.getItem(STORAGE_KEY) as Locale | null;
const initial: Locale = stored && LOCALES.includes(stored) ? stored : 'ru';

export const i18n = createI18n({
  legacy: false,
  globalInjection: true,
  locale: initial,
  fallbackLocale: 'ru',
  messages: { ru, en },
});

document.documentElement.lang = initial;

export function setLocale(locale: Locale) {
  i18n.global.locale.value = locale;
  document.documentElement.lang = locale;
  localStorage.setItem(STORAGE_KEY, locale);
}
