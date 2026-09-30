// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/test/java/com/sentori/SentoriMaskTest.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test

class SentoriMaskTest {
    @After
    fun tearDown() = SentoriMask.__resetForTests()

    @Test
    fun `nothing is masked until a host registers a query`() {
        SentoriMask.__resetForTests()
        assertEquals(emptyList<String>(), SentoriMask.ids())
    }

    @Test
    fun `the registered ids come back`() {
        SentoriMask.register { listOf("card-number", "camera-feed") }
        assertEquals(listOf("card-number", "camera-feed"), SentoriMask.ids())
    }

    @Test
    fun `a throwing query masks nothing rather than taking the capture down`() {
        // The capture tick runs on our thread, not the host's, but a
        // throw here would still end the tick and every tick after it
        // — a replay that silently stopped, which looks like a quiet
        // app rather than a broken SDK.
        SentoriMask.register { throw IllegalStateException("host bug") }
        assertEquals(emptyList<String>(), SentoriMask.ids())
        // And the next frame still asks.
        SentoriMask.register { listOf("ok") }
        assertEquals(listOf("ok"), SentoriMask.ids())
    }

    @Test
    fun `registering null clears`() {
        SentoriMask.register { listOf("x") }
        SentoriMask.register(null)
        assertEquals(emptyList<String>(), SentoriMask.ids())
    }
}
