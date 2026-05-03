package com.nexus_player_frontend.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.provider.MediaStore
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject

class NexusLibraryBridge(private val activity: MainActivity) {
    private val permission: String
        get() = if (Build.VERSION.SDK_INT >= 33) Manifest.permission.READ_MEDIA_AUDIO else Manifest.permission.READ_EXTERNAL_STORAGE

    @android.webkit.JavascriptInterface
    fun hasPermission(): Boolean = ContextCompat.checkSelfPermission(activity, permission) == PackageManager.PERMISSION_GRANTED

    @android.webkit.JavascriptInterface
    fun requestPermission() {
        ActivityCompat.requestPermissions(activity, arrayOf(permission), 4102)
    }

    @android.webkit.JavascriptInterface
    fun scanAudio(): String {
        if (!hasPermission()) return "[]"
        val result = JSONArray()
        val projection = arrayOf(
            MediaStore.Audio.Media._ID,
            MediaStore.Audio.Media.DATA,
            MediaStore.Audio.Media.SIZE,
            MediaStore.Audio.Media.DATE_MODIFIED
        )
        val selection = "${MediaStore.Audio.Media.IS_MUSIC} != 0"
        activity.contentResolver.query(MediaStore.Audio.Media.EXTERNAL_CONTENT_URI, projection, selection, null, null)?.use { cursor ->
            val idColumn = cursor.getColumnIndexOrThrow(MediaStore.Audio.Media._ID)
            val pathColumn = cursor.getColumnIndexOrThrow(MediaStore.Audio.Media.DATA)
            val sizeColumn = cursor.getColumnIndexOrThrow(MediaStore.Audio.Media.SIZE)
            val modifiedColumn = cursor.getColumnIndexOrThrow(MediaStore.Audio.Media.DATE_MODIFIED)
            while (cursor.moveToNext()) {
                val path = cursor.getString(pathColumn) ?: continue
                result.put(JSONObject().apply {
                    put("sourceKey", "mediastore:${cursor.getLong(idColumn)}")
                    put("path", path)
                    put("size", cursor.getLong(sizeColumn))
                    put("modifiedAt", cursor.getLong(modifiedColumn) * 1000)
                })
            }
        }
        return result.toString()
    }
}
