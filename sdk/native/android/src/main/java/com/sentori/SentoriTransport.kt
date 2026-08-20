package com.sentori

import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import org.json.JSONArray
import org.json.JSONObject

/**
 * The only place the SDK talks to the network.
 *
 * Ported from `sdk/react-native/src/transport.ts` and matching the
 * Swift one: batch on a 5 s timer or a 10-deep queue, whichever comes
 * first; three attempts with a doubling delay; a 429 waits out
 * `retryAfterMs`; anything left after that spills to disk and drains
 * on the next launch.
 *
 * The iron rule is harder here than in JavaScript. A JS verb returns
 * because everything after it is a microtask; the JVM has no such
 * floor, so "fire and forget" is a real queue on a real background
 * thread. [enqueue] does one bounded append under a lock and returns —
 * no encoding, no I/O.
 *
 * `HttpURLConnection` rather than OkHttp: a monitoring SDK that drags
 * a networking library into every host app has already broken the
 * footprint half of its own contract, and OkHttp would also mean this
 * SDK's traffic goes through the same interceptors the host installed
 * for its own — which is a surprising place for a crash report to be
 * logged, retried or blocked.
 */
object SentoriTransport {

    private const val FLUSH_INTERVAL_MS = 5_000L
    private const val BATCH_SIZE = 10
    private const val MAX_RETRY = 3
    private const val MAX_PERSISTED = 1000

    /**
     * Bounded, because an unbounded in-memory queue is a leak with a
     * nicer name. Past it the oldest go: a crash from ten minutes ago
     * matters less than the one happening now.
     */
    private const val MAX_QUEUED = 500

    private val lock = Any()

    /**
     * Work that may only run once the server has taken particular
     * events.
     *
     * Attachments are the reason: the server keys one on an event id
     * it must already know, so uploading before that event lands is a
     * guaranteed 404.
     *
     * Keyed on ids rather than "the next delivery", which was the
     * first version and had a race at each end. `flush` hands the send
     * to a worker and returns, so registering afterwards can miss a
     * batch that has already come back — on a fast network the uploads
     * then never happen at all, silently, which is the bug this exists
     * to prevent. Registering beforehand instead lets an unrelated
     * batch already in flight fire the block early, into a 404.
     * Matching on ids has neither end.
     */
    private val afterDelivery = mutableListOf<Pair<Set<String>, () -> Unit>>()

    /**
     * Run [block] once the server has accepted every event in [ids].
     * If the server refuses them outright the block is discarded — they
     * are not there to attach to. A batch that merely spilled keeps its
     * waiters, since the drain retries it under the same ids.
     */
    internal fun afterDelivery(ids: Set<String>, block: () -> Unit) {
        if (ids.isEmpty()) return
        synchronized(lock) { afterDelivery.add(ids to block) }
    }

    private fun settle(events: List<Map<String, Any?>>, accepted: Boolean) {
        val ids = events.mapNotNull { it["id"] as? String }.toSet()
        if (ids.isEmpty()) return
        val ready = mutableListOf<() -> Unit>()
        synchronized(lock) {
            val kept = mutableListOf<Pair<Set<String>, () -> Unit>>()
            for ((waiting, block) in afterDelivery) {
                if (waiting.none { id -> id in ids }) {
                    kept.add(waiting to block)
                    continue
                }
                if (!accepted) continue // refused: drop the waiter
                // Subtract what landed rather than asking whether this
                // one batch carried everything. A waiter on two events
                // whose events go out in two batches is otherwise never
                // due — it is a subset of neither.
                val left = waiting - ids
                if (left.isEmpty()) ready.add(block) else kept.add(left to block)
            }
            afterDelivery.clear()
            afterDelivery.addAll(kept)
        }
        ready.forEach { it() }
    }
    private val queue = ArrayDeque<Map<String, Any?>>()
    private val assertStats = mutableMapOf<String, MutableMap<String, Any?>>()
    private var started = false
    private var dropped = 0
    private var delivered = 0
    private var pending: ScheduledFuture<*>? = null

    /** Where the spill lives. Set by [start]; null keeps it in memory only. */
    private var spillDir: File? = null

    private val worker: ScheduledExecutorService =
        Executors.newSingleThreadScheduledExecutor { r ->
            Thread(r, "sentori-transport").apply { isDaemon = true }
        }

    /** O(1) on the calling thread: append, maybe schedule. */
    @JvmStatic
    fun enqueue(event: Map<String, Any?>) {
        val due: Boolean
        synchronized(lock) {
            queue.addLast(event)
            while (queue.size > MAX_QUEUED) {
                queue.removeFirst()
                dropped += 1
            }
            due = queue.size >= BATCH_SIZE
        }
        if (due) worker.execute { flush() } else scheduleFlush(FLUSH_INTERVAL_MS)
    }

