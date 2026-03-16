import { defineStore } from 'pinia';
import { ref } from 'vue';

export const useThemeStore = defineStore('theme', () => {
  const activeTheme = ref(localStorage.getItem('theme') || 'dark');
  const customCss = ref(localStorage.getItem('custom-theme-css') || '');

  const setTheme = (themeName: string) => {
    activeTheme.value = themeName;
    document.documentElement.setAttribute('data-theme', themeName);
    localStorage.setItem('theme', themeName);
  };

  const setCustomCss = (css: string) => {
    customCss.value = css;
    localStorage.setItem('custom-theme-css', css);
    applyCustomCss(css);
  };

  const applyCustomCss = (css: string) => {
    let styleEl = document.getElementById('user-custom-theme');
    if (!styleEl) {
      styleEl = document.createElement('style');
      styleEl.id = 'user-custom-theme';
      document.head.appendChild(styleEl);
    }
    styleEl.textContent = css;
  };

  const initTheme = () => {
    document.documentElement.setAttribute('data-theme', activeTheme.value);
    if (customCss.value) {
      applyCustomCss(customCss.value);
    }
  };

  return {
    activeTheme,
    customCss,
    setTheme,
    setCustomCss,
    initTheme,
  };
});
