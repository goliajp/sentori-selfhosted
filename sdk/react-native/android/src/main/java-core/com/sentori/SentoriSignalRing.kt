// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/main/java/com/sentori/SentoriSignalRing.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import kotlin.math.roundToLong

/**
 * What the user was doing for the last sixty seconds, shipped inside
 * `payload.signals` when an error or warn goes out.
 *
 * Bounded and overwrite-oldest. [push] is on the hot path — a tap
 * handler, a navigation — so it does one append and at most one
 * removal under a lock, and nothing else.
 *
 * The window matches the replay buffer deliberately: at 30 s of
 * signals against 60 s of replay, the left half of a case timeline had
 * frames and no events, which reads as "nothing happened" rather than
 * "we were not looking" (insight, round 4).
 */
object SentoriSignalRing {

    private data class Entry(val at: Long, val kind: String, val data: Map<String, Any?>?)

    private const val DEFAULT_CAPACITY = 100
    private const val DEFAULT_WINDOW_MS = 60_000L

    private val lock = Any()
    private val entries = ArrayDeque<Entry>()
    private var capacity = DEFAULT_CAPACITY
    private var windowMs = DEFAULT_WINDOW_MS

    @JvmStatic
    fun configure(capacity: Int, windowMs: Long) {
        synchronized(lock) {
            if (capacity > 0) this.capacity = capacity
            if (windowMs > 0) this.windowMs = windowMs
        }
    }

    /**
     * Record one signal. Any [kind] is accepted — the server does not
     * enumerate them, so a host can push its own without waiting for
     * an SDK release. The five event kinds are a different list and
     * are validated.
     */
    @JvmStatic
    @JvmOverloads
    fun push(kind: String, data: Map<String, Any?>? = null) {
        synchronized(lock) {
            entries.addLast(Entry(System.currentTimeMillis(), kind, data))
            while (entries.size > capacity) entries.removeFirst()
        }
    }

    /**
     * The ring relative to [now], windowed, oldest first — the shape
     * `payload.signals` carries. `t` is seconds before now, negative,
     * to one decimal.
     */
    @JvmStatic
    @JvmOverloads
    fun snapshot(now: Long = System.currentTimeMillis()): List<Map<String, Any?>> {
        val (all, window) = synchronized(lock) { entries.toList() to windowMs }
        val cutoff = now - window
        return all.filter { it.at >= cutoff }.map { e ->
            val out = mutableMapOf<String, Any?>(
                "t" to ((e.at - now) / 100.0).roundToLong() / 10.0,
                "kind" to e.kind,
            )
            if (e.data != null) out["data"] = e.data
            out
        }
    }

    @JvmStatic
    fun clear() {
        synchronized(lock) { entries.clear() }
    }
}
