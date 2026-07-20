// Phase 16 sub-E: Robolectric coverage for SentoriCrashHandler (回填 Phase 7).
//
// Run via Gradle on the Android host:
//   ./gradlew :sentori-react-native:testDebugUnitTest
//
// We can't easily catch a real uncaught Throwable in-process (the test
// runner would itself crash), so we drive the persistence helper
// directly and assert it writes a protocol-shaped JSON file.

package com.sentori

import androidx.test.core.app.ApplicationProvider
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.json.JSONObject
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

@RunWith(RobolectricTestRunner::class)
class SentoriCrashHandlerTest {
    private lateinit var pendingDir: File

    @Before
    fun setUp() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        pendingDir = File(ctx.filesDir, "sentori/pending")
        pendingDir.deleteRecursively()
        pendingDir.mkdirs()
        SentoriCrashHandler.installForTesting(ctx)
    }

    @After
    fun tearDown() {
        pendingDir.deleteRecursively()
    }

    @Test
    fun writePendingProducesValidEventJson() {
        val ex = RuntimeException("boom from robolectric")
        SentoriCrashHandler.persistForTesting(ex, "android-test-thread")

        val files = pendingDir.listFiles { f -> f.name.endsWith(".json") }
        assertNotNull(files, "no listing in $pendingDir")
        assertEquals(1, files.size, "expected exactly one pending file")

        val payload = JSONObject(files[0].readText())
        assertEquals("error", payload.getString("kind"))
        assertEquals("android", payload.getString("platform"))
        assertNotNull(payload.getString("id"))
        assertNotNull(payload.getString("timestamp"))
        val error = payload.getJSONObject("error")
        assertEquals("RuntimeException", error.getString("type"))
        assertTrue(error.getString("message").contains("boom"))
    }
}
