package com.nexus_player_frontend.app

import android.annotation.SuppressLint
import android.app.Dialog
import android.graphics.Bitmap
import android.util.Log
import android.view.ViewGroup
import android.view.Window
import android.view.WindowManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

private const val TAG = "SCChallenge"

@InvokeArg
class ChallengeArgs {
    lateinit var label: String
    lateinit var url: String
    var script: String = ""
}

@TauriPlugin
class SoundCloudChallengePlugin(private val activity: android.app.Activity) : Plugin(activity) {

    private var activeDialog: Dialog? = null

    @SuppressLint("SetJavaScriptEnabled")
    @Command
    fun open(invoke: Invoke) {
        Log.d(TAG, "openChallengeWindow called")
        val args = try {
            invoke.parseArgs(ChallengeArgs::class.java)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse args: $e")
            invoke.reject("Failed to parse args: $e")
            return
        }
        Log.d(TAG, "Opening challenge for url=${args.url} label=${args.label}")

        activity.runOnUiThread {
            try {
                activeDialog?.dismiss()
                activeDialog = null

                val dialog = Dialog(activity, android.R.style.Theme_Black_NoTitleBar_Fullscreen)
                dialog.requestWindowFeature(Window.FEATURE_NO_TITLE)
                dialog.window?.setLayout(
                    WindowManager.LayoutParams.MATCH_PARENT,
                    WindowManager.LayoutParams.MATCH_PARENT
                )

                val webView = WebView(activity)
                webView.layoutParams = ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
                webView.settings.apply {
                    javaScriptEnabled = true
                    domStorageEnabled = true
                    loadWithOverviewMode = true
                    useWideViewPort = true
                    userAgentString = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
                }

                // Enable WebView debugging (visible in chrome://inspect)
                WebView.setWebContentsDebuggingEnabled(true)

                var done = false
                val label = args.label

                webView.addJavascriptInterface(object : Any() {
                    @android.webkit.JavascriptInterface
                    fun onClientId(clientId: String, lbl: String) {
                        Log.d(TAG, "onClientId called: clientId=${clientId.take(8)}... label=$lbl done=$done")
                        if (done || clientId.isEmpty()) return
                        done = true
                        activity.runOnUiThread {
                            activeDialog?.dismiss()
                            activeDialog = null
                        }
                        val result = JSObject()
                        result.put("clientId", clientId)
                        result.put("label", lbl)
                        invoke.resolve(result)
                    }

                    @android.webkit.JavascriptInterface
                    fun log(msg: String) {
                        Log.d(TAG, "[JS] $msg")
                    }
                }, "_NexusScBridge")

                val interceptScript = buildInterceptScript(label)

                webView.webViewClient = object : WebViewClient() {
                    override fun onPageStarted(view: WebView, pageUrl: String, favicon: Bitmap?) {
                        Log.d(TAG, "onPageStarted: $pageUrl done=$done")
                        if (done) return
                        // Inject early — patches XHR/fetch before page JS runs
                        view.evaluateJavascript(interceptScript) { result ->
                            Log.d(TAG, "evaluateJavascript(onPageStarted) result=$result")
                        }
                    }

                    override fun onPageFinished(view: WebView, pageUrl: String) {
                        Log.d(TAG, "onPageFinished: $pageUrl done=$done")
                        if (done) return
                        // Re-inject as fallback in case onPageStarted was too early
                        view.evaluateJavascript(interceptScript) { result ->
                            Log.d(TAG, "evaluateJavascript(onPageFinished) result=$result")
                        }
                    }

                    override fun shouldOverrideUrlLoading(
                        view: WebView,
                        request: WebResourceRequest
                    ): Boolean {
                        Log.d(TAG, "shouldOverrideUrlLoading: ${request.url}")
                        return false
                    }
                }

                dialog.setOnDismissListener {
                    Log.d(TAG, "Dialog dismissed, done=$done")
                    activeDialog = null
                    if (!done) {
                        done = true
                        invoke.reject("cancelled")
                    }
                }

                webView.loadUrl(args.url)
                dialog.setContentView(webView)
                dialog.show()
                activeDialog = dialog
                Log.d(TAG, "Dialog shown successfully")
            } catch (e: Exception) {
                Log.e(TAG, "Error opening dialog: $e")
                invoke.reject("Error opening dialog: $e")
            }
        }
    }

    @Command
    fun close(invoke: Invoke) {
        Log.d(TAG, "closeChallengeWindow called")
        activity.runOnUiThread {
            activeDialog?.dismiss()
            activeDialog = null
        }
        invoke.resolve()
    }

    private fun buildInterceptScript(label: String): String {
        val labelJs = "\"${label.replace("\\", "\\\\").replace("\"", "\\\"")}\""
        return """
            (function() {
                _NexusScBridge.log('Script injected, watching=' + !!window._nexusScWatching);
                if (window._nexusScWatching) return;
                window._nexusScWatching = true;
                var label = $labelJs;

                function emitSolved(id, method) {
                    if (window._nexusScSolved) return;
                    window._nexusScSolved = true;
                    _NexusScBridge.log('client_id found via ' + method + ': ' + id.substring(0, 8) + '...');
                    try { _NexusScBridge.onClientId(id, label); } catch(e) {
                        _NexusScBridge.log('onClientId error: ' + e);
                    }
                }

                var oldOpen = XMLHttpRequest.prototype.open;
                XMLHttpRequest.prototype.open = function(method, url) {
                    var u = url ? url.toString() : '';
                    if (u.indexOf('client_id=') !== -1) {
                        var m = u.match(/client_id=([a-zA-Z0-9]{20,})/);
                        if (m) emitSolved(m[1], 'XHR');
                    }
                    return oldOpen.apply(this, arguments);
                };

                var oldFetch = window.fetch;
                window.fetch = function(input, init) {
                    var u = typeof input === 'string' ? input
                        : (input instanceof URL ? input.href
                        : (input && input.url ? input.url : ''));
                    if (u && u.indexOf('client_id=') !== -1) {
                        var m = u.match(/client_id=([a-zA-Z0-9]{20,})/);
                        if (m) emitSolved(m[1], 'Fetch');
                    }
                    return oldFetch.apply(this, arguments);
                };

                setInterval(function() {
                    if (window._nexusScSolved) return;
                    if (window.__sc_config && window.__sc_config.client_id) {
                        emitSolved(window.__sc_config.client_id, 'Config');
                    }
                }, 1000);

                _NexusScBridge.log('Intercept hooks installed');
            })();
        """.trimIndent()
    }
}
