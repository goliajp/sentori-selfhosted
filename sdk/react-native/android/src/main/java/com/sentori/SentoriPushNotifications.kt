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
    fun requestPermission(activity: Activity?, completion: (String) -> Unit) {
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
        pendingPermissionCallback = completion
        ActivityCompat.requestPermissions(
            ctx,
            arrayOf(Manifest.permission.POST_NOTIFICATIONS),
            PERMISSION_REQUEST_CODE,
        )
    }

    /**
     * Hook for the host Activity's `onRequestPermissionsResult`. Not
     * mandatory — Android dispatches the result back to the same
     * Activity that requested it, but ActivityCompat's flow doesn't
     * give us a callback API on older devices. Hosts that want
     * deterministic delivery can call this from their override.
     *
     * The Activity-based ActivityResultLauncher pattern would be
     * cleaner but requires the Activity to be a ComponentActivity;
     * we stick with ActivityCompat for broader RN host compat and
     * accept that the callback may not fire on every device — the
     * JS drain loop will still pick up the `granted` state next tick.
     */
    @JvmStatic
    fun handlePermissionResult(requestCode: Int, grantResults: IntArray) {
        if (requestCode != PERMISSION_REQUEST_CODE) return
        val cb = pendingPermissionCallback ?: return
        pendingPermissionCallback = null
        val granted = grantResults.isNotEmpty() &&
            grantResults[0] == PackageManager.PERMISSION_GRANTED
        cb(if (granted) "granted" else "denied")
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

    /** Called when the user taps a notification (host wires this in
     *  Activity.onCreate to forward the intent extras). */
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
