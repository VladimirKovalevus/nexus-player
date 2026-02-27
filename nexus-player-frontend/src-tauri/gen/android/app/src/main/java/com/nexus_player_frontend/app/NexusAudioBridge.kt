package com.nexus_player_frontend.app

import android.content.Intent
import androidx.core.content.ContextCompat

/**
 * JavaScript bridge injected into the main Tauri WebView as "_NexusAudio".
 * Called from audio.ts to sync playback state with the Android Foreground Service.
 */
class NexusAudioBridge(private val activity: MainActivity) {

    /**
     * Called from JS whenever the audio state changes.
     * Starts (or updates) the foreground service with the current track info.
     */
    @android.webkit.JavascriptInterface
    fun updateState(title: String, artist: String, isPlaying: Boolean) {
        val intent = Intent(activity, AudioForegroundService::class.java).apply {
            action = AudioForegroundService.ACTION_UPDATE
            putExtra(AudioForegroundService.EXTRA_TITLE, title)
            putExtra(AudioForegroundService.EXTRA_ARTIST, artist)
            putExtra(AudioForegroundService.EXTRA_IS_PLAYING, isPlaying)
        }
        ContextCompat.startForegroundService(activity, intent)
    }

    /**
     * Called from JS when playback stops completely.
     * Stops the foreground service and removes the notification.
     */
    @android.webkit.JavascriptInterface
    fun stopService() {
        val intent = Intent(activity, AudioForegroundService::class.java).apply {
            action = AudioForegroundService.ACTION_STOP_SERVICE
        }
        activity.startService(intent)
    }
}
