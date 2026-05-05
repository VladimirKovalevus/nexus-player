package com.nexus_player_frontend.app

import android.annotation.SuppressLint
import android.app.Dialog
import android.graphics.Bitmap
import android.util.Log
import android.view.ViewGroup
import android.view.Window
import android.view.WindowManager
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.webkit.CookieManager
import android.webkit.WebView
import android.webkit.WebViewClient
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class VkCaptchaArgs {
    lateinit var url: String
    var interactive: Boolean = true
}

/** Native Android host for VK ID captcha. All VK network/auth calls stay in Rust. */
@TauriPlugin
class VkLoginPlugin(private val activity: android.app.Activity) : Plugin(activity) {
    private val logTag = "NexusVkAuth"
    private var captchaActive = false

    @SuppressLint("SetJavaScriptEnabled", "JavascriptInterface")
    @Command
    fun openCaptcha(invoke: Invoke) {
        val args = invoke.parseArgs(VkCaptchaArgs::class.java)
        Log.i(logTag, "openCaptcha received; interactive=${args.interactive}")
        activity.runOnUiThread {
            if (captchaActive) {
                Log.w(logTag, "openCaptcha rejected because a dialog is already active")
                invoke.reject("VK captcha window is already open")
                return@runOnUiThread
            }
            captchaActive = true
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
                userAgentString = "VKAndroidApp/8.154-99999 (Android 12; SDK 32; arm64-v8a; Pixel 6; ru; 2960x1440)"
            }
            CookieManager.getInstance().setAcceptCookie(true)
            CookieManager.getInstance().setAcceptThirdPartyCookies(webView, true)

            var done = false
            fun resolve(token: String) {
                if (done || token.isBlank()) return
                Log.i(logTag, "captcha success token detected; resolving plugin invocation")
                done = true
                val result = JSObject()
                result.put("successToken", token)
                invoke.resolve(result)
                dialog.dismiss()
                webView.destroy()
            }

            webView.addJavascriptInterface(object : Any() {
                @android.webkit.JavascriptInterface
                fun onSuccessToken(token: String) {
                    Log.i(logTag, "success token received from WebView JavaScript bridge")
                    activity.runOnUiThread { resolve(token) }
                }
            }, "_NexusVkCaptcha")

            val interceptionScript = """
                (function () {
                  if (window.__nexusVkCaptchaInstalled) return;
                  window.__nexusVkCaptchaInstalled = true;
                  var sent = false;
                  function submit(value) {
                    if (sent || !value) return;
                    sent = true;
                    _NexusVkCaptcha.onSuccessToken(String(value));
                  }
                  function inspect(data) {
                    try {
                      var value = typeof data === 'string' ? JSON.parse(data) : data;
                      submit(value && value.response && value.response.success_token);
                      submit(value && value.success_token);
                      submit(value && value.token);
                    } catch (_) {}
                  }
                  var oldFetch = window.fetch;
                  window.fetch = async function () {
                    var response = await oldFetch.apply(this, arguments);
                    try { inspect(await response.clone().json()); } catch (_) {}
                    return response;
                  };
                  var oldOpen = XMLHttpRequest.prototype.open;
                  XMLHttpRequest.prototype.open = function () {
                    this.addEventListener('load', function () { inspect(this.responseText); });
                    return oldOpen.apply(this, arguments);
                  };
                  var timer = setInterval(function () {
                    if (window.VKCaptcha) {
                      window.VKCaptcha.onResult = function (result) {
                        inspect(result);
                        if (typeof result === 'string') submit(result);
                      };
                      clearInterval(timer);
                    }
                  }, 250);
                })();
            """.trimIndent()

            webView.webViewClient = object : WebViewClient() {
                override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
                    Log.d(logTag, "captcha page started: ${android.net.Uri.parse(url).host ?: "unknown-host"}")
                    view.evaluateJavascript(interceptionScript, null)
                }

                override fun onPageFinished(view: WebView, url: String) {
                    Log.d(logTag, "captcha page finished: ${android.net.Uri.parse(url).host ?: "unknown-host"}")
                    view.evaluateJavascript(interceptionScript, null)
                }
            }

            dialog.setOnDismissListener {
                Log.i(logTag, "captcha dialog dismissed; completed=$done")
                captchaActive = false
                if (!done) {
                    done = true
                    invoke.reject("cancelled")
                    webView.destroy()
                }
            }
            if (args.interactive) {
                dialog.setContentView(webView)
            } else {
                val layout = LinearLayout(activity).apply {
                    orientation = LinearLayout.VERTICAL
                    addView(webView, LinearLayout.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f
                    ))
                }
                val answer = EditText(activity).apply { hint = "Текст с картинки" }
                val submit = Button(activity).apply {
                    text = "Отправить"
                    setOnClickListener { resolve(answer.text.toString()) }
                }
                layout.addView(answer)
                layout.addView(submit)
                dialog.setContentView(layout)
            }
            dialog.show()
            Log.i(logTag, "captcha dialog shown; loading URL")
            webView.loadUrl(args.url)
        }
    }
}
