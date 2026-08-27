// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/main/java/com/sentori/SentoriNotificationTap.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import android.app.Activity
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Bundle

/**
 * Taps, delivered without asking the host for anything.
 *
 * `handleNotificationTap` used to be the only way a tap reached
 * `register(onTap:)`, and its own comment said the host wires this in
 * `Activity.onCreate to forward the intent extras`. Nothing in the SDK
 * called it, the public docs never mentioned it, and so a native host
 * passed an `onTap` callback that could not fire. insight measured
 * that: the tray entry appeared, the app opened, and the callback
 * stayed silent. It is the same shape as the permission bug — a
 * premise carried over from the React Native integration, where the
 * host module did the forwarding, and quietly false everywhere else.
 *
 * There are two ways a tap can reach an app, and this covers both.
 *
 * **A notification the SDK posted.** The pending intent is ours, so
 * the tap arrives here with the message that produced it. Nothing can
 * intercept it and nothing needs to cooperate. This is the path the
 * Sentori server produces, because it sends `data` messages for
 * exactly this reason.
 *
 * **A notification the system posted**, from a `notification` message
 * some other sender emitted. The system opens the launcher activity
 * with the payload in the intent extras and never calls our service.
 * All that can be done is to read the intent the Activity was handed,
 * which `register` does. A tap that arrives while the app is already
 * running reaches the host's `onNewIntent`, and only a host that
 * calls `setIntent` makes it visible to anyone else — so that case
 * still needs the host, and the documentation says so rather than
 * pretending otherwise.
 */
object SentoriNotificationTap {

    /** Extras on our own pending intent. */
    const val EXTRA_MARKER = "com.sentori.tap"

    /**
     * FCM stamps this on anything it delivers. Its presence is how an
     * ordinary launch is told apart from a notification tap when the
     * system posted the notification.
     */
    private const val FCM_MESSAGE_ID = "google.message_id"

    private val lock = Any()
    private val seen = ArrayDeque<String>()
    private const val SEEN_CAP = 32

    /**
     * A tap is delivered once.
     *
     * The same intent is visible to `register` on a cold start and to
     * the pending intent that caused it, and an Activity that is
     * recreated on rotation hands back the intent it launched with.
     * Reporting a tap twice makes a host count one open as two.
     */
    private fun firstTime(id: String): Boolean = synchronized(lock) {
        if (seen.contains(id)) return false
        seen.addLast(id)
        while (seen.size > SEEN_CAP) seen.removeFirst()
        true
    }

    /** The pending intent attached to a notification the SDK posts. */
    fun pendingIntent(ctx: Context, data: Map<String, String>, requestCode: Int): PendingIntent? {
        // `getLaunchIntentForPackage` returns null more often than it
        // looks: an app with no exported launcher activity, and any
        // Robolectric host. The first version gave up there and
        // posted a notification with no content intent — a tray entry
        // that swallows the tap, which is worse than the bug this
        // file exists to fix, because it looks like it works.
        val launch = ctx.packageManager.getLaunchIntentForPackage(ctx.packageName)
            ?: Intent(Intent.ACTION_MAIN)
                .addCategory(Intent.CATEGORY_LAUNCHER)
                .setPackage(ctx.packageName)
        launch.flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
        launch.putExtra(EXTRA_MARKER, true)
        for ((k, v) in data) launch.putExtra(k, v)
        return PendingIntent.getActivity(
            ctx,
            requestCode,
            launch,
            // Immutable is required from Android 12 and correct
            // everywhere: nothing downstream should be able to rewrite
            // the payload this carries.
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    /**
     * Read whatever launched this Activity and report a tap if that is
     * what it was.
     *
     * Called by `register`, so a host that does nothing at all still
     * gets its `onTap`.
     */
    fun consume(activity: Activity?) {
        consume(activity?.intent?.extras)
    }

    /**
     * Report a tap from intent extras, if that is what they are.
     *
     * Public because the documentation asks hosts to call it. A tap
     * arriving while the app is already running reaches the host's
     * `onNewIntent` and nowhere else, and the docs said so — while
     * this object was `internal`, so the only reachable alternative
     * was `handleNotificationTap`, which records whatever it is
     * given. insight ended up copying the two-key test out of here by
     * hand, including a constant that exists only in this file.
     *
     * Safe to call with any intent's extras: an ordinary launch is
     * not a tap and is ignored, and the same tap is reported once.
     */
    fun consume(extras: Bundle?) {
        val bundle = extras ?: return
        val ours = bundle.getBoolean(EXTRA_MARKER, false)
        val fcm = bundle.containsKey(FCM_MESSAGE_ID)
        // An ordinary launch has neither, and reporting it as a tap
        // would put a notification in every session that never had
        // one.
        if (!ours && !fcm) return

        val payload = mutableMapOf<String, Any?>()
        for (key in bundle.keySet()) {
            if (key == EXTRA_MARKER) continue
            payload[key] = bundle.get(key)
        }
        val id = (bundle.getString(FCM_MESSAGE_ID) ?: payload["id"] as? String)
            ?: payload.toString()
        if (!firstTime(id)) return
        SentoriPushNotifications.handleNotificationTap(payload)
    }

    internal fun resetForTests() {
        synchronized(lock) { seen.clear() }
    }
}