    /**
     * Assert outcomes aggregate rather than becoming events, and ride
     * whatever batch goes out next — the liveness ledger without a
     * heartbeat flood.
     */
    @JvmStatic
    fun countAssert(name: String, ok: Boolean, release: String) {
        val idle: Boolean
        synchronized(lock) {
            val key = "$name$release"
            val stat =
                assertStats.getOrPut(key) {
                    mutableMapOf(
                        "name" to name,
                        "release" to release,
                        "passDelta" to 0,
                        "failDelta" to 0,
                    )
                }
            val field = if (ok) "passDelta" else "failDelta"
            stat[field] = (stat[field] as Int) + 1
            idle = queue.isEmpty()
        }
        if (idle) scheduleFlush(FLUSH_INTERVAL_MS * 6)
    }

    @JvmStatic
    @JvmOverloads
    fun start(spill: File? = null) {
        synchronized(lock) {
            started = true
            spillDir = spill
        }
        worker.execute { drainPersisted() }
    }

    @JvmStatic
    fun flush() {
        // Before touching the queue, not after: draining first and
        // checking second destroys anything enqueued before init.
        val config = SentoriConfig.current ?: return

        val events: List<Map<String, Any?>>
        val stats: List<Map<String, Any?>>
        val lost: Int
        synchronized(lock) {
            if (!started || (queue.isEmpty() && assertStats.isEmpty())) return
            events = queue.toList()
            stats = assertStats.values.map { it.toMap() }
            lost = dropped
            queue.clear()
            assertStats.clear()
            dropped = 0
            pending?.cancel(false)
            pending = null
        }

        val envelope = mutableMapOf<String, Any?>("events" to events)
        if (stats.isNotEmpty()) envelope["assertStats"] = stats
        config.backendHealthUrl?.let { envelope["backendHealthUrl"] = it }
        // Say so rather than let the gap look like quiet: a hole in a
        // timeline otherwise reads as a calm period.
        if (lost > 0) envelope["droppedEvents"] = lost

        worker.execute {
            when (sendWithRetry(envelope, config)) {
                Outcome.DELIVERED -> {
                    synchronized(lock) { delivered += events.size }
                    settle(events, accepted = true)
                }
                // Handled, but not accepted. Nothing to retry and
                // nothing to count.
                Outcome.DROPPED -> settle(events, accepted = false)
                // Spilled, not lost: `drainPersisted` puts these back
                // through this path on the next start with the same
                // ids, so anything waiting on them keeps waiting.
                else -> persist(events)
            }
        }
    }

    // ── the network ───────────────────────────────────────────────

    private enum class Outcome {
        DELIVERED,
        DROPPED,
        RETRY_AFTER,
        FAILED,
    }

    private var lastRetryAfterMs = 5_000L

    /**
     * Returns the terminal outcome rather than a bool. "Stop trying"
     * would conflate a 2xx with a 4xx — not worth retrying is not the
     * same as delivered, and a counter built on the bool counts a
     * rejected batch as a success.
     */
    private fun sendWithRetry(envelope: Map<String, Any?>, config: SentoriConfig): Outcome {
        var delay = 1_000L
        repeat(MAX_RETRY) { attempt ->
            when (val outcome = sendOnce(envelope, config)) {
                Outcome.DELIVERED,
                Outcome.DROPPED -> return outcome
                Outcome.RETRY_AFTER -> {
                    if (attempt == MAX_RETRY - 1) return outcome
                    sleep(lastRetryAfterMs)
                }
                Outcome.FAILED -> {
                    if (attempt == MAX_RETRY - 1) return outcome
                    sleep(delay)
                    delay *= 2
                }
            }
        }
        return Outcome.FAILED
    }

    /**
     * Test seam: skip the network and return this outcome.
     * `0` delivered, `1` dropped, anything else failed.
     *
     * The iOS suite arrived here after two attempts to provoke a real
     * failure by sending to a closed port: one CI runner dropped the
     * packet and waited out the timeout, another answered in a way
     * that read as a 4xx — and both times a working transport was
     * reported broken. What these tests are about is the spill, the
     * drain and the ordering, not TCP; `android-live-ingest` is what
     * exercises the wire.
     */
    internal var forcedOutcomeForTests: Int? = null

