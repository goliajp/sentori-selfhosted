package com.sentori

import android.content.Context
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import kotlin.random.Random

/**
 * The surface a host app writes against.
 *
 *     Sentori.error(e)                what went wrong?
 *     Sentori.warn("checkout.slow")   where did the user struggle?
 *     Sentori.trace("cart.opened")    what happened here?
 *     Sentori.assert("total", ok)     should this hold?
 *     Sentori.probe("SEN-482")        is that bug back?
 *
 * Every one is synchronous, returns the event id it minted, and never
 * throws — including before [start], where each is a no-op that still
 * hands back an id. An app that mis-wires its token gets a silent SDK,
 * not an exception on a path it did not know it had.
 *
 * Same semantics as the Swift and React Native surfaces, because the
 * dashboard reading them cannot tell which one sent an event and
 * should not have to.
 */
object Sentori {

    // ── lifecycle ─────────────────────────────────────────────────

    /**
     * Configure and start. [context] is used for one thing: the
     * directory events spill into when the network is gone. Pass the
     * application context, not an Activity.
     *
     * Nothing here reaches the network; the first request happens when
     * there is something to send.
     */
    @JvmStatic
    @JvmOverloads
    fun start(config: SentoriConfig, context: Context? = null) {
        SentoriConfig.set(config)
        SentoriDevice.bind(context)
        SentoriTransport.start(context?.let { java.io.File(it.filesDir, "sentori") })

        // Crash capture needs a Context for its storage, so an app
        // that starts without one gets everything except this.
        if (context != null) {
            // The handler runs inside an uncaught-exception handler
            // where almost nothing is safe, so it reads its release
            // and environment from SharedPreferences rather than from
            // a live object. Without this call every crash ships as
            // `unknown` / `prod` — matching no release, so
            // symbolicating against nothing.
            SentoriCrashHandler.setConfig(
                mapOf("release" to config.release, "environment" to config.environment)
            )
            SentoriCrashHandler.register(context)

            // The crash that killed the last launch. Until now the
            // handler wrote files into a directory nothing emptied —
            // a crash reporter that captured crashes and never sent
            // one.
            SentoriPendingCrash.ship()
        }
    }

    /**
     * Identify the person using the app. Only a hash of [id] (or
     * [email] when there is no id) travels; the raw values stay here.
     *
     * Order does not matter with push: a device that has registered
     * updates itself when this changes, so signing in after
     * registering makes the device reachable straight away. It said to
     * call this first, which was advice for a defect rather than a
     * design — most apps learn who the user is well after launch.
     *
     * [traits] are attributes a push campaign can select on: plan,
     * cohort, org. They travel raw, unlike [id] and [email], so put
     * nothing there that identifies the person.
     */
    @JvmStatic
    @JvmOverloads
    fun user(id: String?, email: String?, traits: Map<String, Any?>? = null) =
        SentoriScope.setUser(id, email, traits)

    /** Merge keys into the ambient context that rides every event. */
    @JvmStatic
    fun context(patch: Map<String, Any?>) = SentoriScope.patchContext(patch)

    /**
     * Record a signal for the last-sixty-seconds ring that ships with
     * an error. Any [kind] is accepted; the dashboard reads `http` as
     * `{ method, url, status, ms }` and `trace` as a quiet breadcrumb.
     *
     * This SDK deliberately does not install an OkHttp interceptor:
     * watching the host's traffic is the host's decision, not ours to
     * make silently.
     */
    @JvmStatic
    @JvmOverloads
    fun pushSignal(kind: String, data: Map<String, Any?>? = null) =
        SentoriSignalRing.push(kind, data)

    /**
     * `Sentori.push.register(...)`, matching `sentori.push.register()`
     * in the React Native SDK and `Sentori.push` in Swift.
     */
    @JvmStatic val push: SentoriPush get() = SentoriPush

    // ── the verbs ─────────────────────────────────────────────────

    /** Something went wrong. */
    @JvmStatic
    @JvmOverloads
    fun error(err: Throwable, data: Map<String, Any?>? = null): String =
        emit("error", null, describe(err), data, withSignals = true)

    /** Something went wrong, described rather than thrown. */
    @JvmStatic
    @JvmOverloads
    fun error(message: String, type: String = "Error", data: Map<String, Any?>? = null): String =
        emit("error", null, mapOf("type" to type, "message" to message), data, withSignals = true)

    /**
     * The user struggled here. Not a crash — a place the product hurt,
     * named by you.
     */
    @JvmStatic
    @JvmOverloads
    fun warn(name: String, data: Map<String, Any?>? = null): String =
        emit("warn", name, null, data, withSignals = true)

