// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/test/java/com/sentori/SentoriVerbsTest.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The five verbs, and the contract an integrator is entitled to:
 * synchronous, an id back every time, never a throw, and a no-op
 * before [Sentori.start] rather than a crash.
 *
 * Deliberately the same assertions as `SentoriVerbsTests.swift`. The
 * dashboard cannot tell which platform sent an event and should not
 * have to, so the two suites disagreeing is itself the bug.
 */
@RunWith(RobolectricTestRunner::class)
class SentoriVerbsTest {

    @Before
    fun setUp() {
        SentoriTransport.resetForTests()
        SentoriConfig.resetForTests()
        SentoriScope.clear()
        SentoriSignalRing.clear()
        SentoriDevice.resetForTests()
    }

    @After
    fun tearDown() {
        SentoriTransport.resetForTests()
        SentoriConfig.resetForTests()
    }

    private fun started() {
        // Port 9 is discard: nothing listens, so a send fails without
        // waiting on DNS.
        Sentori.start(
            SentoriConfig(
                token = "st_test",
                ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0",
                environment = "test",
            )
        )
    }

    private fun queued() = SentoriTransport.peekQueue()

    @Test
    fun everyVerbBeforeStartIsASilentNoOpThatStillReturnsAnId() {
        val ids =
            listOf(
                Sentori.error(IllegalStateException("boom")),
                Sentori.warn("checkout.slow"),
                Sentori.trace("cart.opened"),
                Sentori.assert("total.positive", false),
                Sentori.probe("SEN-482"),
            )
        ids.forEach { assertEquals("every verb returns an event id", 36, it.length) }
        assertTrue("and nothing is queued before init", queued().isEmpty())
    }

    @Test
    fun errorCarriesTypeMessageStackAndTheSignalRing() {
        started()
        Sentori.pushSignal("nav", mapOf("to" to "/checkout"))
        Sentori.error(IllegalStateException("boom"), mapOf("cartId" to "c_1"))

        val e = queued().last()
        assertEquals("error", e["kind"])
        assertEquals("android", e["platform"])
        assertEquals("app@1.0.0", e["release"])

        @Suppress("UNCHECKED_CAST") val payload = e["payload"] as Map<String, Any?>
        @Suppress("UNCHECKED_CAST") val err = payload["error"] as Map<String, Any?>
        assertEquals("java.lang.IllegalStateException", err["type"])
        assertEquals("boom", err["message"])
        assertTrue("a stack with no frames symbolicates nothing", (err["stack"] as List<*>).isNotEmpty())

        @Suppress("UNCHECKED_CAST") val data = payload["data"] as Map<String, Any?>
        assertEquals("c_1", data["cartId"])

        val signals = payload["signals"] as List<*>
        assertEquals(1, signals.size)
    }

    @Test
    fun aPassingAssertIsNeverAnEvent() {
        started()
        Sentori.assert("total.positive", true)
        Sentori.assert("total.positive", true)
        assertTrue("passes aggregate; only failures are events", queued().isEmpty())
        assertEquals(2, SentoriTransport.peekAssertStats().first()["passDelta"])

        Sentori.assert("total.positive", false)
        assertEquals(1, queued().size)
        assertEquals("assert", queued().first()["kind"])
    }

    @Test
    fun assertNeverStopsTheProgram() {
        started()
        Sentori.assert("impossible", false)
        Sentori.assert("also impossible", false)
        assertEquals("reached here, twice", 2, queued().size)
    }

    @Test
    fun quietTraceReachesTheRingAndNotTheQueue() {
        started()
        Sentori.trace("tick", quiet = true)
        assertTrue("a quiet trace must stay affordable", queued().isEmpty())
        assertEquals("but it is still context", 1, SentoriSignalRing.snapshot().size)

        Sentori.trace("cart.opened")
        assertEquals(1, queued().size)
        assertEquals("cart.opened", queued().first()["name"])
    }

    @Test
    fun userKeyRidesEventsOnlyAfterUserIsCalled() {
        started()
        Sentori.warn("before")
        assertNull("no identity, no key — not an empty one", queued().last()["userKey"])

        Sentori.user("usr_123", null)
        Sentori.warn("after")
        assertEquals(SentoriIdentity.hash("id", "usr_123"), queued().last()["userKey"])
    }

    @Test
    fun contextMergesAndRidesEveryEvent() {
        started()
        Sentori.context(mapOf("tenant" to "acme"))
        Sentori.context(mapOf("plan" to "pro"))
        Sentori.probe("SEN-1")

        @Suppress("UNCHECKED_CAST")
        val payload = queued().last()["payload"] as Map<String, Any?>
        @Suppress("UNCHECKED_CAST") val ctx = payload["context"] as Map<String, Any?>
        assertEquals("acme", ctx["tenant"])
        assertEquals("pro", ctx["plan"])
    }

    @Test
    fun eventIdsAreUuidV7AndSortByTime() {
        val first = Sentori.newEventId()
        Thread.sleep(10)
        val second = Sentori.newEventId()
        assertEquals(36, first.length)
        assertEquals("version nibble", '7', first[14])
        assertTrue("v7 ids sort by creation time as strings", first < second)
    }

    @Test
    fun theQueueIsBoundedAndDropsOldestFirst() {
        started()
        // Not flushed: nothing drains to a dead port fast enough to
        // race this, so it measures the cap rather than a timer.
        repeat(2000) { Sentori.trace("t$it") }
        assertTrue("an unbounded queue is a leak with a nicer name", queued().size <= 500)
        assertEquals("t1999", queued().last()["name"])
    }

    @Test
    fun garbageInDataDoesNotReachTheCaller() {
        started()
        val id =
            Sentori.warn(
                "bad",
                mapOf("nan" to Double.NaN, "obj" to Any(), "nested" to mapOf("k" to Double.NaN)),
            )
        assertEquals(36, id.length)
        assertNotNull(queued().last())
    }
}
