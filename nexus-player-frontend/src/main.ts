import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import './styles/global.scss';
import { useNotificationStore } from './stores/notifications';
import { useThemeStore } from './stores/theme';

const app = createApp(App);
const pinia = createPinia();

// Enable Vue performance marks — visible in Web Inspector → Timelines → JS & Events
// Toggle this flag to profile component render/update/patch timing
const PERF_PROFILING = true;
if (PERF_PROFILING) app.config.performance = true;

app.use(pinia);

// Initialize stores
const themeStore = useThemeStore(pinia);
themeStore.initTheme();

const notificationStore = useNotificationStore(pinia);

app.config.errorHandler = (err, _instance, info) => {
  console.error('Vue Error:', err, info);
  notificationStore.addNotification(`Vue Error: ${err}`, 'error');
};

window.addEventListener('error', (event) => {
  console.error('Global Error:', event.error);
  notificationStore.addNotification(`Global Error: ${event.message}`, 'error');
});

window.addEventListener('unhandledrejection', (event) => {
  console.error('Promise Rejection:', event.reason);
  notificationStore.addNotification(`Promise Error: ${event.reason}`, 'error');
});

app.mount('#app');
