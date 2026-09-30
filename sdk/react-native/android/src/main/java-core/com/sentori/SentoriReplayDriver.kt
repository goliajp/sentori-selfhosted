// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/main/java/com/sentori/SentoriReplayDriver.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import org.json.JSONObject

/**
 * Runs the replay: tick, capture, ring.
 *
 * Off unless the host asks for it. Replay is the most expensive thing
 * this SDK can do, and the rule for anything in that class is that the
 * customer turns it on deliberately rather than discovering it in a
 * profile.
 *
 * ## The timer is not on the main thread
 *
 * `SentoriReplayCapture.captureWireframe` reads the view hierarchy,
 * which only the main thread may touch, so it posts and waits when
 * called from elsewhere. A timer on the main Looper would post to the
 * queue it is already draining and wait for itself. The timer lives on
 * its own single-thread executor, where blocking costs nobody a frame.
 */
object SentoriReplayDriver {
    private val lock = Any()
    private var pool: ScheduledExecutorService? = null
    private var task: ScheduledFuture<*>? = null
    private var ring = SentoriReplay.Ring()

    /** Ticks per second. Two is enough to follow a user through a
     *  screen; four is for motion-heavy apps that would rather spend
     *  the CPU. */
    const val DEFAULT_HZ = 2.0

    /**
     * Start capturing. Calling it twice is a no-op rather than a
     * second timer — a host that calls start from two entry points
     * should not pay twice.
     */
    @JvmStatic
    @JvmOverloads
    fun start(hz: Double = DEFAULT_HZ, keyframeMs: Double = 4000.0) = synchronized(lock) {
        if (task != null) return
        ring = SentoriReplay.Ring(keyframeMs = keyframeMs)
        val periodMs = (1000.0 / maxOf(hz, 0.1)).toLong()
        val executor = pool ?: Executors.newSingleThreadScheduledExecutor { runnable ->
            // Daemon, so a host that forgets to stop us does not have
            // a thread holding its process open.
            Thread(runnable, "sentori-replay").apply { isDaemon = true }
        }.also { pool = it }
        task = executor.scheduleWithFixedDelay(::tick, periodMs, periodMs, TimeUnit.MILLISECONDS)
    }

    @JvmStatic
    fun stop() = synchronized(lock) {
        task?.cancel(false)
        task = null
    }

    /**
     * The window so far, as the newline-delimited JSON the player
     * reads, and start again cold. Empty when nothing was captured.
     */
    @JvmStatic
    fun drain(): String {
        val entries = ring.drain()
        if (entries.isEmpty()) return ""
        return entries.joinToString("\n") { it.toString() }
    }

    private fun tick() {
        // This runs on our own thread, but a throw here would cancel
        // the scheduled task and every tick after it — a replay that
        // silently stopped, which reads as a quiet app rather than as
        // a broken SDK. `scheduleWithFixedDelay` does exactly that on
        // an uncaught exception, with nothing logged.
        try {
            val json = SentoriReplayCapture.captureWireframe(SentoriMask.ids())
            if (json.isNullOrEmpty()) return
            val raw = JSONObject(json)
            val nodes = raw.optJSONArray("nodes")
            val parsed = (0 until (nodes?.length() ?: 0)).mapNotNull { i ->
                val item = nodes?.optJSONObject(i) ?: return@mapNotNull null
                SentoriReplay.Node(
                    x = item.optDouble("x", 0.0),
                    y = item.optDouble("y", 0.0),
                    w = item.optDouble("w", 0.0),
                    h = item.optDouble("h", 0.0),
                    kind = if (item.has("kind")) item.optString("kind") else null,
                    text = if (item.has("text")) item.optString("text") else null,
                    color = if (item.has("color")) item.optString("color") else null,
                )
            }
            ring.push(
                SentoriReplay.Frame(
                    ts = raw.optDouble("ts", System.currentTimeMillis().toDouble()),
                    width = raw.optDouble("width", 0.0),
                    height = raw.optDouble("height", 0.0),
                    nodes = parsed,
                )
            )
        } catch (t: Throwable) {
            android.util.Log.w("sentori", "replay tick failed; capture continues: $t")
        }
    }

    @JvmStatic
    fun __isRunningForTests(): Boolean = synchronized(lock) { task != null }

    @JvmStatic
    fun __pushForTests(frame: SentoriReplay.Frame) {
        ring.push(frame)
    }

    @JvmStatic
    fun __resetForTests() {
        stop()
        ring.drain()
    }
}
