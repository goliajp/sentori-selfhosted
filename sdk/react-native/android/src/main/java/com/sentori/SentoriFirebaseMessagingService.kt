// v2.10 — FCM message routing service.
//
// Manifest-registered in `AndroidManifest.xml`. Firebase's manifest
// merger picks this up via the `MESSAGING_EVENT` intent filter.
// Three responsibilities:
//
//   * `onNewToken` — push the refreshed FCM token into
//     `SentoriPushNotifications.handleRegisteredToken`.
//   * `onMessageReceived` — extract a `Map<String, Any?>` payload
//     from the `RemoteMessage` and route it to
//     `SentoriPushNotifications.handleIncomingNotification`. Whether
//     the system also displays the notification tray entry depends
//     on `notification` vs `data`-only messages — we always surface
//     it to JS regardless.
//
// `firebase-messaging` is `compileOnly` in `build.gradle`. The class
// compiles against Firebase but isn't loaded at runtime unless the
// host app pulls in `firebase-messaging` themselves. The
// AndroidManifest `<service>` declaration is harmless when Firebase
// isn't on the classpath — it just never gets invoked.

package com.sentori

import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage

class SentoriFirebaseMessagingService : FirebaseMessagingService() {

    override fun onNewToken(token: String) {
        super.onNewToken(token)
        try {
            SentoriPushNotifications.handleRegisteredToken(token)
        } catch (_: Throwable) {
            // never crash a Firebase callback
        }
    }

    override fun onMessageReceived(message: RemoteMessage) {
        super.onMessageReceived(message)
        try {
            val payload = mutableMapOf<String, Any?>(
                "id" to (message.messageId ?: ""),
                "userInfo" to message.data,
                "receivedAt" to (message.sentTime / 1000.0),
            )
            message.notification?.let { notif ->
                notif.title?.let { payload["title"] = it }
                notif.body?.let { payload["body"] = it }
                notif.channelId?.let { payload["channelId"] = it }
            }
            SentoriPushNotifications.handleIncomingNotification(payload)
        } catch (_: Throwable) {
            // never crash a Firebase callback
        }
    }
}
