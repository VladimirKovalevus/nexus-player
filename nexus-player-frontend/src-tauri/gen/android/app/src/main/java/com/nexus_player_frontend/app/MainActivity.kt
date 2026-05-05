package com.nexus_player_frontend.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.content.ContextCompat

class MainActivity : TauriActivity() {

    private var mainWebView: WebView? = null

    // Receives media control broadcasts from the notification buttons
    private val mediaControlReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val command = when (intent.action) {
                AudioForegroundService.ACTION_PLAY -> "resume"
                AudioForegroundService.ACTION_PAUSE -> "pause"
                AudioForegroundService.ACTION_NEXT -> "play_next"
                AudioForegroundService.ACTION_PREV -> "play_previous"
                else -> return
            }
            mainWebView?.post {
                mainWebView?.evaluateJavascript(
                    "window.__nexusControl && window.__nexusControl('$command')",
                    null
                )
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        val filter = IntentFilter().apply {
            addAction(AudioForegroundService.ACTION_PLAY)
            addAction(AudioForegroundService.ACTION_PAUSE)
            addAction(AudioForegroundService.ACTION_NEXT)
            addAction(AudioForegroundService.ACTION_PREV)
        }
        ContextCompat.registerReceiver(
            this, mediaControlReceiver, filter,
            ContextCompat.RECEIVER_EXPORTED
        )
    }

    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        mainWebView = webView
        pluginManager.load(webView, "vkLogin", VkLoginPlugin(this), "{}")
        pluginManager.load(webView, "soundCloudChallenge", SoundCloudChallengePlugin(this), "{}")
        webView.addJavascriptInterface(NexusAudioBridge(this), "_NexusAudio")
        webView.addJavascriptInterface(NexusLibraryBridge(this), "_NexusLibrary")
    }

    override fun onDestroy() {
        super.onDestroy()
        try { unregisterReceiver(mediaControlReceiver) } catch (_: Exception) {}
        mainWebView = null
    }
}
