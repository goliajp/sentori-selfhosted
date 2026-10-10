package com.sentori

import android.content.Context
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import java.util.concurrent.ThreadLocalRandom

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
        try {
            configure(config, context)
        } catch (_: Throwable) {
            // A broken host Context or platform hook disables capture instead
            // of turning SDK initialization into a host application failure.
            SentoriConfig.set(null)
        }
    }

    private fun configure(config: SentoriConfig, context: Context?) {
        SentoriConfig.set(config)
        SentoriDevice.bind(context)
        SentoriTransport.startFromContext(context)

        // Crash capture needs a Context for its storage, so an app
        // that starts without one gets everything except this.
        if (context != null) {
            // Capture configuration before installing the handler, including
            // the first launch before it has a bound Context.
            SentoriCrashHandler.setConfig(
                mapOf("release" to config.release, "environment" to config.environment)
            )
            SentoriCrashHandler.register(context)

            // Foreground / background / memory pressure into the
            // signal ring, and a flush on the way out.
            SentoriLifecycle.register(context)

            // The crash that killed the last launch. Until now the
            // handler wrote files into a directory nothing emptied —
            // a crash reporter that captured crashes and never sent
            // one.
            SentoriTransport.background {
                SentoriPendingCrash.ship()
                SentoriExitInfo.collect(context)
            }

            // Why the last process died, from the system rather than
            // inferred. On API 30+ this is the same record Play
            // Console reports on, so an ANR count of ours and theirs
            // cannot disagree. The watchdog stays for older devices.
            if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.R) {
                SentoriAnrWatchdog.start(context)
            }
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
        emit("error", null, null, data, withSignals = true, pendingError = err)

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
        pendingError: Throwable? = null,
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
        payload["device"] = SentoriDevice.snapshot()

        val event =
            mutableMapOf<String, Any?>(
                "id" to id,
                "kind" to kind,
                // Formatting initializes locale/calendar machinery. The worker
                // formats this captured clock value, preserving occurrence time.
                pendingTimestampKey to System.currentTimeMillis(),
                "platform" to "android",
                "release" to config.release,
                "environment" to config.environment,
                "payload" to payload,
            )
        name?.let { event["name"] = it }
        if (withSignals) event[pendingSignalsKey] = SentoriSignalRing.capture()
        if (pendingError != null) event[pendingErrorKey] = pendingError
        SentoriScope.userKey?.let { event["userKey"] = it }

        SentoriTransport.enqueue(event)
        return id
    }

    private fun describe(err: Throwable, depth: Int = 0): Map<String, Any?> {
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
        err.cause?.let { if (it !== err && depth < 8) out["cause"] = describe(it, depth + 1) }
        return out
    }

    private object TimestampFormatter {
        val instance = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.US).apply {
            timeZone = TimeZone.getTimeZone("UTC")
        }
    }

    internal const val pendingTimestampKey = "_sentoriOccurredAtMs"
    private const val pendingSignalsKey = "_sentoriSignalSnapshot"
    private const val pendingErrorKey = "_sentoriThrowable"

    internal fun prepareEvent(event: Map<String, Any?>): Map<String, Any?> {
        val out = event.toMutableMap()
        (out.remove(pendingErrorKey) as? Throwable)?.let { throwable ->
            @Suppress("UNCHECKED_CAST")
            val payload = (out["payload"] as Map<String, Any?>).toMutableMap()
            payload["error"] = try { describe(throwable) } catch (_: Throwable) {
                mapOf("type" to "Error", "message" to "Error details unavailable")
            }
            out["payload"] = payload
        }
        (out.remove(pendingTimestampKey) as? Long)?.let { out["occurredAt"] = iso8601(Date(it)) }
        @Suppress("UNCHECKED_CAST")
        val capture = out.remove(pendingSignalsKey) as? (() -> List<Map<String, Any?>>)
        if (capture != null) {
            val signals = capture()
            @Suppress("UNCHECKED_CAST")
            val payload = (out["payload"] as Map<String, Any?>).toMutableMap()
            if (signals.isNotEmpty()) payload["signals"] = signals
            out["payload"] = payload
        }
        return out
    }

    internal fun iso8601(date: Date): String = synchronized(TimestampFormatter.instance) { TimestampFormatter.instance.format(date) }

    /**
     * UUIDv7: 48 bits of milliseconds then random, so ids sort by
     * creation time. The server keys events on them and the dashboard
     * orders by them, so a v4 would scatter a session across the
     * index.
     */
    internal fun newEventId(): String {
        val bytes = ByteArray(16)
        ThreadLocalRandom.current().nextBytes(bytes)
        val ms = System.currentTimeMillis()
        bytes[0] = ((ms shr 40) and 0xff).toByte()
        bytes[1] = ((ms shr 32) and 0xff).toByte()
        bytes[2] = ((ms shr 24) and 0xff).toByte()
        bytes[3] = ((ms shr 16) and 0xff).toByte()
        bytes[4] = ((ms shr 8) and 0xff).toByte()
        bytes[5] = (ms and 0xff).toByte()
        bytes[6] = ((bytes[6].toInt() and 0x0f) or 0x70).toByte()
        bytes[8] = ((bytes[8].toInt() and 0x3f) or 0x80).toByte()

        val digits = "0123456789abcdef"
        val text = CharArray(36)
        var at = 0
        for ((index, byte) in bytes.withIndex()) {
            if (index == 4 || index == 6 || index == 8 || index == 10) text[at++] = '-'
            val value = byte.toInt() and 0xff
            text[at++] = digits[value ushr 4]
            text[at++] = digits[value and 15]
        }
        return String(text)
    }
}
