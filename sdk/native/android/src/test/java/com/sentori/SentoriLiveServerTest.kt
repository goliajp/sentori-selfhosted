package com.sentori

import androidx.test.core.app.ApplicationProvider
import java.io.File
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Against a real Sentori server, not a mock.
 *
 * Everything else in this suite asserts the shape this SDK *builds*.
 * That is worth nothing if the server rejects it, and a mock would
 * agree with whatever mistake the SDK is making — which is how a wire
 * format quietly diverges. The one thing that cannot agree with a
 * mistake is the server.
 *
 * Configuration comes from a file rather than the environment, the
 * same way the iOS suite reads it, and for the same reason: it was
 * written once by whoever brought a server up.
 *
 * `assumeTrue` skips when there is none. A skip in Gradle's report is
 * visible; `scripts/android-live-ingest.sh` still fails if this test
 * did not actually run, because a suite that quietly tested nothing
 * reports exactly like one that passed.
 */
@RunWith(RobolectricTestRunner::class)
class SentoriLiveServerTest {

    private var base: String? = null
    private var token: String? = null

    private fun fixture(): File? {
        var dir = File(System.getProperty("user.dir") ?: ".").absoluteFile
        repeat(8) {
            val f = File(dir, "sdk/native/fixtures/live-server.json")
            if (f.exists()) return f
            dir = dir.parentFile ?: dir
        }
        return null
    }

    @Before
    fun setUp() {
        val f = fixture()
        assumeTrue("no sdk/native/fixtures/live-server.json — run scripts/android-live-ingest.sh", f != null)
        val j = JSONObject(f!!.readText())
        base = j.getString("base")
        token = j.getString("token")
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

    @Test
    fun theServerAcceptsWhatThisSdkSends() {
        Sentori.start(
            SentoriConfig(
                token = token!!,
                ingestUrl = base!!,
                release = "kotlin-e2e@1.0.0",
                environment = "test",
            )
        )
        Sentori.user("usr_kotlin_1", null)
        Sentori.context(mapOf("tenant" to "acme"))
        Sentori.pushSignal("nav", mapOf("to" to "/checkout"))

        val id = Sentori.error(IllegalStateException("boom"), mapOf("cartId" to "c_1"))
        Sentori.warn("checkout.slow", mapOf("ms" to 3200))
        Sentori.assert("total.positive", false)
        Sentori.probe("SEN-482")
        SentoriTransport.flush()

        // Poll for delivery rather than sleeping and checking that
        // nothing has failed *yet*: both "no spill" and "queue empty"
        // are true while retries are in flight, and both are true when
        // the server answers 4xx.
        val deadline = System.currentTimeMillis() + 30_000
        while (SentoriTransport.peekDelivered() == 0 && System.currentTimeMillis() < deadline) {
            Thread.sleep(250)
        }

        assertTrue(
            "the server never accepted a batch — spilled: " +
                "${SentoriTransport.peekPersisted().size}, queued: ${SentoriTransport.peekQueue().size}",
            SentoriTransport.peekDelivered() > 0,
        )
        assertEquals(36, id.length)

        // The multipart body against a real parser.
        //
        // Everything else about attachments is asserted as a string,
        // and a string that looks right is exactly what React Native
        // shipped for a release while every upload failed on the
        // device. The body is hand-built; the only proof it is
        // well-formed is a server that read it.
        val uploaded = java.util.concurrent.CountDownLatch(1)
        var ok = false
        SentoriAttachment.upload(
            eventId = id,
            kind = "screenshot",
            base64 = ONE_PIXEL_JPEG_BASE64,
            mediaType = "image/jpeg",
        ) { result ->
            ok = result
            uploaded.countDown()
        }
        assertTrue(
            "the attachment upload never came back",
            uploaded.await(30, java.util.concurrent.TimeUnit.SECONDS),
        )
        assertTrue(
            "the server refused the multipart body — it parses this, or attachments " +
                "die silently on every device",
            ok,
        )
    }

    companion object {
        /**
         * The smallest valid JPEG, base64. Content matters: the server
         * stores what it decodes, and a payload that is not an image
         * would pass the upload and fail the viewer.
         */
        private const val ONE_PIXEL_JPEG_BASE64 =
            "/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0a" +
                "HBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/wAALCAABAAEBAREA/8QAFAABAAAAAAAA" +
                "AAAAAAAAAAAACf/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AKp//2Q=="
    }

}
