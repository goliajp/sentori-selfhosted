package com.sentori

import android.content.Context
import android.os.Build
import android.util.Base64
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import java.util.UUID

/**
 * Static crash handler — captures Java/Kotlin uncaught exceptions on
 * Android and writes one event-shaped JSON file per crash to
 * <filesDir>/sentori/pending/<uuid>.json. JS drains that directory on
 * next launch via Sentori.drainPending().
 *
 * What this does NOT do (Phase 7 v0.1):
 *   - native crashes (NDK / SIGSEGV) — Phase 7 explicitly skips signal-
 *     based handlers per ROADMAP.
 *   - ANR detection — deferred to v0.2.
 */
object SentoriCrashHandler {

    private const val PREFS = "sentori"
    private const val PENDING_DIR_NAME = "sentori/pending"

    @Volatile private var appCtx: Context? = null
    @Volatile private var previousHandler: Thread.UncaughtExceptionHandler? = null

    @JvmStatic
    fun register(context: Context) {
        appCtx = context.applicationContext
        previousHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            try {
                write(throwable)
            } catch (_: Throwable) {
                // never throw inside the crash handler
            }
            previousHandler?.uncaughtException(thread, throwable)
        }
        // Phase 42 sub-F.01: have the screenshot helper track the
        // foreground Activity so it knows which Window to PixelCopy.
        (appCtx as? android.app.Application)?.let {
            SentoriScreenshotCapture.register(it)
            // v0.9.6 #2 — replay capture also tracks the activity for
            // wireframe view-tree walks.
            SentoriReplayCapture.register(it)
        }
    }

    @JvmStatic
    fun setConfig(config: Map<String, Any?>) {
        val ctx = appCtx ?: return
        val prefs = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val edit = prefs.edit()
        edit.clear()
        for ((k, v) in config) {
            when (v) {
                is String -> edit.putString(k, v)
                is Int -> edit.putInt(k, v)
                is Boolean -> edit.putBoolean(k, v)
                else -> {}
            }
        }
        edit.apply()
    }

    @JvmStatic
    fun consumePending(): List<String> {
        val dir = pendingDir() ?: return emptyList()
        if (!dir.exists()) return emptyList()
        val out = mutableListOf<String>()
        val files = dir.listFiles { f -> f.extension == "json" } ?: emptyArray()
        for (f in files) {
            try {
                out.add(f.readText())
            } catch (_: Throwable) {
                // skip unreadable file
            }
            f.delete()
        }
        return out
    }

    // ── internals ────────────────────────────────────────────────

    private fun pendingDir(): File? {
        val ctx = appCtx ?: return null
        val dir = File(ctx.filesDir, PENDING_DIR_NAME)
        if (!dir.exists()) dir.mkdirs()
        return dir
    }

    private fun configMap(): Map<String, String> {
        val ctx = appCtx ?: return emptyMap()
        val prefs = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val out = mutableMapOf<String, String>()
        for ((k, v) in prefs.all) if (v is String) out[k] = v
        return out
    }

    private fun write(throwable: Throwable) {
        val cfg = configMap()
        val release = cfg["release"] ?: "unknown"
        val environment = cfg["environment"] ?: "prod"

        val device = JSONObject().apply {
            put("os", "android")
            put("osVersion", Build.VERSION.RELEASE)
            put("model", "${Build.MANUFACTURER} ${Build.MODEL}")
        }

        val app = JSONObject().apply {
            put("version", appVersion())
            put("build", appBuild())
        }

        val error = errorToJson(throwable)

        // v0.9.5 #7 — detect crashes originating from native code (JNI
        // / .so libs). Pure SIGSEGV in a stripped .so won't reach us
        // without breakpad (queued for v1.1), but throws that surface
        // as UnsatisfiedLinkError or have native frames in the stack
        // are tagged so the dashboard can split them out.
        val tags = JSONObject()
        if (SentoriNativeOrigin.looksNative(throwable)) {
            tags.put("native_signal", "true")
        }

        val event = JSONObject().apply {
            put("id", uuidLower())
            put("timestamp", iso8601Now())
            put("kind", "error")
            put("platform", "android")
            put("release", release)
            put("environment", environment)
            put("device", device)
            put("app", app)
            put("user", JSONObject.NULL)
            put("tags", tags)
            put("breadcrumbs", JSONArray())
            put("error", error)
            put("fingerprint", JSONArray())
            put("traceId", JSONObject.NULL)
            put("spanId", JSONObject.NULL)
        }

        // Phase 42 sub-F.05/08: capture screen + view tree before the
        // app dies, attach as `_pendingAttachments` for the JS side to
        // upload on next launch (same shape as iOS sub-E).
        attachPending(event)

        val dir = pendingDir() ?: return
        val file = File(dir, "${uuidLower()}.json")
        try {
            file.writeText(event.toString())
        } catch (_: Throwable) {
            // best-effort
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
            // Convert Map → JSONObject → string → base64
            val asJson = SentoriScreenshotCapture.toJson(vt)
            val base64 = Base64.encodeToString(
                asJson.toString().toByteArray(Charsets.UTF_8),
                Base64.NO_WRAP,
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

    private fun errorToJson(throwable: Throwable): JSONObject {
        return JSONObject().apply {
            put("type", throwable.javaClass.name)
            put("message", throwable.message ?: "")
            put("stack", framesToJson(throwable))
            val cause = throwable.cause
            if (cause != null && cause !== throwable) {
                put("cause", errorToJson(cause))
            } else {
                put("cause", JSONObject.NULL)
            }
        }
    }

    private fun framesToJson(throwable: Throwable): JSONArray {
        val arr = JSONArray()
        for (f in throwable.stackTrace) {
            val frame = JSONObject().apply {
                put("function", "${f.className}.${f.methodName}")
                put("file", f.fileName ?: "<unknown>")
                put("line", f.lineNumber.coerceAtLeast(0))
                put("inApp", isInApp(f))
            }
            arr.put(frame)
        }
        return arr
    }

    private fun isInApp(f: StackTraceElement): Boolean {
        val cls = f.className
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

    private fun uuidLower(): String = UUID.randomUUID().toString().lowercase(Locale.US)

    private fun appVersion(): String {
        val ctx = appCtx ?: return "0.0.0"
        return try {
            val pi = ctx.packageManager.getPackageInfo(ctx.packageName, 0)
            pi.versionName ?: "0.0.0"
        } catch (_: Throwable) {
            "0.0.0"
        }
    }

    private fun appBuild(): String {
        val ctx = appCtx ?: return "0"
        return try {
            val pi = ctx.packageManager.getPackageInfo(ctx.packageName, 0)
            pi.longVersionCode.toString()
        } catch (_: Throwable) {
            "0"
        }
    }
}
