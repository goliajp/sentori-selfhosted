// v2.10 — Android push notification bridge.
//
// Mirrors the iOS shape:
//   * Static singleton owning 32-slot FIFO buffers for token,
//     foreground notifications, and tap responses.
//   * JS drains via `drainState()` at 1 Hz.
//   * No EventEmitter — the existing crash-handler pattern.
//
// FCM-specific:
//   * `firebase-messaging` is a `compileOnly` dep so non-push hosts
//     pay nothing. Runtime gate via `Class.forName` before any
//     Firebase call.
//   * Token retrieval / refresh routes through
//     `SentoriFirebaseMessagingService.onNewToken` (system-initiated)
//     and `FirebaseMessaging.getInstance().token` (caller-initiated).
//
// Android 13+ (API 33) added `POST_NOTIFICATIONS` as a runtime
// permission. We surface it via `requestPermission(activity, cb)`;
// older Android resolves immediately to `granted` (system grants at
// install time; user can still disable it in Settings, which we
// detect via `NotificationManagerCompat.areNotificationsEnabled`).

package com.sentori

import android.Manifest
import android.app.Activity
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

object SentoriPushNotifications {
    private const val DEFAULT_CHANNEL_ID = "sentori"
    private const val DEFAULT_CHANNEL_NAME = "Sentori notifications"
    private const val BUFFER_CAP = 32
    private const val PERMISSION_REQUEST_CODE = 0x5E70_3001.toInt()

    private val lock = Any()
    private var tokenHex: String? = null
    private var registrationError: String? = null
    private val notifications = mutableListOf<Map<String, Any?>>()
    private val taps = mutableListOf<Map<String, Any?>>()

    private var pendingPermissionCallback: ((String) -> Unit)? = null

    private val watcher = java.util.concurrent.Executors.newSingleThreadExecutor { r ->
        Thread(r, "sentori-push-permission").apply { isDaemon = true }
    }

    // ── status / permission ─────────────────────────────────────

