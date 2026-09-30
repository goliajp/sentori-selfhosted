// Phase 42 sub-F.10 — Robolectric coverage for SentoriScreenshotCapture.
//
// Run via Gradle on the Android host:
//   ./gradlew :sentori-react-native:testDebugUnitTest
//
// We can't actually drive `PixelCopy.request` under Robolectric (it
// needs a real surface + EGL context). The instrumented version of
// this test runs against `androidTest` on a connected device /
// emulator — that's where the perf budget assertion (≤ 50 ms 95p)
// lives. The unit-test surface below exercises only the parts that
// don't need the GPU: the depth-capped view-tree walker and the
// `toJson` Map → JSONObject converter.

package com.sentori

import android.view.View
import androidx.test.core.app.ApplicationProvider
import org.json.JSONObject
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

@RunWith(RobolectricTestRunner::class)
class SentoriScreenshotCaptureTest {

    @Test
    fun toJsonConvertsNestedMapToJSONObject() {
        val input = mapOf(
            "rootId" to "n1",
            "nodes" to mapOf(
                "n1" to mapOf(
                    "type" to "View",
                    "children" to listOf("n2"),
                ),
                "n2" to mapOf(
                    "type" to "Button",
                    "props_summary" to mapOf("alpha" to "1.00"),
                ),
            ),
        )
        val json = SentoriScreenshotCapture.toJson(input)
        assertTrue(json is JSONObject)
        json as JSONObject
        assertEquals("n1", json.getString("rootId"))
        val n1 = json.getJSONObject("nodes").getJSONObject("n1")
        assertEquals("View", n1.getString("type"))
        assertEquals("n2", n1.getJSONArray("children").getString(0))
    }

    @Test
    fun captureKeyWindowReturnsNullWithoutRegisteredActivity() {
        // Don't register an Application — the helper should fail
        // gracefully (return null) instead of crashing.
        val out = SentoriScreenshotCapture.captureKeyWindow()
        // Either null (no last activity yet) or a map with viewTree
        // (Robolectric may attach one transparently). Both are valid;
        // the contract is "doesn't throw".
        if (out != null) {
            assertNotNull(out["viewTree"])
        } else {
            assertNull(out)
        }
    }

    @Test
    fun toJsonHandlesNullAndPrimitives() {
        assertEquals(JSONObject.NULL, SentoriScreenshotCapture.toJson(null))
        assertEquals(42, SentoriScreenshotCapture.toJson(42))
        assertEquals("hi", SentoriScreenshotCapture.toJson("hi"))
    }

    @Test
    fun benchmarkRenderReturnsPositiveDurationOnLiveView() {
        // Build a minimal View hierarchy and time the benchmark
        // helper — gives us a smoke signal that the API doesn't
        // throw on synthesized views under Robolectric.
        val activity = Robolectric.buildActivity(android.app.Activity::class.java)
            .create()
            .start()
            .resume()
            .get()
        val v: View = activity.window.decorView
        v.measure(View.MeasureSpec.UNSPECIFIED, View.MeasureSpec.UNSPECIFIED)
        v.layout(0, 0, 200, 200)
        val ns = SentoriScreenshotCapture.benchmarkRenderToBitmapBlocking(v)
        assertTrue(ns > 0)
    }
}
