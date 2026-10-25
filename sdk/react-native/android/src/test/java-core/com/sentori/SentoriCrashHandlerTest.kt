// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/android/src/test/java/com/sentori/SentoriCrashHandlerTest.kt
// Run `node scripts/sync-native-core.mjs` after editing it.
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
import java.net.InetAddress
import java.net.ServerSocket
import java.io.DataInputStream
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

@RunWith(RobolectricTestRunner::class)
class SentoriCrashHandlerTest {
    private lateinit var pendingDir: File
    private var originalHandler: Thread.UncaughtExceptionHandler? = null

    @Before
    fun setUp() {
        originalHandler = Thread.getDefaultUncaughtExceptionHandler()
        SentoriCrashHandler.resetForTests()
        SentoriConfig.resetForTests()
        SentoriTransport.resetForTests()
        SentoriScope.clear()
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        pendingDir = File(ctx.filesDir, "sentori/pending")
        pendingDir.deleteRecursively()
        pendingDir.mkdirs()
        SentoriCrashHandler.setConfig(mapOf("release" to "cold-start@1", "environment" to "test"))
        SentoriCrashHandler.installForTesting(ctx)
    }

    @After
    fun tearDown() {
        awaitStartup()
        SentoriCrashHandler.resetForTests()
        Thread.setDefaultUncaughtExceptionHandler(originalHandler)
        SentoriConfig.resetForTests()
        SentoriTransport.resetForTests()
        SentoriScope.clear()
        pendingDir.deleteRecursively()
    }

    private fun awaitStartup() {
        val done = CountDownLatch(1)
        SentoriTransport.background { done.countDown() }
        assertTrue(done.await(10, TimeUnit.SECONDS), "startup worker did not finish")
    }

    private fun start(release: String, environment: String, url: String = "http://127.0.0.1:9") {
        Sentori.start(
            SentoriConfig("st_test", url, release, environment),
            ApplicationProvider.getApplicationContext<android.content.Context>(),
        )
        awaitStartup()
    }

    @Test
    fun firstStartAndReconfigurationUseCurrentIdentityAndDelegateOnce() {
        // No previously installed Context: exercise the order in start itself.
        SentoriCrashHandler.resetForTests()
        var delegated = 0
        Thread.setDefaultUncaughtExceptionHandler { _, _ -> delegated++ }
        start("insight-native@1.2+3", "production")
        val handler = Thread.getDefaultUncaughtExceptionHandler()!!
        Sentori.context(mapOf("app" to "insight-native"))
        handler.uncaughtException(Thread.currentThread(), IllegalStateException("first launch"))
        val first = JSONObject(SentoriCrashHandler.consumePending().single())
        assertEquals("insight-native@1.2+3", first.getString("release"))
        assertEquals("production", first.getString("environment"))
        assertEquals("insight-native", first.getJSONObject("context").getString("app"))
        assertEquals(1, delegated)

        start("insight-native@1.3+4", "staging")
        assertTrue(handler === Thread.getDefaultUncaughtExceptionHandler())
        handler.uncaughtException(Thread.currentThread(), IllegalStateException("reconfigured"))
        val second = JSONObject(SentoriCrashHandler.consumePending().single())
        assertEquals("insight-native@1.3+4", second.getString("release"))
        assertEquals("staging", second.getString("environment"))
        assertEquals(2, delegated)
    }

    @Test
    fun nextLaunchShipsCrashTimeIdentityAndContext() {
        Sentori.context(mapOf("app" to "insight-native", "route" to "checkout"))
        SentoriCrashHandler.persistForTesting(IllegalStateException("previous launch"))
        // Model a fresh process with a new release and different ambient context.
        SentoriCrashHandler.resetForTests()
        SentoriScope.clear()
        Sentori.context(mapOf("app" to "new-app", "route" to "home"))
        // Assert the actual HTTP body, not a queue that can be empty while its
        // asynchronous flush is in flight.
        CrashReceiver().use { receiver ->
            start("next-release@2", "staging", "http://127.0.0.1:${receiver.port}")
            val body = receiver.body.get(10, TimeUnit.SECONDS)
            val events = JSONObject(body).getJSONArray("events")
            assertEquals(1, events.length())
            val wire = events.getJSONObject(0)
            assertEquals("cold-start@1", wire.getString("release"))
            assertEquals("test", wire.getString("environment"))
            val payload = wire.getJSONObject("payload")
            assertEquals("previous launch", payload.getJSONObject("error").getString("message"))
            assertEquals("insight-native", payload.getJSONObject("context").getString("app"))
            assertEquals("checkout", payload.getJSONObject("context").getString("route"))
            assertTrue(SentoriCrashHandler.consumePending().isEmpty())
        }
    }

    // Android's compile classpath lacks com.sun.net.httpserver. A loopback
    // socket still exercises the SDK's real HTTP serialization and delivery.
    private class CrashReceiver : AutoCloseable {
        private val socket = ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"))
        val port: Int get() = socket.localPort
        val body = CompletableFuture<String>()
        private val thread = Thread {
            try {
                socket.accept().use { connection ->
                    connection.soTimeout = 10_000
                    val input = DataInputStream(connection.getInputStream())
                    val header = StringBuilder()
                    while (!header.endsWith("\r\n\r\n")) header.append(input.readUnsignedByte().toChar())
                    require(header.startsWith("POST /v1/events:batch "))
                    val length = Regex("(?i)content-length: *(\\d+)").find(header)!!.groupValues[1].toInt()
                    val bytes = ByteArray(length)
                    input.readFully(bytes)
                    val response = "{\"accepted\":1,\"outcomes\":[]}"
                    connection.getOutputStream().apply {
                        write(("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n" +
                            "Content-Length: ${response.length}\r\nConnection: close\r\n\r\n" + response).toByteArray())
                        flush()
                    }
                    body.complete(bytes.toString(Charsets.UTF_8))
                }
            } catch (error: Throwable) {
                body.completeExceptionally(error)
            }
        }.apply { isDaemon = true; start() }

        override fun close() {
            socket.close()
            thread.join(1000)
        }
    }

    @Test
    fun writePendingProducesValidEventJson() {
        val ex = RuntimeException("boom from robolectric")
        SentoriCrashHandler.persistForTesting(ex)

        val files = pendingDir.listFiles { f -> f.name.endsWith(".json") }
        assertNotNull(files, "no listing in $pendingDir")
        assertEquals(1, files.size, "expected exactly one pending file")

        val payload = JSONObject(files[0].readText())
        assertEquals("error", payload.getString("kind"))
        assertEquals("android", payload.getString("platform"))
        assertEquals("cold-start@1", payload.getString("release"))
        assertEquals("test", payload.getString("environment"))
        assertNotNull(payload.getString("id"))
        assertNotNull(payload.getString("timestamp"))
        val error = payload.getJSONObject("error")
        // Fully qualified, not the simple name: on Android the package
        // is real signal — two `FooException` classes from different
        // libraries are different bugs — and it is what every shipped
        // Android event already carries, so shortening it here would
        // split existing issue groups in two.
        assertEquals("java.lang.RuntimeException", error.getString("type"))
        assertTrue(error.getString("message").contains("boom"))
    }
}
