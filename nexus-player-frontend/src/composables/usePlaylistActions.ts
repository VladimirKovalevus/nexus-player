import { errorMessage } from '@/utils/errorMessage';
import { useI18n } from 'vue-i18n';
import { invoke } from '@/utils/providerAuth';
import { confirmAction } from '@/composables/useConfirm';
import { useNotificationStore } from '@/stores/notifications';
import { usePlaylistsStore } from '@/stores/playlists';
import type { PlaylistWithItemsDto } from '@/types/types';

export function usePlaylistActions() {
  const { t } = useI18n();
  const notifications = useNotificationStore();
  const playlistsStore = usePlaylistsStore();

  /**
   * Runs an add command and always reports the outcome: a nested playlist lands at the end
   * of a playlist that is not on screen, so without a message nothing appears to happen.
   */
  const addToPlaylist = async (
    targetPlaylistId: number,
    itemTitle: string,
    command: 'add_track_to_playlist' | 'add_playlist_to_playlist',
    args: Record<string, unknown>,
  ) => {
    const target = playlistsStore.playlists.find((p) => p.id === targetPlaylistId)?.title ?? '';
    try {
      await invoke(command, args);
      notifications.addNotification(t('playlist.added', { item: itemTitle, target }), 'success');
    } catch (e) {
      console.error('Failed to add to playlist:', e);
      const message = errorMessage(e);
      // Backend errors carry no codes; anything unrecognized is shown as it is.
      if (message.includes('UNIQUE constraint failed')) {
        notifications.addNotification(t('playlist.alreadyIn', { item: itemTitle, target }), 'warning');
      } else if (message.includes('would create a cycle')) {
        notifications.addNotification(t('playlist.wouldCycle', { item: itemTitle, target }), 'warning');
      } else {
        notifications.addNotification(t('playlist.addFailed', { error: message }), 'error');
      }
    }
  };

  /** Saves a streaming album or playlist as a new playlist and nests it in the target. */
  const addContainerToPlaylist = async (
    targetPlaylistId: number,
    title: string,
    createChild: () => Promise<number>,
  ) => {
    let childPlaylistId: number;
    try {
      childPlaylistId = await createChild();
    } catch (e) {
      console.error('Failed to save container as a playlist:', e);
      notifications.addNotification(t('playlist.addFailed', { error: errorMessage(e) }), 'error');
      return;
    }
    await addToPlaylist(targetPlaylistId, title, 'add_playlist_to_playlist', {
      parentPlaylistId: targetPlaylistId,
      childPlaylistId,
    });
  };

  /** Queues every track of the playlist for download. Nested playlists are not followed. */
  const cachePlaylist = async (playlistId: number, title: string) => {
    notifications.addNotification(t('playlist.cacheStarted', { title }), 'info');
    try {
      const result = await invoke<PlaylistWithItemsDto | null>('get_playlist_with_tracks', { playlistId });
      if (!result) return;

      let done = 0;
      let failed = 0;
      for (const item of result.items) {
        if (item.itemType !== 'track') continue;
        try {
          await invoke('save_track_to_file', { track: item.track });
          done++;
        } catch (e) {
          console.error(`Failed to cache track ${item.track.title}:`, e);
          failed++;
        }
      }

      if (failed === 0) {
        notifications.addNotification(t('playlist.cacheQueued', { title, count: done }), 'success');
      } else {
        notifications.addNotification(t('playlist.cachePartial', { title, done, failed }), 'warning');
      }
    } catch (e) {
      console.error('Failed to cache playlist:', e);
      notifications.addNotification(t('playlist.cacheFailed', { error: errorMessage(e) }), 'error');
    }
  };

  /** Resolves to true when the playlist was deleted. */
  const deletePlaylist = async (playlistId: number, title: string) => {
    const confirmed = await confirmAction({
      title: t('playlist.deleteTitle', { title }),
      message: t('playlist.deleteMessage'),
      confirmLabel: t('common.delete'),
    });
    if (!confirmed) return false;

    try {
      await invoke('delete_playlist', { playlistId });
      return true;
    } catch (e) {
      console.error('Failed to delete playlist:', e);
      notifications.addNotification(t('common.error', { error: errorMessage(e) }), 'error');
      return false;
    }
  };

  return { addToPlaylist, addContainerToPlaylist, cachePlaylist, deletePlaylist };
}
