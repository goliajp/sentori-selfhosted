package com.sentori

import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import java.util.UUID
import java.util.concurrent.atomic.AtomicBoolean

/**
 * ANR (Application Not Responding) detector.
 *
 * Lightweight watchdog: a worker thread posts a tick onto the main
 * Looper every `intervalMs` and sleeps for `timeoutMs`. If the main
 * thread didn't acknowledge the tick within that window we capture the
 * main thread's stack trace as a Sentori event with `kind = "anr"`
 * and write it to the same pending dir SentoriCrashHandler uses, so
 * the next-launch drain delivers it.
 *
 * Design notes:
 *   - Single-shot per ANR: once we report, we wait for the main
 *     thread to recover before re-arming. Otherwise a 30 s freeze
 *     would dump six events.
 *   - We DON'T kill the process or rethrow. The OS does that on its
 *     own ANR threshold (~5 s for input-driven, longer otherwise).
 *   - Worker thread is a daemon so it doesn't keep the process alive.
 *   - Disabled in debug builds by default — the JS debugger pauses
 *     the main thread routinely and we don't want a flood. The host
 *     app can override via `SentoriAnrWatchdog.start(ctx, force = true)`.
 */
object SentoriAnrWatchdog {

    private const val DEFAULT_TIMEOUT_MS = 5_000L
    private const val DEFAULT_INTERVAL_MS = 1_000L
    private const val PENDING_DIR_NAME = "sentori/pending"

    @Volatile private var running = AtomicBoolean(false)
    @Volatile private var thread: Thread? = null
    @Volatile private var appCtx: Context? = null

    /**
     * Start the watchdog. Idempotent — calling start() twice is a
     * no-op. Pass `force = true` to enable in debug builds.
     */
    @JvmStatic
    @JvmOverloads
    fun start(
        context: Context,
        timeoutMs: Long = DEFAULT_TIMEOUT_MS,
        intervalMs: Long = DEFAULT_INTERVAL_MS,
        force: Boolean = false,
    ) {
        if (!force && isDebuggable(context)) return
        if (running.getAndSet(true)) return
        appCtx = context.applicationContext

        val mainHandler = Handler(Looper.getMainLooper())
        val watchdogThread = Thread {
            val tick = MainTick()
            while (running.get()) {
                tick.armed = true
                mainHandler.post(tick)
                try {
                    Thread.sleep(timeoutMs)
                } catch (_: InterruptedException) {
                    return@Thread
                }
                if (tick.armed) {
                    // Main thread is wedged — capture once, then wait
                    // for the tick to land before arming again.
                    captureAnr()
                    while (running.get() && tick.armed) {
                        try {
                            Thread.sleep(intervalMs)
                        } catch (_: InterruptedException) {
                            return@Thread
                        }
                    }
                }
            }
        }
        watchdogThread.name = "Sentori-ANR-Watchdog"
        watchdogThread.isDaemon = true
        watchdogThread.start()
        thread = watchdogThread
    }

    @JvmStatic
    fun stop() {
        running.set(false)
        thread?.interrupt()
        thread = null
    }

    private fun captureAnr() {
        val ctx = appCtx ?: return
        try {
            val mainStack = Looper.getMainLooper().thread.stackTrace
            val event = buildAnrEvent(ctx, mainStack)
            // Phase 42 sub-F.07: ANR by definition means main thread
            // is wedged — but PixelCopy runs on a HandlerThread, so we
            // can still snapshot the (frozen) UI. Attach screenshot
            // + view tree like the crash path.
            attachPending(event)
            val dir = File(ctx.filesDir, PENDING_DIR_NAME)
            if (!dir.exists()) dir.mkdirs()
            val file = File(dir, "${uuid()}.json")
            file.writeText(event.toString())
        } catch (_: Throwable) {
            // never throw from inside the watchdog — losing one
            // capture beats killing the worker thread.
        }
    }

