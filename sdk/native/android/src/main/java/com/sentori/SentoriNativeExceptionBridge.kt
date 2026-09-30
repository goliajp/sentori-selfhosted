package com.sentori

import java.util.concurrent.ConcurrentLinkedDeque
import java.util.concurrent.atomic.AtomicLong

/**
 * v0.9.5 #8 — partial fix for TurboModule swallowing native
 * exceptions into a generic JSError.
 *
 * Android side mirrors the iOS implementation. Host TurboModule code:
 *
 *   try {
 *     riskyOperation()
 *   } catch (e: Exception) {
 *     SentoriNativeExceptionBridge.record(e)
 *     throw e
 *   }
 *
 * JS-side coerceError calls `getRecent()` and, if an exception
 * within the last 1 s exists, attaches its stack to the resulting
 * sentori event.
 */
object SentoriNativeExceptionBridge {

    private const val RING_SIZE = 8
    private const val WINDOW_MS = 1_000L

    private data class Stash(
        val timestamp: Long,
        val name: String,
        val reason: String,
        val stack: List<String>,
    )

    private val ring = ConcurrentLinkedDeque<Stash>()
    private val lastPurgeAt = AtomicLong(0)

    @JvmStatic
    fun record(t: Throwable) {
        val frames = t.stackTrace.take(48).map { it.toString() }
        val stash = Stash(
            timestamp = System.currentTimeMillis(),
            name = t.javaClass.simpleName,
            reason = t.message ?: "",
            stack = frames,
        )
        ring.addLast(stash)
        while (ring.size > RING_SIZE) ring.pollFirst()
        purgeIfDue()
    }

    @JvmStatic
    fun getRecent(): Map<String, Any?>? {
        purgeIfDue()
        val latest = ring.peekLast() ?: return null
        return mapOf(
            "name" to latest.name,
            "reason" to latest.reason,
            "stack" to latest.stack,
            "ageMs" to (System.currentTimeMillis() - latest.timestamp).toInt(),
        )
    }

    private fun purgeIfDue() {
        val now = System.currentTimeMillis()
        val last = lastPurgeAt.get()
        if (now - last < 100) return
        lastPurgeAt.set(now)
        val cutoff = now - WINDOW_MS
        val it = ring.iterator()
        while (it.hasNext()) {
            if (it.next().timestamp < cutoff) it.remove()
        }
    }
}
