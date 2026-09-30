package com.sentori

import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * This transport and the TypeScript kernel must count the same way.
 *
 * `droppedEvents` is the one number that tells a reader "you are not
 * seeing everything". If it means one thing on Android and another in
 * the browser, it stops being readable at all — and there is no way to
 * notice, because a number that is too low looks exactly like a quiet
 * period.
 *
 * The vectors come from `sdk/core/src/transport.ts` itself, so this
 * asserts against what the other implementations do rather than what
 * this file's author believed they do. `identity-vectors` exists for
 * the same reason and caught a real divergence in two languages.
 */
@RunWith(RobolectricTestRunner::class)
class SentoriTransportVectorsTest {

    private fun fixture(): JSONObject {
        var dir = File(System.getProperty("user.dir") ?: ".").absoluteFile
        repeat(8) {
            val candidate = File(dir, "sdk/native/fixtures/transport-vectors.json")
            if (candidate.exists()) return JSONObject(candidate.readText())
            dir = dir.parentFile ?: dir
        }
        throw IllegalStateException(
            "transport-vectors.json not found above ${System.getProperty("user.dir")}",
        )
    }

    @Before
    fun reset() {
        SentoriTransport.resetForTests()
    }

    @Test
    fun countsTheSameWayTheKernelDoes() {
        val vectors = fixture().getJSONArray("vectors")
        // A fixture that read as empty would pass every assertion below
        // without running one.
        assertTrue("fixture looks truncated: ${vectors.length()}", vectors.length() >= 5)

        var ran = 0
        for (i in 0 until vectors.length()) {
            val v = vectors.getJSONObject(i)
            val applies = v.getJSONArray("applies")
            if ((0 until applies.length()).none { applies.getString(it) == "kotlin" }) continue
            ran += 1

            SentoriTransport.resetForTests()
            val ops = v.getJSONArray("ops")
            var counter = 0
            for (j in 0 until ops.length()) {
                val op = ops.getJSONObject(j)
                when (op.getString("op")) {
                    "enqueue" ->
                        repeat(op.getInt("n")) {
                            SentoriTransport.enqueue(mapOf("id" to "e${counter++}", "kind" to "error"))
                        }
                    "assert" ->
                        SentoriTransport.countAssert(
                            op.getString("name"),
                            op.getBoolean("ok"),
                            op.optString("release", "vectors@1.0.0"),
                        )
                    // `session` never appears with `kotlin` in
                    // `applies`; reaching it means the fixture grew a
                    // case this transport has no answer for, and
                    // silently ignoring it is how a gate stops
                    // covering the thing it names.
                    else -> throw IllegalStateException("unknown op ${op.getString("op")}")
                }
            }

            val expect = v.getJSONObject("expect")
            val name = v.getString("name")
            assertEquals("$name — queued", expect.getInt("queued"), SentoriTransport.peekQueue().size)
            assertEquals("$name — dropped", expect.getInt("dropped"), SentoriTransport.peekDropped())

            val wantStats = expect.getJSONArray("assertStats")
            val got = SentoriTransport.peekAssertStats().sortedBy {
                "${it["name"]}${it["release"]}"
            }
            assertEquals("$name — assert stat rows", wantStats.length(), got.size)
            for (k in 0 until wantStats.length()) {
                val want = wantStats.getJSONObject(k)
                assertEquals("$name — stat $k name", want.getString("name"), got[k]["name"])
                assertEquals("$name — stat $k release", want.getString("release"), got[k]["release"])
                assertEquals(
                    "$name — stat $k passDelta",
                    want.getInt("passDelta"),
                    (got[k]["passDelta"] as Number).toInt(),
                )
                assertEquals(
                    "$name — stat $k failDelta",
                    want.getInt("failDelta"),
                    ((got[k]["failDelta"] as? Number)?.toInt() ?: 0),
                )
            }
        }

        // The fixture says which implementations each case binds. If
        // none bound this one, the loop above asserted nothing and
        // this test was decoration.
        assertTrue("no vector applied to kotlin — this test ran nothing", ran >= 4)
    }
}