    private fun sendOnce(envelope: Map<String, Any?>, config: SentoriConfig): Outcome {
        forcedOutcomeForTests?.let {
            return when (it) {
                0 -> Outcome.DELIVERED
                1 -> Outcome.DROPPED
                else -> Outcome.FAILED
            }
        }
        val body =
            try {
                toJson(envelope).toString()
            } catch (_: Throwable) {
                // Unencodable content is our problem, not the host's,
                // and it will not become encodable on a retry.
                return Outcome.DROPPED
            }

        var conn: HttpURLConnection? = null
        return try {
            conn = URL("${config.ingestUrl}/v1/events:batch").openConnection() as HttpURLConnection
            conn.requestMethod = "POST"
            conn.setRequestProperty("Content-Type", "application/json")
            conn.setRequestProperty("Authorization", "Bearer ${config.token}")
            conn.setRequestProperty("Sentori-Sdk", "kotlin/${SentoriVersion.CURRENT}")
            conn.connectTimeout = 15_000
            conn.readTimeout = 15_000
            conn.doOutput = true
            conn.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }

            when (val code = conn.responseCode) {
                in 200..299 -> Outcome.DELIVERED
                429 -> {
                    lastRetryAfterMs =
                        try {
                            val text = conn.errorStream?.bufferedReader()?.readText().orEmpty()
                            JSONObject(text).optLong("retryAfterMs", 5_000L)
                        } catch (_: Throwable) {
                            5_000L
                        }
                    Outcome.RETRY_AFTER
                }
                in 500..599 -> Outcome.FAILED
                else -> {
                    // 4xx other than 429: our request is wrong and
                    // will be wrong again. Per-item outcomes are the
                    // server's business.
                    if (code >= 400) Outcome.DROPPED else Outcome.FAILED
                }
            }
        } catch (_: Throwable) {
            Outcome.FAILED
        } finally {
            conn?.disconnect()
        }
    }

    private fun sleep(ms: Long) {
        try {
            TimeUnit.MILLISECONDS.sleep(ms)
        } catch (_: InterruptedException) {
            Thread.currentThread().interrupt()
        }
    }

    // ── json ──────────────────────────────────────────────────────

    /**
     * Hand-rolled rather than a serialization library, for the same
     * reason as `HttpURLConnection`: `org.json` is in the platform, so
     * it costs the host nothing.
     */
    internal fun toJson(value: Any?): Any {
        return when (value) {
            null -> JSONObject.NULL
            is Map<*, *> -> {
                val o = JSONObject()
                for ((k, v) in value) o.put(k.toString(), toJson(v))
                o
            }
            is List<*> -> {
                val a = JSONArray()
                for (v in value) a.put(toJson(v))
                a
            }
            is Number, is Boolean, is String -> value
            else -> value.toString()
        }
    }

    // ── the offline queue ─────────────────────────────────────────

    private fun spillFile(): File? {
        val dir = synchronized(lock) { spillDir } ?: return null
        if (!dir.exists()) dir.mkdirs()
        return File(dir, "pending-events.json")
    }

    private fun persist(events: List<Map<String, Any?>>) {
        val file = spillFile() ?: return
        try {
            val all = readPersisted().toMutableList()
            all.addAll(events)
            // Newest wins: the file has to stop growing on a device
            // that is offline for a week.
            while (all.size > MAX_PERSISTED) all.removeAt(0)
            file.writeText(toJson(all).toString())
        } catch (_: Throwable) {
            // A full disk must not become the host's problem.
        }
    }

    private fun readPersisted(): List<Map<String, Any?>> {
        val file = spillFile() ?: return emptyList()
        if (!file.exists()) return emptyList()
        return try {
            val arr = JSONArray(file.readText())
            (0 until arr.length()).map { fromJson(arr.getJSONObject(it)) }
        } catch (_: Throwable) {
            emptyList()
        }
    }

    private fun fromJson(o: JSONObject): Map<String, Any?> {
        val out = mutableMapOf<String, Any?>()
        for (k in o.keys()) {
            out[k] =
                when (val v = o.get(k)) {
                    is JSONObject -> fromJson(v)
                    JSONObject.NULL -> null
                    else -> v
                }
        }
        return out
    }

    /**
     * Read the spill, clear it, put the events back through the normal
     * path — clearing first, so a batch that fails again spills once
     * rather than doubling.
     */
    private fun drainPersisted() {
        val pendingEvents = readPersisted()
        if (pendingEvents.isEmpty()) return
        spillFile()?.delete()
        pendingEvents.forEach { enqueue(it) }
        flush()
    }

    // ── timer ─────────────────────────────────────────────────────

    private fun scheduleFlush(afterMs: Long) {
        synchronized(lock) {
            if (pending != null) return
            pending =
                worker.schedule(
                    {
                        synchronized(lock) { pending = null }
                        flush()
                    },
                    afterMs,
                    TimeUnit.MILLISECONDS,
                )
        }
    }

    // ── test seams ────────────────────────────────────────────────

    internal fun resetForTests() {
        synchronized(lock) {
            spillFile()?.delete()
            queue.clear()
            assertStats.clear()
            dropped = 0
            delivered = 0
            afterDelivery.clear()
            forcedOutcomeForTests = null
            started = false
            pending?.cancel(false)
            pending = null
            spillDir = null
        }
    }

    internal fun peekQueue(): List<Map<String, Any?>> = synchronized(lock) { queue.toList() }

    internal fun peekAssertStats(): List<Map<String, Any?>> =
        synchronized(lock) { assertStats.values.map { it.toMap() } }

    internal fun peekPersisted(): List<Map<String, Any?>> = readPersisted()

    /**
     * Events the server actually accepted. A test that checks only for
     * the absence of a spill passes while retries are still in flight
     * — which is how the iOS live suite went green against a server
     * that had stored nothing.
     */
    internal fun peekDelivered(): Int = synchronized(lock) { delivered }

    /**
     * How many blocks are still waiting on events. Without this a test
     * cannot tell "discarded because the server refused it" from "has
     * not been reached yet", and the version that tried instead
     * flipped the forced outcome mid-send — measuring the race rather
     * than the rule.
     */
    internal fun peekWaiting(): Int = synchronized(lock) { afterDelivery.size }
}
