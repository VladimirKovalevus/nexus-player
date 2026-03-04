import { defineStore } from 'pinia';
import { ref } from 'vue';
import type { SearchResult } from '@/types/types';

export const useStreamingStore = defineStore('streaming', () => {
  const query = ref('');
  const selectedProvider = ref('bandcamp');
  const selectedContentType = ref<string | null>(null);
  const limit = ref(10);
  const offset = ref(0);
  const searchResults = ref<SearchResult[]>([]);
  const error = ref<string | null>(null);

  return { query, selectedProvider, selectedContentType, limit, offset, searchResults, error };
});
