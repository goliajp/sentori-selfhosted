package com.sentori

import androidx.test.core.app.ApplicationProvider
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The screenshot taken as the app died, and the view tree behind it.
 *
 * Both were already being captured and written into the crash file on
 * both platforms — and then nothing uploaded them. The event arrived,
 * the evidence did not, and the dashboard showed a crash with an empty
 * viewport that looked like a capture bug.
 *
 * Two things can go wrong here and neither shows up as an error:
 * uploading before the server has the event (a 404 nobody reads), and
 * refusing a kind the server would have accepted.
 *
 * Mirrors `SentoriAttachmentTests.swift`.
 */
@RunWith(RobolectricTestRunner::class)
class SentoriAttachmentTest {

    private val uploads = mutableListOf<Triple<String, String, String>>()

    @Before
    fun setUp() {
        SentoriTransport.resetForTests()
        SentoriConfig.resetForTests()
        SentoriCrashHandler.consumePending()
        uploads.clear()
        SentoriAttachment.recorderForTests = { id, kind, body ->
            synchronized(uploads) { uploads.add(Triple(id, kind, body)) }
        }
    }

    @After
    fun tearDown() {
        SentoriAttachment.recorderForTests = null
        SentoriTransport.resetForTests()
        SentoriConfig.resetForTests()
    }

    /**
     * `start()` with no directory leaves the spill a no-op, which made
     * the "nothing uploads when the batch never lands" test wait five
     * seconds for a file that could never be written — a test failing
     * on its own setup while the code under it was correct.
     */
    private fun spillDir(): File =
        File(
            ApplicationProvider.getApplicationContext<android.content.Context>().cacheDir,
            "sentori-spill",
        )
            .apply {
                deleteRecursively()
                mkdirs()
            }

    private fun configure() {
        SentoriConfig.set(
            SentoriConfig(
                token = "st_test",
                ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0",
                environment = "test",
            ),
        )
    }

