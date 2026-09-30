// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/test/java/com/sentori/SentoriExitInfoTest.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
package com.sentori

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

// Robolectric for a real `org.json`: the stub `android.jar` the plain
// JVM tests link against answers null from every method, so an
// assertion about what the transport will serialise would be an
// assertion about the stub.
@RunWith(RobolectricTestRunner::class)
class SentoriExitInfoTest {

    private val anrDump =
        """
        ----- pid 1234 at 2026-09-30 05:00:00 -----
        Cmd line: com.example.app

        "main" prio=5 tid=1 Blocked
          | group="main" sCount=1
          at com.example.cart.Cart.total(Cart.kt:48)
          at com.example.cart.CartScreen.render(CartScreen.kt:12)
          at android.os.Looper.loop(Looper.java:223)
          at android.app.ActivityThread.main(ActivityThread.java:7656)

        "Binder:1234_2" prio=5 tid=9 Native
          at com.example.other.Ignored.method(Ignored.kt:1)
        """
            .trimIndent()

    @Test
    fun `only the main thread's frames are kept`() {
        // The dump holds every thread and runs to hundreds of
        // kilobytes. What opens the case is where main was stuck.
        val frames = SentoriExitInfo.parseTrace(anrDump.byteInputStream())
        assertEquals(4, frames.size)
        assertEquals("com.example.cart.Cart.total", frames[0]["function"])
        assertEquals("Cart.kt", frames[0]["file"])
        assertEquals(48, frames[0]["line"])
        assertTrue(frames.none { (it["function"] as String).contains("Ignored") })
    }

    @Test
    fun `app frames and platform frames are told apart`() {
        val frames = SentoriExitInfo.parseTrace(anrDump.byteInputStream())
        assertEquals(true, frames[0]["inApp"])
        assertEquals(false, frames[2]["inApp"]) // android.os.Looper
    }

    @Test
    fun `a dump with no main thread yields no frames rather than the wrong ones`() {
        val other =
            """
            "Binder:1_1" prio=5 tid=9 Native
              at com.example.other.Ignored.method(Ignored.kt:1)
            """
                .trimIndent()
        assertTrue(SentoriExitInfo.parseTrace(other.byteInputStream()).isEmpty())
    }

    @Test
    fun `an empty dump is empty, not a crash`() {
        assertTrue(SentoriExitInfo.parseTrace("".byteInputStream()).isEmpty())
    }

    @Test
    fun `a record we have already reported is not reported twice`() {
        // One freeze opening two issues is worse than none: the
        // second is indistinguishable from a real recurrence.
        val anr = 6 // ApplicationExitInfo.REASON_ANR
        assertTrue(SentoriExitInfo.isNew(timestamp = 200, reason = anr, seenAt = 100))
        assertFalse(SentoriExitInfo.isNew(timestamp = 100, reason = anr, seenAt = 100))
        assertFalse(SentoriExitInfo.isNew(timestamp = 50, reason = anr, seenAt = 100))
    }

    @Test
    fun `a clean exit is not an incident`() {
        // REASON_USER_REQUESTED — the user swiped the app away, which
        // is the app working.
        assertFalse(SentoriExitInfo.isNew(timestamp = 200, reason = 10, seenAt = 100))
        // REASON_EXIT_SELF
        assertFalse(SentoriExitInfo.isNew(timestamp = 200, reason = 1, seenAt = 100))
    }

    @Test
    fun `the event is one the five-kind wire accepts`() {
        // `kind` has five values and no sixth. An ANR is an error the
        // app could not report itself; what it was is in the type.
        val frames = SentoriExitInfo.parseTrace(anrDump.byteInputStream())
        val event =
            SentoriExitInfo.wireEvent(
                occurredAtMillis = 1_790_000_000_000L,
                reason = 6,
                description = "Input dispatching timed out",
                trace = frames,
            )
        assertEquals("error", event["kind"])
        assertEquals("android", event["platform"])
        assertTrue((event["occurredAt"] as String).endsWith("Z"))
        val payload = event["payload"] as Map<*, *>
        val error = payload["error"] as Map<*, *>
        assertEquals("ApplicationNotResponding", error["type"])
        assertEquals("Input dispatching timed out", error["message"])
        assertEquals(4, (error["stack"] as List<*>).size)
        assertEquals(true, payload["nativeCrash"])
        // Serialises: the transport puts this through org.json, and a
        // value it cannot take would be dropped on the way out.
        assertTrue(JSONObject(event as Map<*, *>).toString().contains("ApplicationNotResponding"))
    }
}
