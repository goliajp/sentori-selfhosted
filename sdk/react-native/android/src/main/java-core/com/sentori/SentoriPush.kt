// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/main/java/com/sentori/SentoriPush.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import android.app.Activity
import android.content.Context
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import org.json.JSONObject

/**
 * Push, as an app writes it.
 *
 *     Sentori.push.register(activity) { result ->
 *         if (result is SentoriPush.Result.Failure) { … }
 *     }
 *
 * The pieces underneath already existed — the POST_NOTIFICATIONS
 * prompt, the FCM service, the token and tap buffers — and were
 * reachable only through the React Native bridge. What was missing
 * everywhere but JavaScript is the part that puts a token on the
 * server, which is the only reason a registered device is reachable
 * at all.
 *
 * Asked for by insight (2026-08-11): two apps with no React Native,
 * blocked, and unwilling to reimplement the HTTP contract because a
 * second implementation drifts silently on the one path nobody
 * watches.
 *
 * Callback-based rather than suspending, so this is usable from Java
 * and from a codebase that has not adopted coroutines. The callback
 * runs on a background thread; touch UI from it at your own risk.
 */
object SentoriPush {

    /**
     * Why a registration did not produce a device handle. Same five
     * names as `PushRegisterFailure` in the React Native SDK and the
     * Swift one, so a single set of integration notes covers all
     * three and an operator reading a support thread does not have to
     * translate.
     */
    enum class Failure(val reason: String) {
        /** [Sentori.start] has not run. A wiring bug. */
        NOT_INITIALISED("not-initialised"),
        /**
         * The user said no. Not an error: do not retry on a timer,
         * and offer it again from a settings screen.
         */
        PERMISSION_DENIED("permission-denied"),
        /**
         * No Firebase in this build, or no `google-services.json`.
         * Nothing to do at runtime.
         */
        NO_TRANSPORT("no-transport"),
        /**
         * FCM never handed back a token inside the window. Retrying
         * later is reasonable.
         */
        TOKEN_TIMEOUT("token-timeout"),
        /** Sentori answered non-2xx. Settings ▸ Push is where to look. */
        SERVER_REJECTED("server-rejected"),
    }

    /**
     * Registration never throws. A denied permission is an ordinary
     * answer, and an opt-in that throws inside someone's ViewModel is
     * the failure this SDK's contract with its host is written
     * against.
     */
    sealed class Result {
        /**
         * The `device_tokens` row id. Revoking takes it, and so does a
         * targeted send.
         */
        data class Success(val handle: String) : Result()

        data class Failure(val reason: SentoriPush.Failure, val message: String) : Result()
    }

    private val lock = Any()
    private var cachedHandle: String? = null
    private var onMessage: ((Map<String, Any?>) -> Unit)? = null
    private var onTap: ((Map<String, Any?>) -> Unit)? = null
    private var drainTask: ScheduledFuture<*>? = null

    private const val PREFS = "com.sentori.push"
    private const val HANDLE_KEY = "handle"

    /**
     * Which installation this is, kept for as long as the app is
     * installed.
     *
     * The server keys the device row on it, so a vendor rotating its
     * token becomes an update of the row that already exists rather
     * than a new row with a new address. Before this, a rotation
     * silently retired whatever `spToken` a backend was holding.
     *
     * Generated here rather than issued by the server because it has
     * to exist before the first registration, and it never leaves the
     * device except in that registration — it is not the address, and
     * an identifier that can claim a row must not be one that travels.
     */
    private const val INSTALL_KEY = "install_id"

    /**
     * The last token the vendor issued, so a sign-in can send the
     * registration again without asking FCM for a token first.
     *
     * It is already in this app's private storage — FCM keeps its own
     * copy there — so keeping it costs nothing new. The handle stays
     * the capability; this is only an input to producing one.
     */
    private const val TOKEN_KEY = "native_token"

    /** What the server was last told, so a repeat is not sent. */
    @Volatile private var lastSentIdentity: String? = null

    private val worker =
        Executors.newSingleThreadScheduledExecutor { r ->
            Thread(r, "sentori-push").apply { isDaemon = true }
        }

