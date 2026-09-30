// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/main/java/com/sentori/SentoriExitInfo.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.os.Build
import java.io.InputStream

/**
 * Why the last process died, asked of the system rather than guessed.
 *
 * The watchdog in [SentoriAnrWatchdog] infers an ANR from the main
 * thread missing a tick. That is the only thing available before
 * Android 11, and it is an inference: a debugger pause, a long GC or a
 * device going to sleep all look the same to it, and it cannot see an
 * ANR that killed the process before it could write anything.
 *
 * From API 30 the system keeps the answer. `getHistoricalProcessExitReasons`
 * returns the same records Play Console reports on, including the ANR
 * trace the system itself captured — so a report from here and the
 * number the customer's release dashboard shows come from one source
 * and agree. That is the point: an ANR count of ours that disagreed
 * with Play Console would be argued with rather than acted on.
 *
 * Read once at start, for what happened since the last time we looked.
 * The high-water mark is a timestamp in the same SharedPreferences the
 * crash handler uses — a record delivered twice would open a second
 * issue for one freeze.
 */
object SentoriExitInfo {
    private const val PREFS = "sentori"
    private const val SEEN_KEY = "exitInfoSeenAt"

    /** More than a launch's worth of history is not actionable. */
    private const val MAX_RECORDS = 10

    /**
     * What we report on. A death by ANR or by a native signal is
     * something the app can be fixed for; `REASON_USER_REQUESTED` (a
     * swipe away) and `REASON_EXIT_SELF` are the app working.
     *
     * A JVM crash is deliberately absent: [SentoriCrashHandler] has
     * already written that one with a real stack, and this would be
     * the same death reported twice with less in it.
     */
    private val REPORTED =
        setOf(ApplicationExitInfo.REASON_ANR, ApplicationExitInfo.REASON_SIGNALED)

    /**
     * Report anything new, and remember how far we read.
     *
     * Silent on every failure. A monitoring SDK that throws out of
     * `start()` has taken the host down for the thing that was meant
     * to tell them it fell over.
     */
    fun collect(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return
        try {
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            val seenAt = prefs.getLong(SEEN_KEY, 0L)
            val am =
                context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager ?: return
            val records =
                am.getHistoricalProcessExitReasons(context.packageName, 0, MAX_RECORDS)
            var newest = seenAt
            for (record in records) {
                if (record.timestamp > newest) newest = record.timestamp
                if (!isNew(record.timestamp, record.reason, seenAt)) continue
                SentoriTransport.enqueue(
                    wireEvent(
                        occurredAtMillis = record.timestamp,
                        reason = record.reason,
                        description = record.description,
                        trace = readTrace(record),
                    )
                )
            }
            if (newest > seenAt) prefs.edit().putLong(SEEN_KEY, newest).apply()
        } catch (_: Throwable) {
            // Including the SecurityException a host that restricts
            // the ActivityManager would throw.
        }
    }

    /** Separated from the system call so it can be tested. */
    fun isNew(timestamp: Long, reason: Int, seenAt: Long): Boolean =
        timestamp > seenAt && reason in REPORTED

    /**
     * The event the server stores. `kind` is `error` because there
     * are five kinds and no sixth — an ANR is an error the app could
     * not report itself, and what it was is in the type.
     */
    fun wireEvent(
        occurredAtMillis: Long,
        reason: Int,
        description: String?,
        trace: List<Map<String, Any?>>,
    ): Map<String, Any?> {
        val config = SentoriConfig.current
        val payload =
            mutableMapOf<String, Any?>(
                "error" to
                    mapOf(
                        "type" to typeName(reason),
                        "message" to (description ?: typeName(reason)),
                        "stack" to trace,
                    ),
                // Same flag the pending-crash path sets: this arrived
                // from the grave, not from an app that noticed.
                "nativeCrash" to true,
                // Where the number came from, so a disagreement with
                // Play Console is answerable rather than argued about.
                "exitInfo" to mapOf("source" to "ApplicationExitInfo", "reason" to reason),
            )
        return mapOf(
            "id" to Sentori.newEventId(),
            "kind" to "error",
            "occurredAt" to Sentori.iso8601(java.util.Date(occurredAtMillis)),
            "platform" to "android",
            "release" to (config?.release ?: ""),
            "environment" to (config?.environment ?: ""),
            "payload" to payload,
        )
    }

    // These are compile-time constants, so they are the platform's
    // real values even on the stub `android.jar` the unit tests link
    // against. Only the call into ActivityManager needs a version
    // guard; gating the decision on `SDK_INT` as well made every one
    // of these tests pass by never deciding anything.
    private fun typeName(reason: Int): String =
        if (reason == ApplicationExitInfo.REASON_ANR) {
            "ApplicationNotResponding"
        } else {
            "ProcessSignalled"
        }

    private fun readTrace(record: ApplicationExitInfo): List<Map<String, Any?>> =
        try {
            record.traceInputStream?.use { parseTrace(it) } ?: emptyList()
        } catch (_: Throwable) {
            emptyList()
        }

    /**
     * The system's ANR trace, cut down to the main thread's frames.
     *
     * The dump holds every thread and runs to hundreds of kilobytes;
     * what opens the case is where the main thread was stuck. Frames
     * look like:
     *
     *     at com.example.Cart.total(Cart.java:48)
     *     at android.os.Looper.loop(Looper.java:223)
     */
    fun parseTrace(stream: InputStream): List<Map<String, Any?>> {
        val frames = mutableListOf<Map<String, Any?>>()
        var inMain = false
        var done = false
        stream.bufferedReader().forEachLine { line ->
            if (done) return@forEachLine
            val text = line.trim()
            if (text.startsWith("\"")) {
                // The next thread's header ends main's section. Only
                // skipping the header, and leaving the flag set, kept
                // reading the next thread's frames as if they were
                // main's — a stack that looks right and is not.
                if (inMain) {
                    done = true
                } else {
                    inMain = text.startsWith("\"main\"")
                }
                return@forEachLine
            }
            if (!inMain || !text.startsWith("at ")) return@forEachLine
            if (frames.size >= MAX_FRAMES) return@forEachLine
            frames += frame(text.removePrefix("at ").trim())
        }
        return frames
    }

    private const val MAX_FRAMES = 60

    private fun frame(text: String): Map<String, Any?> {
        val open = text.lastIndexOf('(')
        val close = text.lastIndexOf(')')
        if (open < 0 || close < open) {
            return mapOf("function" to text, "file" to "", "line" to 0, "inApp" to isInApp(text))
        }
        val method = text.substring(0, open)
        val location = text.substring(open + 1, close)
        val colon = location.lastIndexOf(':')
        val file = if (colon > 0) location.substring(0, colon) else location
        val line = if (colon > 0) location.substring(colon + 1).toIntOrNull() ?: 0 else 0
        return mapOf(
            "function" to method,
            "file" to file,
            "line" to line,
            "inApp" to isInApp(method),
        )
    }

    private fun isInApp(qualified: String): Boolean {
        val system =
            listOf(
                "android.", "androidx.", "java.", "javax.", "kotlin.", "kotlinx.",
                "com.android.", "dalvik.", "sun.", "libcore.",
            )
        return system.none { qualified.startsWith(it) }
    }
}
