// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/main/java/com/sentori/SentoriFirebaseMessagingService.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
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
            // And tell the server, now. Buffering it in a field was
            // the whole of what happened here before, so a rotation
            // was invisible until the host next called `register` —
            // and the device received nothing in between.
            SentoriPush.handleRotatedToken(applicationContext, token)
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

            // A `data` message is invisible until somebody draws it,
            // and the Sentori server sends `data` on purpose so that
            // the tap comes back through a pending intent we own. If
            // the system already drew this one — a `notification`
            // message from some other sender — leave it alone rather
            // than showing it twice.
            if (message.notification == null) {
                SentoriPushNotifications.postNotification(applicationContext, message.data)
            }
        } catch (_: Throwable) {
            // never crash a Firebase callback
        }
    }
}