    /**
     * Ask for permission, get a token, register it.
     *
     * Safe to call on every launch: Android returns its cached
     * decision without re-prompting, and the server upserts on
     * `(project, provider, token)`.
     *
     * Call [Sentori.user] first if the device should be reachable from
     * an issue. Without it the registration carries no user key and
     * the device receives broadcasts only — the dashboard shows that
     * as "N devices, 0 addressable", which is the one symptom with no
     * other explanation.
     *
     * [activity] is needed only for the Android 13+ runtime prompt;
     * pass null to skip prompting and use whatever was already
     * granted.
     */
    @JvmStatic
    @JvmOverloads
    fun register(
        context: Context,
        activity: Activity? = null,
        timeoutMs: Long = 8_000,
        onMessage: ((Map<String, Any?>) -> Unit)? = null,
        onTap: ((Map<String, Any?>) -> Unit)? = null,
        completion: (Result) -> Unit,
    ) {
        val config = SentoriConfig.current
        if (config == null) {
            deliver(completion, Result.Failure(Failure.NOT_INITIALISED, "Sentori.start has not run"))
            return
        }

        // Bind before asking for anything: a tap that arrived while
        // the app was dead is replayed as soon as the service starts,
        // and a callback set afterwards misses it.
        synchronized(lock) {
            this.onMessage = onMessage
            this.onTap = onTap
        }

        val appContext = context.applicationContext

        // Whatever launched this Activity, before anything else can
        // replace it. A cold start from a notification arrives here
        // and nowhere else, and `onTap` used to need the host to
        // forward it by hand — which nothing told the host to do.
        SentoriNotificationTap.consume(activity)
        val proceed = { status: String ->
            worker.execute {
                deliver(completion, finishRegister(appContext, config, status, timeoutMs))
            }
        }

        val current = SentoriPushNotifications.currentPermission(appContext)
        if (current == "notDetermined" && activity != null) {
            // `timeoutMs` is a network budget — how long to wait for a
            // device token. Spending it on a person reading a dialog
            // is a category error: the default is eight seconds, and
            // nobody answers a permission prompt in eight seconds.
            SentoriPushNotifications.requestPermission(
                activity,
                timeoutMs = permissionTimeoutMs,
            ) { proceed(it) }
        } else {
            proceed(current)
        }
    }

    /**
     * How long to wait for someone to answer the permission dialog.
     *
     * Separate from `timeoutMs`, which is about the network. A person
     * may be reading, or may have put the phone down; two minutes
     * covers the first and gives up on the second rather than leaving
     * a registration that never reports anything.
     */
    @JvmStatic
    var permissionTimeoutMs: Long = 120_000

    /**
     * A vendor has issued this device a new token; tell the server
     * now rather than at the next launch.
     *
     * `onNewToken` fired and the SDK wrote the value into a field.
     * Nothing sent it. So from the moment a token rotated until the
     * host next called `register`, the server held a dead token: the
     * sends went out, the vendor answered UNREGISTERED, quarantine
     * retired the device, and it came back at the next launch under a
     * different address. For an app that stays resident, "the next
     * launch" is not a bounded wait.
     *
     * Only re-registers a device that has registered before —
     * `spToken` on disk is the evidence. A token arriving for a
     * device the host never registered is not something to act on
     * unasked.
     */
    @JvmStatic
    fun handleRotatedToken(context: Context, token: String) {
        val appContext = context.applicationContext
        val config = SentoriConfig.current ?: return
        if (cachedDeviceHandle(appContext) == null) return
        worker.execute {
            when (val r = registerWithServer(appContext, token, config)) {
                is Result.Success -> rememberHandle(appContext, r.handle, token)
                is Result.Failure ->
                    android.util.Log.w(
                        "sentori",
                        "push re-register after token rotation failed " +
                            "(${r.reason.reason}): ${r.message}",
                    )
            }
        }
    }

    /**
     * Keep the address, in memory and on disk.
     *
     * Three callers now — the first registration, a rotation, and the
     * live test — and three copies of four lines is how two of them
     * end up writing different keys.
     */
    private fun rememberHandle(context: Context, handle: String, token: String? = null) {
        synchronized(lock) { cachedHandle = handle }
        val appContext = context.applicationContext
        appContext
            .getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(HANDLE_KEY, handle)
            .apply {
                // Kept for the sign-in path below, which has no other
                // way back to a token.
                if (token != null) putString(TOKEN_KEY, token)
            }
            .apply()
        // From here on, signing in or out updates the row by itself.
        // Installing this replays a sign-in that happened while the
        // registration was in flight — it announced to nobody, and
        // [SentoriScope] holds it until someone is listening.
        SentoriScope.setIdentityListener { identityChanged(appContext) }
    }

