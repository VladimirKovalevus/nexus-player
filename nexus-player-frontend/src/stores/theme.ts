import { defineStore } from 'pinia';
import { ref } from 'vue';

export const THEMES = ['dark', 'light'] as const;
export type ThemeName = (typeof THEMES)[number];

const isTheme = (value: string | null): value is ThemeName =>
  THEMES.includes(value as ThemeName);

export const useThemeStore = defineStore('theme', () => {
  // Values saved by removed themes (retro, cyberpunk, cartoon) fall back to dark.
  const stored = localStorage.getItem('theme');
  const activeTheme = ref<ThemeName>(isTheme(stored) ? stored : 'dark');

  const setTheme = (themeName: ThemeName) => {
    activeTheme.value = themeName;
    document.documentElement.setAttribute('data-theme', themeName);
    localStorage.setItem('theme', themeName);
  };

  const initTheme = () => setTheme(activeTheme.value);

  return {
    activeTheme,
    setTheme,
    initTheme,
  };
});