    /** Returns `granted` / `denied` / `notDetermined` without
     *  prompting. Mirrors the iOS string return. */
    @JvmStatic
    fun currentPermission(ctx: Context): String {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val status = ContextCompat.checkSelfPermission(
                ctx,
                Manifest.permission.POST_NOTIFICATIONS,
            )
            if (status == PackageManager.PERMISSION_GRANTED) return "granted"
            // Permission has been explicitly denied or never requested.
            // The framework distinguishes these only via
            // `shouldShowRequestPermissionRationale` which needs an
            // Activity; without one we conservatively report
            // `notDetermined`.
            return "notDetermined"
        }
        // Pre-Android 13: install-time permission. The user can
        // still disable notifications per-app; we surface that.
        val enabled = NotificationManagerCompat.from(ctx).areNotificationsEnabled()
        return if (enabled) "granted" else "denied"
    }

    /**
     * Requests POST_NOTIFICATIONS on Android 13+ (no-op on older
     * Android — they auto-grant + the SDK resolves immediately).
     *
     * Callbacks run on the main thread.
     */
    @JvmStatic
    @JvmOverloads
    fun requestPermission(
        activity: Activity?,
        timeoutMs: Long = 60_000,
        completion: (String) -> Unit,
    ) {
        val ctx = activity ?: run {
            completion("error:no-activity")
            return
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            completion(currentPermission(ctx))
            return
        }
        val current = ContextCompat.checkSelfPermission(
            ctx,
            Manifest.permission.POST_NOTIFICATIONS,
        )
        if (current == PackageManager.PERMISSION_GRANTED) {
            completion("granted")
            return
        }
        synchronized(lock) { pendingPermissionCallback = completion }
        ActivityCompat.requestPermissions(
            ctx,
            arrayOf(Manifest.permission.POST_NOTIFICATIONS),
            PERMISSION_REQUEST_CODE,
        )
        watchForPermission(ctx.applicationContext, timeoutMs)
    }

    /**
     * Fast path for hosts that forward `onRequestPermissionsResult`.
     *
     * It used to be the *only* path, and the comment here said the
     * callback might not fire because "the JS drain loop will still
     * pick up the granted state next tick". That is true of React
     * Native and of nothing else. A native host calls `register` and
     * never hears of this method, so the callback was never invoked,
     * `finishRegister` never ran, and a first launch registered
     * nothing at all — silently, with the user having tapped Allow.
     * The next launch worked, because by then the permission was
     * already granted and the flow never suspended. New users simply
     * did not get push until they happened to reopen the app.
     *
     * Nothing needs to call this now. It stays because forwarding the
     * result resolves in milliseconds instead of on the next poll,
     * and because hosts that already call it should keep working.
     */
    @JvmStatic
    fun handlePermissionResult(requestCode: Int, grantResults: IntArray) {
        if (requestCode != PERMISSION_REQUEST_CODE) return
        val granted = grantResults.isNotEmpty() &&
            grantResults[0] == PackageManager.PERMISSION_GRANTED
        settlePermission(if (granted) "granted" else "denied")
    }

    /** Deliver a pending permission outcome exactly once. */
    private fun settlePermission(status: String) {
        val cb = synchronized(lock) {
            pendingPermissionCallback.also { pendingPermissionCallback = null }
        } ?: return
        cb(status)
    }

    /**
     * Watch the permission the framework actually holds, and settle
     * when it changes.
     *
     * `checkSelfPermission` flips the moment the user answers the
     * dialog, whether or not anyone forwarded the result — so this
     * needs no cooperation from the host, works on any Activity
     * rather than only a `ComponentActivity`, and cannot be defeated
     * by a host that does not know the hook exists.
     *
     * On a background thread, because the caller's thread is never
     * ours to block. If the user never answers, this settles as
     * `notDetermined` at the deadline rather than hanging: a
     * registration that reports nothing is the failure this whole
     * change is about.
     */
    private fun watchForPermission(ctx: Context, timeoutMs: Long) {
        watcher.execute {
            val deadline = System.currentTimeMillis() + timeoutMs
            while (System.currentTimeMillis() < deadline) {
                if (synchronized(lock) { pendingPermissionCallback } == null) return@execute
                if (ContextCompat.checkSelfPermission(
                        ctx,
                        Manifest.permission.POST_NOTIFICATIONS,
                    ) == PackageManager.PERMISSION_GRANTED
                ) {
                    settlePermission("granted")
                    return@execute
                }
                try {
                    Thread.sleep(250)
                } catch (_: InterruptedException) {
                    return@execute
                }
            }
            // Denied and never-answered are the same observation from
            // here: the framework only tells them apart through an
            // Activity that may be gone by now. Say what was seen.
            settlePermission("notDetermined")
        }
    }

    // ── register / unregister ───────────────────────────────────

    /**
     * Kick off FCM token retrieval. The result lands in the buffer
     * (drained by JS) — either via this caller-initiated path or
     * via `SentoriFirebaseMessagingService.onNewToken`, whichever
     * fires first.
     *
     * Silently no-ops when `firebase-messaging` isn't on the
     * classpath — the SDK shipped a `compileOnly` dep, so a host
     * without push runs through this path without throwing.
     */
    @JvmStatic
    fun registerForRemoteNotifications(ctx: Context) {
        ensureChannel(ctx)
        if (!isFirebaseAvailable()) {
            handleRegistrationFailure("firebase-messaging not available")
            return
        }
        try {
            // Call FirebaseMessaging.getInstance().getToken() via
            // reflection so the SDK's bytecode doesn't reference the
            // Firebase classes directly (allows non-push hosts to
            // skip Firebase entirely without LinkageError).
            val cls = Class.forName("com.google.firebase.messaging.FirebaseMessaging")
            val instance = cls.getMethod("getInstance").invoke(null)
            val tokenTask = cls.getMethod("getToken").invoke(instance)
            val taskCls = Class.forName("com.google.android.gms.tasks.Task")
            val listenerCls = Class.forName("com.google.android.gms.tasks.OnCompleteListener")
            val listener = java.lang.reflect.Proxy.newProxyInstance(
                listenerCls.classLoader,
                arrayOf(listenerCls),
            ) { _, method, args ->
                if (method.name == "onComplete") {
                    val task = args?.firstOrNull() ?: return@newProxyInstance null
                    val taskClass = task.javaClass
                    val successful = taskClass.getMethod("isSuccessful").invoke(task) as Boolean
                    if (successful) {
                        val tok = taskClass.getMethod("getResult").invoke(task) as? String
                        if (tok != null) handleRegisteredToken(tok)
                    } else {
                        val ex = taskClass.getMethod("getException").invoke(task) as? Throwable
                        handleRegistrationFailure(ex?.localizedMessage ?: "fcm token request failed")
                    }
                }
                null
            }
            taskCls.getMethod("addOnCompleteListener", listenerCls).invoke(tokenTask, listener)
        } catch (e: Throwable) {
            handleRegistrationFailure(e.localizedMessage ?: e.javaClass.simpleName)
        }
    }

    /** Counterpart — calls `FirebaseMessaging.deleteToken()` via
     *  reflection. Best-effort; failures are swallowed. */
    @JvmStatic
    fun unregisterForRemoteNotifications(ctx: Context) {
        synchronized(lock) {
            tokenHex = null
            registrationError = null
        }
        if (!isFirebaseAvailable()) return
        try {
            val cls = Class.forName("com.google.firebase.messaging.FirebaseMessaging")
            val instance = cls.getMethod("getInstance").invoke(null)
            cls.getMethod("deleteToken").invoke(instance)
        } catch (_: Throwable) {
            // best-effort
        }
    }

    // ── service-callable mutators ───────────────────────────────

    /** Called from `SentoriFirebaseMessagingService.onNewToken`. */
    @JvmStatic
    fun handleRegisteredToken(token: String) {
        synchronized(lock) {
            tokenHex = token
            registrationError = null
        }
    }

    @JvmStatic
    fun handleRegistrationFailure(reason: String) {
        synchronized(lock) {
            registrationError = reason
        }
    }

    /** Called from `SentoriFirebaseMessagingService.onMessageReceived`.
     *  `payload` is the keyset extracted from the FCM RemoteMessage —
     *  see the service for the shape. */
    @JvmStatic
    fun handleIncomingNotification(payload: Map<String, Any?>) {
        synchronized(lock) {
            notifications.add(payload)
            while (notifications.size > BUFFER_CAP) notifications.removeAt(0)
        }
    }

    /**
     * Record a tap.
     *
     * This used to be the only route to `register(onTap:)`, and its
     * comment said the host wires it in `Activity.onCreate`. Nothing
     * in the SDK called it and the docs never mentioned it, so a
     * native host's `onTap` could not fire at all. `SentoriNotificationTap`
     * now reaches this on its own, from a pending intent the SDK owns
     * or from the intent an Activity was launched with; this stays
     * public because a host that already forwards should keep working.
     */
    @JvmStatic
    fun handleNotificationTap(extras: Map<String, Any?>) {
        synchronized(lock) {
            taps.add(extras)
            while (taps.size > BUFFER_CAP) taps.removeAt(0)
        }
    }

    // ── drain (called by Expo AsyncFunction) ───────────────────

    @JvmStatic
    fun drainState(): Map<String, Any?> {
        synchronized(lock) {
            val tok = tokenHex
            val err = registrationError
            val nList = notifications.toList()
            val tList = taps.toList()
            notifications.clear()
            taps.clear()
            val map = mutableMapOf<String, Any?>(
                "notifications" to nList,
                "taps" to tList,
            )
            if (tok != null) map["token"] = tok
            if (err != null) map["error"] = err
            return map
        }
    }

    // ── helpers ────────────────────────────────────────────────

    /**
     * Create the default notification channel idempotently. Android
     * 8+ requires every visible notification to belong to a channel;
     * we provide a sensible "sentori" channel for hosts that don't
     * register one themselves. Hosts that want their own channel
     * pass `channelId` in the SDK push send options.
     */
    private fun ensureChannel(ctx: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val mgr = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager
            ?: return
        if (mgr.getNotificationChannel(DEFAULT_CHANNEL_ID) != null) return
        val channel = NotificationChannel(
            DEFAULT_CHANNEL_ID,
            DEFAULT_CHANNEL_NAME,
            NotificationManager.IMPORTANCE_DEFAULT,
        )
        mgr.createNotificationChannel(channel)
    }

    /**
     * Put a data message in the tray, with a pending intent of ours.
     *
     * Nothing here posted a notification before, which left both
     * kinds of message broken in a different way. A `data` message
     * reached `onMessage` and the user saw nothing — insight's device
     * said "No notifications" while the callback had fired. A
     * `notification` message was drawn by the system, which never
     * calls this service, so the tap had no way home. Posting it here
     * is what makes one path work end to end.
     *
     * Only messages that carry something to display are posted. A
     * data message without a title or a body is a silent instruction
     * to the app, and an app that uses those would not thank us for
     * turning each one into a notification the user has to dismiss.
     */
    @JvmStatic
    fun postNotification(ctx: Context, data: Map<String, String>) {
        val title = data["title"]?.takeIf { it.isNotBlank() }
        val body = data["body"]?.takeIf { it.isNotBlank() }
        if (title == null && body == null) return
        if (!NotificationManagerCompat.from(ctx).areNotificationsEnabled()) return

        ensureChannel(ctx)
        val id = (data["google.message_id"] ?: data["id"] ?: title ?: "").hashCode()
        val builder = androidx.core.app.NotificationCompat.Builder(
            ctx,
            data["channelId"]?.takeIf { it.isNotBlank() } ?: DEFAULT_CHANNEL_ID,
        )
            .setSmallIcon(smallIcon(ctx))
            .setAutoCancel(true)
            .setPriority(androidx.core.app.NotificationCompat.PRIORITY_DEFAULT)
        title?.let { builder.setContentTitle(it) }
        body?.let { builder.setContentText(it) }
        SentoriNotificationTap.pendingIntent(ctx, data, id)?.let { builder.setContentIntent(it) }

        try {
            NotificationManagerCompat.from(ctx).notify(id, builder.build())
        } catch (t: Throwable) {
            // Losing the tray entry is not worth taking the host down
            // for — but losing it in silence is what made this cost
            // insight an afternoon. The channel got created, so they
            // knew the code had reached `notify`; the tray was empty,
            // `dumpsys notification` had nothing, and logcat had not
            // one word. They worked backwards from the channel's
            // existence to a throw nobody had reported.
            //
            // The argument for this line is the same one made when
            // registration failures started logging, one release
            // earlier — and then this catch was written silent in the
            // same release. A failure the host cannot see is the
            // hardest kind there is.
            android.util.Log.w("sentori", "push notification not shown: $t")
        }
    }

    /**
     * A small icon that will not throw.
     *
     * The host's own, because a push SDK that ships its own artwork
     * puts a stranger's mark in someone else's tray. But
     * `applicationInfo.icon` is `0` for an app that declares no
     * `android:icon` — insight's did not, it drew its logo from a
     * launch theme — and `notify` throws on an icon of 0 rather than
     * drawing something ugly. The result was a notification that
     * silently never appeared.
     *
     * A system icon in that case: unbranded and plain, but visible,
     * and visible is the whole point of the tray.
     */
    private fun smallIcon(ctx: Context): Int {
        val declared = try {
            ctx.applicationInfo.icon
        } catch (_: Throwable) {
            0
        }
        return if (declared != 0) declared else android.R.drawable.ic_dialog_info
    }

    private fun isFirebaseAvailable(): Boolean {
        return try {
            Class.forName("com.google.firebase.messaging.FirebaseMessaging")
            true
        } catch (_: ClassNotFoundException) {
            false
        } catch (_: NoClassDefFoundError) {
            false
        }
    }
}