    /**
     * One string standing for "who this device belongs to", used only
     * to answer "has it changed since the last request".
     *
     * Keys sorted so that two maps holding the same pairs cannot
     * compare unequal on iteration order alone — that would report a
     * change that did not happen and send a registration per call of a
     * verb a host may call on every screen, which is the cost this
     * comparison exists to avoid. Matches `identityString` in the
     * Swift SDK.
     */
    private fun identityString(userKey: String?, traits: Map<String, Any?>?): String {
        val rendered =
            (traits ?: emptyMap()).entries
                .sortedBy { it.key }
                .joinToString("\u0001") { "${it.key}=${it.value}" }
        return "${userKey ?: "-"}\u0000$rendered"
    }

    /**
     * Send the registration again because the person changed.
     *
     * Only for a device that has already registered — a stored token
     * is the evidence, the same rule [handleRotatedToken] uses. Runs
     * on the worker: `Sentori.user` is synchronous and stays that way.
     */
    private fun identityChanged(appContext: Context) {
        val config = SentoriConfig.current ?: return
        // Everything, including the preference reads, on the worker.
        // `Sentori.user` is synchronous and is called from wherever
        // the host signs someone in — usually the main thread — and
        // reading preferences there is disk I/O on the thread that
        // draws frames. Warm in practice, since this listener is only
        // installed after a registration has already touched them; on
        // the worker it cannot be anything else.
        worker.execute {
            val prefs = appContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            val token = prefs.getString(TOKEN_KEY, null) ?: return@execute
            if (prefs.getString(HANDLE_KEY, null) == null) return@execute

            // `Sentori.user` is a verb an app may call on every screen,
            // and one request per call is not free to a host.
            val identity = identityString(SentoriScope.userKey, SentoriScope.traits)
            if (identity == lastSentIdentity) return@execute
            lastSentIdentity = identity

            when (val r = registerWithServer(appContext, token, config)) {
                is Result.Success -> rememberHandle(appContext, r.handle, token)
                is Result.Failure -> {
                    // The row still names the previous person, so the
                    // next change has to be allowed to try again.
                    lastSentIdentity = null
                    android.util.Log.w(
                        "sentori",
                        "updating the device after a sign-in failed " +
                            "(${r.reason.reason}): ${r.message}",
                    )
                }
            }
        }
    }

    /**
     * Register a vendor token directly, for the live test.
     *
     * The rotation path can only be exercised against a device that
     * has registered, and there is no FCM in a test host to get a
     * first token from. This is the same call `finishRegister` makes
     * once it has one.
     */
    internal fun registerNativeTokenForTests(context: Context, token: String): String? {
        val config = SentoriConfig.current ?: return null
        return when (val r = registerWithServer(context.applicationContext, token, config)) {
            is Result.Success -> {
                rememberHandle(context, r.handle, token)
                r.handle
            }
            else -> null
        }
    }

    /** Hand one drained batch to the host's callbacks, for tests. */
    internal fun drainForTests(state: Map<String, Any?>) = flush(state)

    /** This installation's id, minted on first use and kept after. */
    internal fun installId(ctx: Context): String {
        val prefs = ctx.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        prefs.getString(INSTALL_KEY, null)?.let { return it }
        val fresh = java.util.UUID.randomUUID().toString()
        prefs.edit().putString(INSTALL_KEY, fresh).apply()
        return fresh
    }

    /**
     * Say it out loud, once, where the person wiring this up is
     * looking.
     *
     * A failed `register` reported only to the server is invisible on
     * the machine where the mistake was made: the integrator has to
     * finish connecting the dashboard before it can tell them they
     * have not finished connecting the dashboard. insight found their
     * first-launch failure by adding a `Log.w` of their own and
     * taking it out again.
     *
     * Warning, never error. A red line in someone else's logcat reads
     * as "your app is broken", and a host team that believes that
     * pulls the SDK out.
     */
    private fun deliver(completion: (Result) -> Unit, result: Result) {
        if (result is Result.Failure) {
            android.util.Log.w(
                "sentori",
                "push register failed (${result.reason.reason}): ${result.message}",
            )
        }
        // The host's callback is the host's code. A registration that
        // succeeded must not be undone by what the host does with the
        // news, and this runs on the single worker thread — an
        // exception here takes the thread, and with it every push
        // request the process makes afterwards.
        safely("register callback") { completion(result) }
    }

