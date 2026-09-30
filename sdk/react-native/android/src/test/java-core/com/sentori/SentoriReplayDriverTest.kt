// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/test/java/com/sentori/SentoriReplayDriverTest.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class SentoriReplayDriverTest {

    @After
    fun tearDown() = SentoriReplayDriver.__resetForTests()

    @Test
    fun `replay is off until a host asks for it`() {
        // The most expensive thing this SDK can do. A customer turns
        // it on deliberately or not at all.
        SentoriReplayDriver.__resetForTests()
        assertFalse(SentoriReplayDriver.__isRunningForTests())
        assertEquals("", SentoriReplayDriver.drain())
    }

    @Test
    fun `starting twice does not run two timers`() {
        SentoriReplayDriver.start(hz = 1.0)
        SentoriReplayDriver.start(hz = 1.0)
        assertTrue(SentoriReplayDriver.__isRunningForTests())
        SentoriReplayDriver.stop()
        assertFalse(SentoriReplayDriver.__isRunningForTests())
    }

    @Test
    fun `drain hands back newline-delimited json and starts cold`() {
        // The player reads one entry per line. Draining also resets,
        // so the next frame is a keyframe — resuming with a delta
        // would reconstruct against a state the player no longer has.
        SentoriReplayDriver.__pushForTests(
            SentoriReplay.Frame(
                ts = 1000.0, width = 320.0, height = 640.0,
                nodes = listOf(SentoriReplay.Node(0.0, 0.0, 10.0, 10.0, kind = "a")),
            )
        )
        SentoriReplayDriver.__pushForTests(
            SentoriReplay.Frame(
                ts = 1500.0, width = 320.0, height = 640.0,
                nodes = listOf(SentoriReplay.Node(0.0, 0.0, 10.0, 10.0, kind = "b")),
            )
        )
        val out = SentoriReplayDriver.drain()
        val lines = out.split("\n")
        assertEquals(2, lines.size)
        assertTrue(lines[0].contains("\"kind\":\"key\""))
        assertTrue(lines[1].contains("\"kind\":\"delta\""))
        assertEquals("", SentoriReplayDriver.drain())
    }
}
