package com.nexus_player_frontend.app

import android.annotation.SuppressLint
import android.app.Dialog
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.view.ViewGroup
import android.view.Window
import android.view.WindowManager
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class OpenArgs {
    lateinit var url: String
}

@TauriPlugin
class VkLoginPlugin(private val activity: android.app.Activity) : Plugin(activity) {

    companion object {
        // Called from MainActivity.onNewIntent when VK returns via deep link
        var pendingTokenCallback: ((String) -> Unit)? = null
    }

    @SuppressLint("SetJavaScriptEnabled")
    @Command
    fun open(invoke: Invoke) {
        val args = invoke.parseArgs(OpenArgs::class.java)
        val url = args.url

        activity.runOnUiThread {
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
                userAgentString = "Mozilla/5.0 (Linux; Android 10; Mobile) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36"
            }

            CookieManager.getInstance().setAcceptCookie(true)
            CookieManager.getInstance().setAcceptThirdPartyCookies(webView, true)

            var done = false

            fun handleToken(token: String) {
                if (done) return
                done = true
                pendingTokenCallback = null
                dialog.dismiss()
                val result = JSObject()
                result.put("token", token)
                invoke.resolve(result)
            }

            // JS bridge: catches the token when VK's own JS navigates to blank.html#access_token=...
            // This fires even when the WebView is in background (while VK app is open)
            webView.addJavascriptInterface(object : Any() {
                @android.webkit.JavascriptInterface
                fun onToken(token: String) {
                    if (token.isNotEmpty()) {
                        activity.runOnUiThread { handleToken(token) }
                    }
                }
            }, "_NexusVkBridge")

            // Register deep link callback so MainActivity can deliver the token
            // when VK native app redirects back to our app
            pendingTokenCallback = { token -> handleToken(token) }

            webView.webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(
                    view: WebView,
                    request: WebResourceRequest
                ): Boolean {
                    val urlStr = request.url.toString()

                    if (urlStr.contains("access_token=")) {
                        handleToken(extractToken(urlStr))
                        return true
                    }

                    // Handle intent:// — VK wants to open the native app
                    if (urlStr.startsWith("intent://")) {
                        try {
                            val intent = Intent.parseUri(urlStr, Intent.URI_INTENT_SCHEME)
                            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                            try {
                                activity.startActivity(intent)
                            } catch (_: android.content.ActivityNotFoundException) {
                                // VK app not installed — open fallback via system (not WebView)
                                val fallback = intent.getStringExtra("browser_fallback_url")
                                if (fallback != null) {
                                    try {
                                        activity.startActivity(
                                            Intent(Intent.ACTION_VIEW, Uri.parse(fallback))
                                        )
                                    } catch (_: Exception) {}
                                }
                            }
                        } catch (_: Exception) {}
                        return true
                    }

                    // Handle market:// (Play Store)
                    if (urlStr.startsWith("market://")) {
                        try {
                            activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(urlStr)))
                        } catch (_: Exception) {
                            val webUrl = "https://play.google.com/store/apps/details" +
                                urlStr.substringAfter("market://details")
                            activity.startActivity(
                                Intent(Intent.ACTION_VIEW, Uri.parse(webUrl))
                            )
                        }
                        return true
                    }

                    return false
                }

                override fun onPageStarted(view: WebView, pageUrl: String, favicon: Bitmap?) {
                    // Catch fragment-based token in page URL (may be stripped in some cases)
                    if (pageUrl.contains("access_token=")) {
                        handleToken(extractToken(pageUrl))
                        return
                    }
                    // Inject JS that monitors hash changes — fires when VK's own JS
                    // navigates to blank.html#access_token=TOKEN (works while WebView is in background)
                    view.evaluateJavascript("""
                        (function() {
                            if (window._nexusVkWatching) return;
                            window._nexusVkWatching = true;
                            function checkHash() {
                                var h = window.location.hash || '';
                                if (h.indexOf('access_token=') !== -1) {
                                    var parts = h.replace('#','').split('&');
                                    for (var i = 0; i < parts.length; i++) {
                                        var kv = parts[i].split('=');
                                        if (kv[0] === 'access_token' && kv[1]) {
                                            _NexusVkBridge.onToken(kv[1]);
                                            return;
                                        }
                                    }
                                }
                            }
                            checkHash();
                            window.addEventListener('hashchange', checkHash);
                            setInterval(checkHash, 500);
                        })();
                    """, null)
                }
            }

            dialog.setOnDismissListener {
                pendingTokenCallback = null
                if (!done) {
                    done = true
                    invoke.reject("cancelled")
                }
            }

            webView.loadUrl(url)
            dialog.setContentView(webView)
            dialog.show()
        }
    }

    private fun extractToken(url: String): String {
        val fragment = url.substringAfter("#", "")
        if (fragment.isNotEmpty()) {
            fragment.split("&").forEach { param ->
                val kv = param.split("=", limit = 2)
                if (kv.size == 2 && kv[0] == "access_token") return kv[1]
            }
        }
        val query = url.substringAfter("?", "").substringBefore("#")
        if (query.isNotEmpty()) {
            query.split("&").forEach { param ->
                val kv = param.split("=", limit = 2)
                if (kv.size == 2 && kv[0] == "access_token") return kv[1]
            }
        }
        return ""
    }
}
