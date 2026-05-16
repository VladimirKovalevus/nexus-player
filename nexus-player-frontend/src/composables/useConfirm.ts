import { shallowRef } from 'vue';

export interface ConfirmRequest {
  title: string;
  message?: string;
  confirmLabel: string;
  resolve: (confirmed: boolean) => void;
}

/** The open request, rendered by the single ConfirmDialog mounted in App.vue. */
export const confirmRequest = shallowRef<ConfirmRequest | null>(null);

/**
 * Asks the user to confirm. Use this instead of window.confirm: under Tauri that
 * returns a Promise, which is always truthy, so a synchronous check never stops the action.
 */
export function confirmAction(options: Omit<ConfirmRequest, 'resolve'>): Promise<boolean> {
  confirmRequest.value?.resolve(false);
  return new Promise((resolve) => {
    confirmRequest.value = { ...options, resolve };
  });
}

export function settleConfirm(confirmed: boolean) {
  confirmRequest.value?.resolve(confirmed);
  confirmRequest.value = null;
}
