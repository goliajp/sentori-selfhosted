package com.sentori

import android.os.Process
import android.os.SystemClock
import android.view.Choreographer
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

/**
 * v0.9.4 #1 — Mobile Vitals.
 *
 * Cold start uses `Process.getStartElapsedRealtime()` (API 24+) —
 * the time the system started the process clock, anchored to boot
 * time. Subtract from current `SystemClock.elapsedRealtime()` at JS
 * bridge ready to get the user-perceived launch budget.
 *
 * Frame counters hook `Choreographer.postFrameCallback` and compare
 * deltas. 16.67ms = slow, 700ms = frozen — same thresholds as iOS
 * for parity. The choreographer callback runs on the UI thread so we
 * don't need extra synchronization for the AtomicInteger counters.
 */
object SentoriMobileVitals {

    private const val SLOW_FRAME_NS: Long = 16_670_000  // 16.67 ms
    private const val FROZEN_FRAME_NS: Long = 700_000_000 // 700 ms

    private val jsBridgeReadyAt = AtomicLong(0)
    private val coldStartMs = AtomicLong(-1)

    private val slowFrames = AtomicInteger(0)
    private val frozenFrames = AtomicInteger(0)
    private var lastFrameNs: Long = 0L
    @Volatile private var frameWatchRunning = false
    private var callback: Choreographer.FrameCallback? = null

    /** Called when JS init runs. Uses Process start elapsed realtime
     *  as the anchor; available API 24+. */
    @JvmStatic
    fun markJsBridgeReady() {
        if (jsBridgeReadyAt.get() != 0L) return
        val now = SystemClock.elapsedRealtime()
        jsBridgeReadyAt.set(now)
        try {
            val processStart = Process.getStartElapsedRealtime()
            coldStartMs.set(now - processStart)
        } catch (_: Throwable) {
            // API < 24 — leave -1 sentinel
        }
    }

    @JvmStatic
    fun getColdStartMs(): Long? {
        val v = coldStartMs.get()
        return if (v < 0) null else v
    }

    @JvmStatic
    fun startFrameWatch() {
        if (frameWatchRunning) return
        frameWatchRunning = true
        val cb = object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                if (!frameWatchRunning) return
                if (lastFrameNs != 0L) {
                    val delta = frameTimeNanos - lastFrameNs
                    if (delta >= FROZEN_FRAME_NS) {
                        frozenFrames.incrementAndGet()
                    } else if (delta >= SLOW_FRAME_NS) {
                        slowFrames.incrementAndGet()
                    }
                }
                lastFrameNs = frameTimeNanos
                Choreographer.getInstance().postFrameCallback(this)
            }
        }
        callback = cb
        // Choreographer must be subscribed on the UI thread. Caller
        // is expected to run this on the main thread (SentoriModule
        // OnCreate runs on the main thread for Expo Modules).
        Choreographer.getInstance().postFrameCallback(cb)
    }

    @JvmStatic
    fun stopFrameWatch() {
        frameWatchRunning = false
        callback?.let { Choreographer.getInstance().removeFrameCallback(it) }
        callback = null
    }

    @JvmStatic
    fun getFrameCounters(): Map<String, Int> {
        return mapOf("slow" to slowFrames.get(), "frozen" to frozenFrames.get())
    }

    @JvmStatic
    fun resetFrameCounters() {
        slowFrames.set(0)
        frozenFrames.set(0)
    }
}