    /**
     * This happened. Always lands in the signal ring; [quiet] keeps it
     * out of the event stream, which is how a high-frequency
     * breadcrumb stays affordable.
     */
    @JvmStatic
    @JvmOverloads
    fun trace(name: String, data: Map<String, Any?>? = null, quiet: Boolean = false): String {
        val signal = mutableMapOf<String, Any?>("name" to name)
        data?.let { signal.putAll(it) }
        SentoriSignalRing.push("trace", signal)
        if (quiet) return newEventId()
        return emit("trace", name, null, data, withSignals = false)
    }

    /**
     * This should hold. A passing assert never becomes an event — it
     * increments a counter that rides the next batch, so a liveness
     * check costs no request. Only failures are events.
     *
     * Unlike the language's own `assert`, this never stops the
     * program. That difference is the point: a monitoring SDK that can
     * halt the app it monitors has picked the wrong side of the
     * bargain.
     */
    @JvmStatic
    @JvmOverloads
    fun assert(name: String, ok: Boolean, data: Map<String, Any?>? = null): String {
        SentoriConfig.current?.let { SentoriTransport.countAssert(name, ok, it.release) }
        if (ok) return newEventId()
        return emit("assert", name, null, data, withSignals = true)
    }

    /**
     * Is that bug back? A tripwire: reaching this call is the signal.
     * It changes no control flow and returns no verdict.
     */
    @JvmStatic
    @JvmOverloads
    fun probe(ref: String, data: Map<String, Any?>? = null): String =
        emit("probe", ref, null, data, withSignals = false)

    // ── assembly ──────────────────────────────────────────────────

    private fun emit(
        kind: String,
        name: String?,
        error: Map<String, Any?>?,
        data: Map<String, Any?>?,
        withSignals: Boolean,
    ): String {
        val id = newEventId()
        // Before init every verb is a no-op that still returns an id,
        // so a call site does not change shape depending on whether
        // the SDK came up.
        val config = SentoriConfig.current ?: return id

        val payload = mutableMapOf<String, Any?>()
        error?.let { payload["error"] = it }
        data?.let { payload["data"] = it }
        SentoriScope.context?.let { payload["context"] = it }
        if (withSignals) {
            val signals = SentoriSignalRing.snapshot()
            if (signals.isNotEmpty()) payload["signals"] = signals
        }
        payload["device"] = SentoriDevice.snapshot()

        val event =
            mutableMapOf<String, Any?>(
                "id" to id,
                "kind" to kind,
                "occurredAt" to iso8601(Date()),
                "platform" to "android",
                "release" to config.release,
                "environment" to config.environment,
                "payload" to payload,
            )
        name?.let { event["name"] = it }
        SentoriScope.userKey?.let { event["userKey"] = it }

        SentoriTransport.enqueue(event)
        return id
    }

    private fun describe(err: Throwable): Map<String, Any?> {
        val frames =
            err.stackTrace.take(50).map { f ->
                mapOf(
                    "function" to "${f.className}.${f.methodName}",
                    "file" to (f.fileName ?: ""),
                    "line" to f.lineNumber,
                )
            }
        val out =
            mutableMapOf<String, Any?>(
                "type" to (err.javaClass.name),
                "message" to (err.message ?: ""),
                "stack" to frames,
            )
        err.cause?.let { if (it !== err) out["cause"] = describe(it) }
        return out
    }

    private val isoFormat =
        SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.US).apply {
            timeZone = TimeZone.getTimeZone("UTC")
        }

    internal fun iso8601(date: Date): String = synchronized(isoFormat) { isoFormat.format(date) }

    /**
     * UUIDv7: 48 bits of milliseconds then random, so ids sort by
     * creation time. The server keys events on them and the dashboard
     * orders by them, so a v4 would scatter a session across the
     * index.
     */
    internal fun newEventId(): String {
        val bytes = ByteArray(16)
        Random.nextBytes(bytes)
        val ms = System.currentTimeMillis()
        bytes[0] = ((ms shr 40) and 0xff).toByte()
        bytes[1] = ((ms shr 32) and 0xff).toByte()
        bytes[2] = ((ms shr 24) and 0xff).toByte()
        bytes[3] = ((ms shr 16) and 0xff).toByte()
        bytes[4] = ((ms shr 8) and 0xff).toByte()
        bytes[5] = (ms and 0xff).toByte()
        bytes[6] = ((bytes[6].toInt() and 0x0f) or 0x70).toByte()
        bytes[8] = ((bytes[8].toInt() and 0x3f) or 0x80).toByte()

        val hex = StringBuilder(32)
        for (b in bytes) hex.append(String.format("%02x", b.toInt() and 0xff))
        val h = hex.toString()
        return "${h.substring(0, 8)}-${h.substring(8, 12)}-${h.substring(12, 16)}-" +
            "${h.substring(16, 20)}-${h.substring(20, 32)}"
    }
}