    private fun finishRegister(
        context: Context,
        config: SentoriConfig,
        status: String,
        timeoutMs: Long,
    ): Result {
        if (status != "granted") {
            return Result.Failure(Failure.PERMISSION_DENIED, "push permission '$status'")
        }

        SentoriPushNotifications.registerForRemoteNotifications(context)

        val token = waitForToken(timeoutMs)
        if (token == null) {
            // An FCM that is not on the classpath reports an error
            // rather than a token, and that is a build fact rather
            // than something the user chose.
            val err = SentoriPushNotifications.drainState()["error"] as? String
            return if (err != null) {
                Result.Failure(Failure.NO_TRANSPORT, err)
            } else {
                Result.Failure(Failure.TOKEN_TIMEOUT, "no device token within ${timeoutMs}ms")
            }
        }

        return when (val r = registerWithServer(context, token, config)) {
            is Result.Success -> {
                rememberHandle(context, r.handle, token)
                startDrain()
                r
            }
            else -> r
        }
    }

    /** The handle from an earlier [register], without a round trip. */
    @JvmStatic
    fun cachedDeviceHandle(context: Context): String? =
        synchronized(lock) { cachedHandle }
            ?: context
                .getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .getString(HANDLE_KEY, null)

    /** Current permission without prompting. */
    @JvmStatic
    fun permissionStatus(context: Context): String =
        SentoriPushNotifications.currentPermission(context.applicationContext)

    /**
     * Revoke the handle server-side and stop local delivery.
     * Idempotent — repeat calls do nothing.
     */
    @JvmStatic
    @JvmOverloads
    fun unregister(context: Context, completion: ((Boolean) -> Unit)? = null) {
        val appContext = context.applicationContext
        val handle = cachedDeviceHandle(appContext)
        synchronized(lock) {
            cachedHandle = null
            onMessage = null
            onTap = null
            drainTask?.cancel(false)
            drainTask = null
        }
        appContext
            .getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .remove(HANDLE_KEY)
            .remove(TOKEN_KEY)
            .apply()
        // Nothing to follow the person to any more; a later sign-in
        // must not resurrect a device the host just revoked.
        SentoriScope.setIdentityListener(null)
        lastSentIdentity = null
        SentoriPushNotifications.unregisterForRemoteNotifications(appContext)

        val config = SentoriConfig.current
        if (handle == null || config == null) {
            completion?.invoke(false)
            return
        }
        worker.execute {
            var conn: HttpURLConnection? = null
            val ok =
                try {
                    conn =
                        URL("${config.ingestUrl}/v1/push/devices/$handle").openConnection()
                            as HttpURLConnection
                    conn.requestMethod = "DELETE"
                    conn.setRequestProperty("Authorization", "Bearer ${config.token}")
                    conn.connectTimeout = 15_000
                    conn.readTimeout = 15_000
                    conn.responseCode in 200..299
                } catch (_: Throwable) {
                    false
                } finally {
                    conn?.disconnect()
                }
            completion?.invoke(ok)
        }
    }

    // ── internals ─────────────────────────────────────────────────

