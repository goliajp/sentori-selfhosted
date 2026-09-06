package com.sentori

import androidx.test.core.app.ApplicationProvider
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The crash that killed the app has to survive the app.
 *
 * [SentoriCrashHandler] wrote one JSON file per crash and nothing in
 * this package read them: the directory filled up and no crash was
 * ever sent.
 *
 * Deliberately the same assertions as `SentoriPendingCrashTests.swift`
 * — a crash must arrive identical whichever SDK sent it.
 */
@RunWith(RobolectricTestRunner::class)
class SentoriPendingCrashTest {

    private val context get() = ApplicationProvider.getApplicationContext<android.content.Context>()

    private val onDisk = JSONObject(
        """
        {
          "id": "019ff080-2aeb-7e30-aba1-4431b296d120",
          "timestamp": "2026-08-11T09:00:00.000Z",
          "kind": "error",
          "platform": "android",
          "release": "com.example@1.4.0+220",
          "environment": "production",
          "device": { "os": "android", "osVersion": "15", "model": "Google Pixel 9" },
          "app": { "version": "1.4.0" },
          "error": {
            "type": "java.lang.IllegalStateException",
            "message": "boom",
            "stack": [
              { "function": "com.example.Foo.bar", "file": "Foo.kt", "line": 42, "inApp": true },
              { "function": "main" }
            ]
          }
        }
        """.trimIndent()
    )

    @Before
    fun setUp() {
        SentoriTransport.resetForTests()
        SentoriConfig.resetForTests()
        // The pending directory is one place on disk shared by every
        // test in the process, and `SentoriCrashHandlerTest` writes
        // there too. Without this one test drains another's crash and
        // asserts on the wrong exception.
        SentoriCrashHandler.register(context)
        SentoriCrashHandler.consumePending()
    }

    @After
    fun tearDown() {
        SentoriTransport.resetForTests()
        SentoriConfig.resetForTests()
    }

    @Test
    fun everyFieldSurvivesTheConversion() {
        val wire = SentoriPendingCrash.toWire(onDisk)

        assertEquals("019ff080-2aeb-7e30-aba1-4431b296d120", wire["id"])
        assertEquals("error", wire["kind"])
        assertEquals("android", wire["platform"])
        assertEquals("com.example@1.4.0+220", wire["release"])
        assertEquals("production", wire["environment"])

        // The file says `timestamp`, the wire says `occurredAt`, and
        // the value is the moment the app died — not the moment the
        // next launch noticed. Losing it puts every crash at the time
        // of the following start-up.
        assertEquals("2026-08-11T09:00:00.000Z", wire["occurredAt"])

        @Suppress("UNCHECKED_CAST") val payload = wire["payload"] as Map<String, Any?>
        @Suppress("UNCHECKED_CAST") val error = payload["error"] as Map<String, Any?>
        assertEquals("java.lang.IllegalStateException", error["type"])
        assertEquals("boom", error["message"])

        @Suppress("UNCHECKED_CAST") val stack = error["stack"] as List<Map<String, Any?>>
        assertEquals(2, stack.size)
        assertEquals("com.example.Foo.bar", stack[0]["function"])
        assertEquals(42, stack[0]["line"])
        assertEquals(true, stack[0]["inApp"])
        // A frame with only a function keeps only a function rather
        // than gaining nulls the server would have to ignore.
        assertEquals(1, stack[1].size)

        @Suppress("UNCHECKED_CAST") val device = payload["device"] as Map<String, Any?>
        assertEquals("Google Pixel 9", device["model"])
        assertEquals(true, payload["nativeCrash"])
    }

    @Test
    fun aMissingErrorStillProducesAReportableEvent() {
        // A crash file written mid-teardown can be missing anything.
        // Dropping the event would lose the only record that the app
        // died at all.
        val wire = SentoriPendingCrash.toWire(JSONObject("""{"platform":"android"}"""))
        @Suppress("UNCHECKED_CAST") val payload = wire["payload"] as Map<String, Any?>
        @Suppress("UNCHECKED_CAST") val error = payload["error"] as Map<String, Any?>

        assertEquals("NativeCrash", error["type"])
        assertEquals("", error["message"])
        assertEquals(0, (error["stack"] as List<*>).size)
        assertEquals(36, (wire["id"] as String).length)
        assertNotNull(wire["occurredAt"])
    }

    @Test
    fun iosFilesKeepTheirPlatform() {
        // The same converter runs on both, and a crash filed as
        // `android` would symbolicate against the wrong artifact.
        assertEquals(
            "ios",
            SentoriPendingCrash.toWire(JSONObject("""{"platform":"ios"}"""))["platform"],
        )
        // Anything unrecognised is this platform rather than a value
        // the server has no column for.
        assertEquals(
            "android",
            SentoriPendingCrash.toWire(JSONObject("""{"platform":"haiku"}"""))["platform"],
        )
    }

    @Test
    fun theConvertedEventIsEncodable() {
        // It goes through `org.json` on the way out; a value that
        // cannot encode would lose the batch it rides in.
        val json = SentoriTransport.toJson(SentoriPendingCrash.toWire(onDisk)).toString()
        assertTrue(json.contains("nativeCrash"))
        assertTrue(json.contains("com.example.Foo.bar"))
    }

    @Test
    fun startIsWhatDrainsThem() {
        // Through `Sentori.start`, not `ship` directly.
        //
        // The iOS suite's first version called `ship()` itself, so
        // deleting the call from `start` left every test green — the
        // same shape as the bug being fixed, one level up. A unit test
        // of a function nobody calls proves the function, not the
        // feature.
        // `persistForTesting` writes the file the way the handler
        // does. Invoking the installed handler directly recurses
        // through it into itself and dies with a StackOverflowError —
        // which is a fact about the test, not about the SDK.
        SentoriCrashHandler.installForTesting(context)
        SentoriCrashHandler.persistForTesting(IllegalStateException("StartupDrain"))

        Sentori.start(
            SentoriConfig(
                token = "st_test",
                ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0",
                environment = "test",
            ),
            context,
        )

        val deadline = System.currentTimeMillis() + 10_000
        var found = false
        while (System.currentTimeMillis() < deadline && !found) {
            val all = SentoriTransport.peekQueue() + SentoriTransport.peekPersisted()
            found =
                all.any { ev ->
                    @Suppress("UNCHECKED_CAST")
                    val p = ev["payload"] as? Map<String, Any?>
                    @Suppress("UNCHECKED_CAST")
                    val e = p?.get("error") as? Map<String, Any?>
                    (e?.get("message") as? String)?.contains("StartupDrain") == true
                }
            if (!found) Thread.sleep(100)
        }
        assertTrue(
            "start did not drain the pending crash — the handler writes files nobody reads",
            found,
        )
    }
}