    private fun attachPending(event: JSONObject) {
        val snap = try {
            SentoriScreenshotCapture.captureKeyWindow()
        } catch (_: Throwable) {
            null
        } ?: return
        val pending = JSONArray()

        @Suppress("UNCHECKED_CAST")
        val sc = snap["screenshot"] as? Map<String, Any>
        if (sc != null) {
            val b64 = sc["base64"] as? String
            val mt = (sc["mediaType"] as? String) ?: "image/webp"
            if (b64 != null) {
                pending.put(JSONObject().apply {
                    put("kind", "screenshot")
                    put("base64", b64)
                    put("mediaType", mt)
                    put("source", "android")
                })
            }
        }
        val vt = snap["viewTree"]
        if (vt != null) {
            val asJson = SentoriScreenshotCapture.toJson(vt)
            val base64 = android.util.Base64.encodeToString(
                asJson.toString().toByteArray(Charsets.UTF_8),
                android.util.Base64.NO_WRAP,
            )
            pending.put(JSONObject().apply {
                put("kind", "viewTree")
                put("base64", base64)
                put("mediaType", "application/json")
                put("source", "android")
            })
        }
        if (pending.length() > 0) {
            event.put("_pendingAttachments", pending)
        }
    }

    private fun buildAnrEvent(ctx: Context, mainStack: Array<StackTraceElement>): JSONObject {
        val cfg = configMap(ctx)
        val release = cfg["release"] ?: "unknown"
        val environment = cfg["environment"] ?: "prod"

        val device = JSONObject().apply {
            put("os", "android")
            put("osVersion", Build.VERSION.RELEASE)
            put("model", "${Build.MANUFACTURER} ${Build.MODEL}")
        }
        val app = JSONObject().apply {
            put("version", appVersion(ctx))
            put("build", appBuild(ctx))
        }

        val frames = JSONArray()
        for (f in mainStack) {
            frames.put(
                JSONObject().apply {
                    put("function", "${f.className}.${f.methodName}")
                    put("file", f.fileName ?: "<unknown>")
                    put("line", f.lineNumber.coerceAtLeast(0))
                    put("inApp", isInApp(f.className))
                },
            )
        }

        val error = JSONObject().apply {
            put("type", "ApplicationNotResponding")
            put("message", "Main thread blocked for ≥ ${DEFAULT_TIMEOUT_MS} ms")
            put("stack", frames)
            put("cause", JSONObject.NULL)
        }

        return JSONObject().apply {
            put("id", uuid())
            put("timestamp", iso8601Now())
            put("kind", "anr")
            put("platform", "android")
            put("release", release)
            put("environment", environment)
            put("device", device)
            put("app", app)
            put("user", JSONObject.NULL)
            put("tags", JSONObject().apply { put("source", "sentori.anrWatchdog") })
            put("breadcrumbs", JSONArray())
            put("error", error)
            put("fingerprint", JSONArray())
            put("traceId", JSONObject.NULL)
            put("spanId", JSONObject.NULL)
        }
    }

    private fun configMap(ctx: Context): Map<String, String> {
        val prefs = ctx.getSharedPreferences("sentori", Context.MODE_PRIVATE)
        val out = mutableMapOf<String, String>()
        for ((k, v) in prefs.all) if (v is String) out[k] = v
        return out
    }

    private fun isInApp(cls: String): Boolean {
        if (cls.startsWith("android.")) return false
        if (cls.startsWith("androidx.")) return false
        if (cls.startsWith("java.")) return false
        if (cls.startsWith("javax.")) return false
        if (cls.startsWith("kotlin.")) return false
        if (cls.startsWith("kotlinx.")) return false
        if (cls.startsWith("com.facebook.react.")) return false
        if (cls.startsWith("com.android.")) return false
        if (cls.startsWith("dalvik.")) return false
        if (cls.startsWith("sun.")) return false
        return true
    }

    private fun iso8601Now(): String {
        val f = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.US)
        f.timeZone = TimeZone.getTimeZone("UTC")
        return f.format(Date())
    }

    private fun uuid(): String = UUID.randomUUID().toString().lowercase(Locale.US)

    private fun appVersion(ctx: Context): String =
        try {
            val pi = ctx.packageManager.getPackageInfo(ctx.packageName, 0)
            pi.versionName ?: "0.0.0"
        } catch (_: Throwable) {
            "0.0.0"
        }

    private fun appBuild(ctx: Context): String =
        try {
            val pi = ctx.packageManager.getPackageInfo(ctx.packageName, 0)
            pi.longVersionCode.toString()
        } catch (_: Throwable) {
            "0"
        }

    private fun isDebuggable(ctx: Context): Boolean =
        (ctx.applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE) != 0

    /** Posted to the main thread every poll. `armed` is flipped to
     *  false once it actually runs. */
    private class MainTick : Runnable {
        @Volatile var armed = true

        override fun run() {
            armed = false
        }
    }
}