    /**
     * Poll rather than sleep: a fixed wait encodes a guess about how
     * fast the machine is, and CI is slower than this one.
     *
     * The default is generous because a passing wait returns
     * immediately and only a failing one spends the budget. Five
     * seconds bought nothing and cost two red builds on the iOS side.
     */
    private fun waitUntil(what: String, timeoutMs: Long = 30_000, cond: () -> Boolean) {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            if (cond()) return
            Thread.sleep(20)
        }
        throw AssertionError("$what — not after ${timeoutMs}ms")
    }

    /** A crash file with a screenshot in it, as the handler writes it. */
    private fun crashFile(id: String): JSONObject =
        JSONObject().apply {
            put("id", id)
            put("timestamp", "2026-08-11T09:00:00.000Z")
            put("kind", "error")
            put("platform", "android")
            put("release", "app@1.0.0")
            put("environment", "test")
            put(
                "device",
                JSONObject().apply {
                    put("os", "android")
                    put("osVersion", "15")
                    put("model", "Pixel 10 Pro")
                },
            )
            put("app", JSONObject().apply { put("version", "1.0.0") })
            put(
                "error",
                JSONObject().apply {
                    put("type", "IllegalStateException")
                    put("message", "boom")
                },
            )
            put(
                "_pendingAttachments",
                JSONArray().apply {
                    put(
                        JSONObject().apply {
                            put("kind", "screenshot")
                            put("base64", "aGVsbG8=")
                            put("mediaType", "image/webp")
                            put("source", "android")
                        },
                    )
                    put(
                        JSONObject().apply {
                            put("kind", "viewTree")
                            put("base64", "eyJhIjoxfQ==")
                            put("mediaType", "application/json")
                            put("source", "android")
                        },
                    )
                },
            )
        }

    // ── the allowlist ─────────────────────────────────────────────

    /**
     * The first version of this list had three entries — `replay`,
     * `screens`, `screenshot` — and the crash handler writes
     * `viewTree`. The delivery path would have dropped, on the way
     * out, the very evidence it exists to deliver.
     */
    @Test
    fun everyKindTheCrashHandlerWritesIsAccepted() {
        for (kind in listOf("screenshot", "viewTree")) {
            assertTrue(
                "the crash handler writes '$kind' and this list would refuse it",
                kind in SentoriAttachment.KNOWN,
            )
        }
    }

    /**
     * Kept in step with `KINDS` in
     * `self-hosted/server/src/handlers/sdk/events_attachments.rs`,
     * where anything else is a 400 and the CHECK constraint behind it
     * would refuse the row anyway.
     */
    @Test
    fun theAllowlistIsTheServersList() {
        assertEquals(
            setOf(
                "logTail",
                "replay",
                "screens",
                "screenshot",
                "sessionTrail",
                "stateSnapshot",
                "viewTree",
            ),
            SentoriAttachment.KNOWN,
        )
        assertEquals(setOf("android", "ios", "js"), SentoriAttachment.KNOWN_SOURCES)
    }

    @Test
    fun anUnknownKindIsDroppedRatherThanPosted() {
        configure()
        SentoriAttachment.upload("e1", "heapDump", "aGk=", "application/octet-stream")
        assertTrue(uploads.isEmpty())
    }

    @Test
    fun nothingIsPostedWithoutAConfig() {
        SentoriAttachment.upload("e1", "screenshot", "aGk=", "image/webp")
        assertTrue(uploads.isEmpty())
    }

    // ── the body ──────────────────────────────────────────────────

    /**
     * Built by hand, so it is worth reading back. The failure mode is
     * a 2xx that stored the wrong bytes: without the transfer encoding
     * the server keeps the base64 *text* as the image, which renders
     * as nothing and reads as a capture problem.
     */
    @Test
    fun multipartBodyIsWhatTheServerParses() {
        val body =
            SentoriAttachment.multipartBody(
                "BOUND",
                "screenshot",
                "image/webp",
                "aGVsbG8=",
                "android",
            )

        assertTrue(body.startsWith("--BOUND\r\n"))
        assertTrue(body.endsWith("--BOUND--\r\n"))
        assertTrue(
            body.contains(
                "Content-Disposition: form-data; name=\"file\"; filename=\"screenshot.bin\"\r\n",
            ),
        )
        assertTrue(body.contains("Content-Type: image/webp\r\n"))
        assertTrue(body.contains("Content-Transfer-Encoding: base64\r\n"))
        assertTrue(body.contains("\r\n\r\naGVsbG8=\r\n"))
        assertTrue(
            body.contains("Content-Disposition: form-data; name=\"source\"\r\n\r\nandroid\r\n"),
        )

        // Two opening delimiters, no more and no fewer.
        assertEquals(2, body.split("--BOUND\r\n").size - 1)
    }

    /** The bytes are identical to what the Swift package would send. */
    @Test
    fun theBodyMatchesTheOneSwiftBuilds() {
        val body =
            SentoriAttachment.multipartBody("B", "viewTree", "application/json", "eyJhIjoxfQ==", "ios")
        assertEquals(
            "--B\r\n" +
                "Content-Disposition: form-data; name=\"file\"; filename=\"viewTree.bin\"\r\n" +
                "Content-Type: application/json\r\n" +
                "Content-Transfer-Encoding: base64\r\n" +
                "\r\neyJhIjoxfQ==\r\n" +
                "--B\r\n" +
                "Content-Disposition: form-data; name=\"source\"\r\n" +
                "\r\nios\r\n" +
                "--B--\r\n",
            body,
        )
    }

    // ── the ordering ──────────────────────────────────────────────

    /**
     * The one that matters. An upload keyed on an event the server has
     * not seen is a 404, and it wins that race every time: the batch
     * waits for a flush and the upload does not.
     */
    @Test
    fun attachmentsUploadOnlyAfterTheBatchLands() {
        configure()
        SentoriCrashHandler.installForTesting(ApplicationProvider.getApplicationContext())
        SentoriTransport.start(spillDir())
        SentoriTransport.forcedOutcomeForTests = 0 // DELIVERED

        val id = "019ff080-2aeb-7e30-aba1-4431b296d120"
        SentoriCrashHandler.persistRawForTesting(crashFile(id))
        SentoriPendingCrash.ship()

        waitUntil("both blobs upload after the batch lands") {
            synchronized(uploads) { uploads.size == 2 }
        }

        assertEquals(setOf("screenshot", "viewTree"), uploads.map { it.second }.toSet())
        for (upload in uploads) {
            assertEquals("an attachment keyed on anything but the event id is a 404", id, upload.first)
        }
        assertTrue(uploads.any { it.third.contains("aGVsbG8=") })
        assertTrue(uploads.any { it.third.contains("Content-Type: application/json") })
    }

    /**
     * If the batch was refused or spilled to disk, the events are not
     * on the server and there is nothing to attach to. Uploading
     * anyway spends the user's bandwidth on a guaranteed 404.
     */
    @Test
    fun nothingUploadsWhenTheBatchNeverLands() {
        configure()
        SentoriCrashHandler.installForTesting(ApplicationProvider.getApplicationContext())
        SentoriTransport.start(spillDir())
        SentoriTransport.forcedOutcomeForTests = 2 // FAILED

        val spilledId = "019ff080-2aeb-7e30-aba1-4431b296d121"
        SentoriCrashHandler.persistRawForTesting(crashFile(spilledId))
        SentoriPendingCrash.ship()

        // The event has to reach the spill before the absence of an
        // upload means anything — otherwise this passes by being early.
        //
        // Ask for this crash, not for a count: the spill is one file
        // the whole process shares, so a size check is an assertion
        // about what every other test left behind.
        waitUntil("the failed batch spills", 60_000) {
            SentoriTransport.peekPersisted().any { it["id"] as? String == spilledId }
        }

        assertTrue(
            "a spilled batch has no event on the server to attach to",
            synchronized(uploads) { uploads.isEmpty() },
        )
    }

    // ── the hook itself ───────────────────────────────────────────

    /** Registered for events that are still queued, it waits. */
    @Test
    fun aHookWaitsUntilItsOwnEventsLand() {
        configure()
        SentoriTransport.start(spillDir())
        SentoriTransport.forcedOutcomeForTests = 0 // DELIVERED

        var fired = 0
        SentoriTransport.afterDelivery(setOf("a", "b")) { fired += 1 }

        SentoriTransport.enqueue(mapOf("id" to "a", "kind" to "error"))
        SentoriTransport.flush()
        waitUntil("the first batch lands") { SentoriTransport.peekDelivered() >= 1 }
        assertEquals("half its events have landed — it is not due yet", 0, fired)

        SentoriTransport.enqueue(mapOf("id" to "b", "kind" to "error"))
        SentoriTransport.flush()
        waitUntil("the hook fires once both have landed") { fired == 1 }
    }

    /**
     * Registered for events the server refuses outright, it is
     * dropped: there is nothing on the server to attach to, and
     * uploading anyway spends a device's bandwidth on a 404.
     */
    @Test
    fun aHookIsDiscardedWhenTheServerRefusesItsEvents() {
        configure()
        SentoriTransport.start(spillDir())
        SentoriTransport.forcedOutcomeForTests = 1 // DROPPED

        var fired = false
        SentoriTransport.afterDelivery(setOf("a")) { fired = true }
        assertEquals(1, SentoriTransport.peekWaiting())

        SentoriTransport.enqueue(mapOf("id" to "a", "kind" to "error"))
        SentoriTransport.flush()

        waitUntil("the refusal discards the waiter") { SentoriTransport.peekWaiting() == 0 }
        assertFalse("the events it waited on were refused", fired)
    }

    /**
     * The shape of the bug CI caught. A hook registered after its
     * batch has already been accepted never fires — which is why
     * `ship` registers before it enqueues, not after it flushes.
     */
    @Test
    fun aHookRegisteredTooLateNeverFires() {
        configure()
        SentoriTransport.start(spillDir())
        SentoriTransport.forcedOutcomeForTests = 0

        SentoriTransport.enqueue(mapOf("id" to "a", "kind" to "error"))
        SentoriTransport.flush()
        waitUntil("the batch lands") { SentoriTransport.peekDelivered() >= 1 }

        var fired = false
        SentoriTransport.afterDelivery(setOf("a")) { fired = true }
        SentoriTransport.enqueue(mapOf("id" to "z", "kind" to "error"))
        SentoriTransport.flush()
        waitUntil("a later batch lands") { SentoriTransport.peekDelivered() >= 2 }

        assertFalse(
            "it fired on someone else's batch — an upload keyed on an event that " +
                "batch never carried is a 404",
            fired,
        )
    }

    /**
     * The blobs travel in the file and never on the wire. An event
     * carrying a base64 screenshot inside its JSON is a megabyte in a
     * batch that is meant to be a few kilobytes.
     */
    @Test
    fun pendingAttachmentsNeverGoOnTheWire() {
        configure()
        SentoriCrashHandler.installForTesting(ApplicationProvider.getApplicationContext())
        SentoriTransport.forcedOutcomeForTests = 0
        SentoriCrashHandler.persistRawForTesting(crashFile("019ff080-2aeb-7e30-aba1-4431b296d122"))
        SentoriPendingCrash.ship()

        val queued = SentoriTransport.peekQueue() + SentoriTransport.peekPersisted()
        for (event in queued) {
            assertFalse(
                "the screenshot bytes must not ride inside the event",
                event.toString().contains("aGVsbG8="),
            )
        }
    }
}
