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
            ContextCompat.RECEIVER_NOT_EXPORTED
        )
    }

    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        mainWebView = webView
        pluginManager.load(webView, "vkLogin", VkLoginPlugin(this), "{}")
        webView.addJavascriptInterface(NexusAudioBridge(this), "_NexusAudio")
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        val data = intent.data ?: return

        val fragment = data.fragment ?: ""
        val query = data.query ?: ""

        val token = (fragment + "&" + query)
            .split("&")
            .firstOrNull { it.startsWith("access_token=") }
            ?.substringAfter("access_token=")
            ?: ""

        if (token.isNotEmpty()) {
            VkLoginPlugin.pendingTokenCallback?.invoke(token)
            VkLoginPlugin.pendingTokenCallback = null
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        try { unregisterReceiver(mediaControlReceiver) } catch (_: Exception) {}
        mainWebView = null
    }
}