    private fun waitForToken(timeoutMs: Long): String? {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            val state = SentoriPushNotifications.drainState()
            flush(state)
            (state["token"] as? String)?.let { return it }
            if (state["error"] != null) return null
            try {
                Thread.sleep(200)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
                return null
            }
        }
        return null
    }

    private fun registerWithServer(
        context: Context,
        token: String,
        config: SentoriConfig,
    ): Result {
        val body =
            mutableMapOf<String, Any?>(
                // `kind`, not `provider`. The React Native SDK sent
                // `provider` for a year and earned a 422 for every
                // registration it ever attempted.
                "kind" to "fcm",
                "nativeToken" to token,
                // Which installation this is. The server keys the row
                // on it, so a rotated token updates this device
                // rather than creating a second one under a new
                // address.
                "installId" to installId(context),
                // No `env`: FCM is one host, with no sandbox and
                // production split for a token to be wrong about.
            )
        // Read once. The body and the record of what the body carried
        // have to describe the same person, and two reads of a value
        // the host can change from any thread do not.
        val userKey = SentoriScope.userKey
        val traits = SentoriScope.traits
        // The same identity hash every event carries, so the dashboard
        // can address this device by the person who hit an issue.
        // Absent until the host calls `Sentori.user`.
        userKey?.let { body["userKey"] = it }
        // Attributes of the person rather than of the device, kept
        // apart so a build channel called "pro" cannot answer a send
        // aimed at the pro plan. Null leaves the row's traits alone;
        // an empty map clears them, which is what signing out sends.
        traits?.let { body["traits"] = it }
        // What this request puts on the wire, recorded before it goes.
        // [identityChanged] reads it back to decide whether the person
        // has changed since — including while this very request was in
        // flight, which is the window a host hits by calling
        // `Sentori.user` right after starting push registration.
        lastSentIdentity = identityString(userKey, traits)

        var conn: HttpURLConnection? = null
        return try {
            conn =
                URL("${config.ingestUrl}/v1/push/devices").openConnection() as HttpURLConnection
            conn.requestMethod = "POST"
            conn.setRequestProperty("Content-Type", "application/json")
            conn.setRequestProperty("Authorization", "Bearer ${config.token}")
            conn.setRequestProperty("Sentori-Sdk", "kotlin/${SentoriVersion.CURRENT}")
            conn.connectTimeout = 15_000
            conn.readTimeout = 15_000
            conn.doOutput = true
            conn.outputStream.use {
                it.write(SentoriTransport.toJson(body).toString().toByteArray(Charsets.UTF_8))
            }

            val code = conn.responseCode
            if (code !in 200..299) {
                Result.Failure(Failure.SERVER_REJECTED, "HTTP $code")
            } else {
                val text = conn.inputStream.bufferedReader().readText()
                // The handle is the `device_tokens` row id, a bare
                // uuid. The RN SDK parsed it as an `ipt_*` string no
                // server has ever returned.
                // The address, by its one name.
                val json = JSONObject(text)
                val handle = json.optString("spToken")
                if (handle.isNullOrEmpty()) {
                    Result.Failure(Failure.SERVER_REJECTED, "server returned no device token id")
                } else {
                    Result.Success(handle)
                }
            }
        } catch (e: Throwable) {
            Result.Failure(Failure.SERVER_REJECTED, e.message ?: e.javaClass.name)
        } finally {
            conn?.disconnect()
        }
    }

    /**
     * 1 Hz while registered. The FCM service buffers arrivals and
     * taps; this hands them to the host.
     */
    private fun startDrain() {
        synchronized(lock) {
            if (drainTask != null) return
            drainTask =
                worker.scheduleWithFixedDelay(
                    // Belt as well as braces: `flush` contains the
                    // host's code, and this contains everything else,
                    // because a task that throws is a task the
                    // executor never runs again.
                    { safely("drain") { flush(SentoriPushNotifications.drainState()) } },
                    1,
                    1,
                    TimeUnit.SECONDS,
                )
        }
    }

    @Suppress("UNCHECKED_CAST")
    /**
     * Hand a drained batch to the host, without letting the host reach
     * back.
     *
     * Push is allowed to fail silently; it is never allowed to make
     * the host app fail. The handlers are the host's own code running
     * inside our loop, and the loop is a `scheduleWithFixedDelay` —
     * where an exception out of the task makes the executor **cancel
     * every future run**, silently. One throwing `onMessage` ended
     * push delivery for the life of the process, and nothing anywhere
     * said so.
     */
    private fun flush(state: Map<String, Any?>) {
        val (message, tap) = synchronized(lock) { onMessage to onTap }
        if (message == null && tap == null) return
        (state["notifications"] as? List<Map<String, Any?>>)?.forEach {
            safely("onMessage") { message?.invoke(it) }
        }
        (state["taps"] as? List<Map<String, Any?>>)?.forEach {
            safely("onTap") { tap?.invoke(it) }
        }
    }

    /** Run the host's own code without letting it reach back into ours. */
    private inline fun safely(what: String, body: () -> Unit) {
        try {
            body()
        } catch (t: Throwable) {
            android.util.Log.w("sentori", "push $what threw — carrying on", t)
        }
    }

    internal fun resetForTests() {
        lastSentIdentity = null
        SentoriScope.setIdentityListener(null)
        synchronized(lock) {
            cachedHandle = null
            onMessage = null
            onTap = null
            drainTask?.cancel(false)
            drainTask = null
        }
    }
}
