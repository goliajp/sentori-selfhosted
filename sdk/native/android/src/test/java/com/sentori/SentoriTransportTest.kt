package com.sentori

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The spill is the last place a failed batch can go. When it cannot be
 * written the events are gone, and the only honest record of that is the
 * counter the next envelope carries — the same one the in-memory cap
 * uses.
 *
 * Deliberately the same assertion as
 * `testASpillThatCannotBeWrittenIsCountedRatherThanLost` in
 * `SentoriTransportTests.swift`: the two platforms disagreeing about
 * what a dropped event is would show up as a dashboard that counts one
 * fleet and not the other.
 */
@RunWith(RobolectricTestRunner::class)
class SentoriTransportTest {

    @Before
    fun setUp() {
        SentoriTransport.resetForTests()
        SentoriConfig.resetForTests()
        SentoriScope.clear()
        SentoriSignalRing.clear()
    }

    @After
    fun tearDown() {
        SentoriTransport.resetForTests()
        SentoriConfig.resetForTests()
    }

    private fun waitUntil(what: String, timeoutMs: Long = 10_000, condition: () -> Boolean) {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (!condition() && System.currentTimeMillis() < deadline) Thread.sleep(50)
        assertTrue(
            "$what — not after ${timeoutMs / 1000}s; persisted: " +
                "${SentoriTransport.peekPersisted().size}, " +
                "queued: ${SentoriTransport.peekQueue().size}, " +
                "dropped: ${SentoriTransport.peekDropped()}",
            condition(),
        )
    }

    @Test
    fun theEventsAServerRefusedInsideA200AreCounted() {
        // The batch endpoint answers 200 and puts each refusal in that
        // event's outcome. Reading only the status counted a refusal as
        // a delivery — the shape a self-hosted server takes when it is
        // older than the SDK talking to it.
        val body = """{"accepted":1,"outcomes":[{},{"error":"invalid_payload","detail":"platform"}]}"""
        assertEquals(1, SentoriTransport.refusedCount(body))

        // Same assertion as `testRefusedEventsInsideA200AreCounted` in
        // SentoriTransportTests.swift: the two platforms disagreeing
        // about what a refusal is would count one fleet and not the
        // other.
        assertEquals(0, SentoriTransport.refusedCount("""{"accepted":2,"outcomes":[{},{}]}"""))
        // A 2xx we cannot read says nothing about the items.
        assertEquals(0, SentoriTransport.refusedCount("not json"))
    }

    @Test
    fun aSpillThatCannotBeWrittenIsCountedRatherThanLost() {
        SentoriConfig.set(
            SentoriConfig(
                token = "st_test",
                ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0",
                environment = "test",
            ),
        )
        // Force the failure rather than arranging for it, and start
        // without a spill directory: that is the in-memory-only mode,
        // where a failed batch has nowhere to go.
        SentoriTransport.forcedOutcomeForTests = 2 // failed
        SentoriTransport.start(null)
        repeat(3) { SentoriTransport.enqueue(mapOf("kind" to "error", "seq" to it)) }
        SentoriTransport.flush()

        waitUntil("the batch the spill could not take is counted") {
            SentoriTransport.peekDropped() == 3
        }
        assertEquals(
            "and nothing was written, which is the premise",
            0,
            SentoriTransport.peekPersisted().size,
        )
    }
}
