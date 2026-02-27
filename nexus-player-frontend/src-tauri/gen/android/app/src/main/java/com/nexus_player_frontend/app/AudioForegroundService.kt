package com.nexus_player_frontend.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.media.app.NotificationCompat as MediaNotificationCompat

class AudioForegroundService : Service() {

    companion object {
        const val CHANNEL_ID = "nexus_player_audio"
        const val NOTIFICATION_ID = 1

        const val ACTION_UPDATE = "nexus.UPDATE_STATE"
        const val ACTION_PLAY = "nexus.PLAY"
        const val ACTION_PAUSE = "nexus.PAUSE"
        const val ACTION_NEXT = "nexus.NEXT"
        const val ACTION_PREV = "nexus.PREV"
        const val ACTION_STOP_SERVICE = "nexus.STOP_SERVICE"

        const val EXTRA_TITLE = "title"
        const val EXTRA_ARTIST = "artist"
        const val EXTRA_IS_PLAYING = "isPlaying"

        var currentTitle: String = "Nexus Player"
        var currentArtist: String = ""
        var currentIsPlaying: Boolean = false
    }

    private lateinit var notificationManager: NotificationManager

    override fun onCreate() {
        super.onCreate()
        notificationManager = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_UPDATE -> {
                currentTitle = intent.getStringExtra(EXTRA_TITLE) ?: currentTitle
                currentArtist = intent.getStringExtra(EXTRA_ARTIST) ?: currentArtist
                currentIsPlaying = intent.getBooleanExtra(EXTRA_IS_PLAYING, currentIsPlaying)
                startForegroundCompat()
                notificationManager.notify(NOTIFICATION_ID, buildNotification())
            }
            ACTION_STOP_SERVICE -> {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                    stopForeground(STOP_FOREGROUND_REMOVE)
                } else {
                    @Suppress("DEPRECATION")
                    stopForeground(true)
                }
                stopSelf()
                return START_NOT_STICKY
            }
            else -> {
                startForegroundCompat()
            }
        }
        return START_STICKY
    }

    private fun startForegroundCompat() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                NOTIFICATION_ID,
                buildNotification(),
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK
            )
        } else {
            startForeground(NOTIFICATION_ID, buildNotification())
        }
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Audio Playback",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Nexus Player audio playback controls"
                setShowBadge(false)
            }
            notificationManager.createNotificationChannel(channel)
        }
    }

    private fun actionIntent(action: String, requestCode: Int): PendingIntent {
        val i = Intent(action).setPackage(packageName)
        return PendingIntent.getBroadcast(
            this, requestCode, i,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
    }

    private fun buildNotification(): Notification {
        val openAppIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        val openAppPending = PendingIntent.getActivity(
            this, 0, openAppIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val playPauseAction = if (currentIsPlaying) {
            NotificationCompat.Action(
                android.R.drawable.ic_media_pause, "Pause",
                actionIntent(ACTION_PAUSE, 2)
            )
        } else {
            NotificationCompat.Action(
                android.R.drawable.ic_media_play, "Play",
                actionIntent(ACTION_PLAY, 2)
            )
        }

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(currentTitle)
            .setContentText(currentArtist.ifEmpty { null })
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentIntent(openAppPending)
            .setOngoing(true)
            .setShowWhen(false)
            .addAction(
                android.R.drawable.ic_media_previous, "Previous",
                actionIntent(ACTION_PREV, 1)
            )
            .addAction(playPauseAction)
            .addAction(
                android.R.drawable.ic_media_next, "Next",
                actionIntent(ACTION_NEXT, 3)
            )
            .setStyle(
                MediaNotificationCompat.MediaStyle()
                    .setShowActionsInCompactView(0, 1, 2)
            )
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .build()
    }

    override fun onBind(intent: Intent?): IBinder? = null
}
