package com.sentori

import androidx.test.core.app.ApplicationProvider
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * Push registration, as far as Robolectric can go.
 *
 * There is no FCM on the classpath here and no Google Play services,
 * so the honest coverage is the shape of the failures and the shape of
 * the request — not a real token. `scripts/android-live-ingest.sh`
 * proves the ingest route against a real server; a device proves the
 * rest, and nothing here pretends otherwise.
 *
 * Deliberately the same assertions as `SentoriPushTests.swift`.
 */
@RunWith(RobolectricTestRunner::class)
class SentoriPushTest {

    private val context get() = ApplicationProvider.getApplicationContext<android.content.Context>()

    @Before
    fun setUp() {
        SentoriConfig.resetForTests()
        SentoriScope.clear()
        SentoriPush.resetForTests()
    }

    @After
    fun tearDown() {
        SentoriConfig.resetForTests()
        SentoriPush.resetForTests()
    }

    private fun registerBlocking(timeoutMs: Long = 1_000): SentoriPush.Result {
        val latch = CountDownLatch(1)
        var result: SentoriPush.Result? = null
        SentoriPush.register(context, activity = null, timeoutMs = timeoutMs) {
            result = it
            latch.countDown()
        }
        assertTrue("register never called back", latch.await(30, TimeUnit.SECONDS))
        return result!!
    }

    @Test
    fun registerBeforeStartReportsItRatherThanThrowing() {
        val r = registerBlocking()
        assertTrue("expected a failure before start, got $r", r is SentoriPush.Result.Failure)
        assertEquals(
            SentoriPush.Failure.NOT_INITIALISED,
            (r as SentoriPush.Result.Failure).reason,
        )
        assertEquals("not-initialised", r.reason.reason)
    }

    @Test
    fun everyFailureHasTheSameNameAsTheOtherTwoSdks() {
        // Same strings as `PushRegisterFailure` in React Native and
        // `SentoriPush.Failure` in Swift, so one set of integration
        // notes covers all three.
        assertEquals("not-initialised", SentoriPush.Failure.NOT_INITIALISED.reason)
        assertEquals("permission-denied", SentoriPush.Failure.PERMISSION_DENIED.reason)
        assertEquals("no-transport", SentoriPush.Failure.NO_TRANSPORT.reason)
        assertEquals("token-timeout", SentoriPush.Failure.TOKEN_TIMEOUT.reason)
        assertEquals("server-rejected", SentoriPush.Failure.SERVER_REJECTED.reason)
    }

    @Test
    fun registerWithoutFcmFailsWithoutThrowing() {
        Sentori.start(
            SentoriConfig(
                token = "st_test",
                ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0",
                environment = "test",
            ),
            context,
        )

        // No Firebase on the classpath, so this ends at permission or
        // at the token wait. Either is fine; reaching the end without
        // a throw and without a hang is the assertion.
        val r = registerBlocking()
        assertTrue("Robolectric cannot actually register, got $r", r is SentoriPush.Result.Failure)
        val reason = (r as SentoriPush.Result.Failure).reason
        assertTrue(
            "unexpected reason ${reason.reason}: ${r.message}",
            reason in
                listOf(
                    SentoriPush.Failure.PERMISSION_DENIED,
                    SentoriPush.Failure.NO_TRANSPORT,
                    SentoriPush.Failure.TOKEN_TIMEOUT,
                ),
        )
        assertNull("a failure must cache nothing", SentoriPush.cachedDeviceHandle(context))
    }

    /**
     * The one that was missing, and the reason a whole class of user
     * never got push.
     *
     * `requestPermission` parked its callback in a field and left it
     * to `handlePermissionResult` — a hook the host has to forward
     * from its own `onRequestPermissionsResult`, which the docs never
     * mentioned and a native host has no reason to know about. So the
     * callback was never called, `finishRegister` never ran, and a
     * first launch registered nothing: the dialog appeared, the user
     * tapped Allow, and the SDK did not continue. It worked on the
     * *next* launch, because the permission was granted by then and
     * the flow never suspended at all.
     *
     * Every test in this file passed throughout, because every one of
     * them passes `activity = null` and skips the prompt entirely.
     *
     * This one takes an Activity, never forwards the result — exactly
     * what a host that has not read the source does — and grants the
     * permission the way the framework does. What is asserted is only
     * that `register` comes back at all. Whether it then succeeds
     * needs an FCM that is not on this classpath.
     */
    @Test
    @Config(sdk = [33])
    fun registerContinuesWhenPermissionIsGrantedWithoutTheHostForwardingIt() {
        SentoriConfig.set(
            SentoriConfig(
                token = "st_test",
                ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0",
                environment = "test",
            ),
        )
        val app = ApplicationProvider.getApplicationContext<android.app.Application>()
        shadowOf(app).denyPermissions(android.Manifest.permission.POST_NOTIFICATIONS)
        val activity = Robolectric.buildActivity(android.app.Activity::class.java).setup().get()

        val latch = CountDownLatch(1)
        var result: SentoriPush.Result? = null
        SentoriPush.permissionTimeoutMs = 20_000
        SentoriPush.register(app, activity = activity, timeoutMs = 500) {
            result = it
            latch.countDown()
        }

        // Not registered yet: the dialog is up and nobody has answered.
        assertNull("register reported before the permission was answered", result)

        // The user taps Allow. Nothing forwards the result — this is
        // the whole point.
        shadowOf(app).grantPermissions(android.Manifest.permission.POST_NOTIFICATIONS)

        assertTrue(
            "register never came back after the permission was granted — the first " +
                "launch after a fresh install registers nothing and says nothing",
            latch.await(30, TimeUnit.SECONDS),
        )
        assertNotNull(result)
    }

    /**
     * And when nobody ever answers, it still reports rather than
     * leaving the caller with a callback that never fires.
     */
    @Test
    @Config(sdk = [33])
    fun registerReportsWhenThePermissionDialogIsIgnored() {
        SentoriConfig.set(
            SentoriConfig(
                token = "st_test",
                ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0",
                environment = "test",
            ),
        )
        val app = ApplicationProvider.getApplicationContext<android.app.Application>()
        shadowOf(app).denyPermissions(android.Manifest.permission.POST_NOTIFICATIONS)
        val activity = Robolectric.buildActivity(android.app.Activity::class.java).setup().get()

        val latch = CountDownLatch(1)
        var result: SentoriPush.Result? = null
        SentoriPush.permissionTimeoutMs = 1_500
        SentoriPush.register(app, activity = activity, timeoutMs = 500) {
            result = it
            latch.countDown()
        }

        assertTrue("register never gave up", latch.await(30, TimeUnit.SECONDS))
        val failure = result as? SentoriPush.Result.Failure
        assertNotNull("expected a failure, got $result", failure)
        assertEquals(SentoriPush.Failure.PERMISSION_DENIED, failure!!.reason)
    }

    @Test
    fun unregisterWithNothingRegisteredIsANoOp() {
        val latch = CountDownLatch(1)
        var ok: Boolean? = null
        SentoriPush.unregister(context) {
            ok = it
            latch.countDown()
        }
        assertTrue(latch.await(10, TimeUnit.SECONDS))
        assertEquals("nothing to revoke", false, ok)
        assertNull(SentoriPush.cachedDeviceHandle(context))
    }

    @Test
    fun theRegistrationBodyIsTheOneTheServerAccepts() {
        // The field names are the whole reason push never worked for a
        // year: the RN SDK sent `provider` where the server reads
        // `kind`, and parsed an `ipt_*` handle no server has ever
        // returned. Pinned here rather than left to a reviewer.
        SentoriScope.setUser("usr_123", null)
        val body =
            mapOf(
                "kind" to "fcm",
                "nativeToken" to "abcd",
                "userKey" to SentoriScope.userKey,
            )
        val json = org.json.JSONObject(SentoriTransport.toJson(body).toString())

        assertEquals("fcm", json.getString("kind"))
        assertTrue("the server has no such field", !json.has("provider"))
        // FCM is one host; an `env` here would be a claim about a
        // sandbox/production split that does not exist.
        assertTrue("FCM has no environment split", !json.has("env"))
        assertEquals(
            SentoriIdentity.hash("id", "usr_123"),
            json.getString("userKey"),
        )
        assertNotNull(json)
    }
    // ── taps ──────────────────────────────────────────────────────

    /**
     * The second half of the same bug as the permission one.
     *
     * `handleNotificationTap` was reachable only from the host, whose
     * own doc comment told it to forward intent extras — advice that
     * appeared nowhere a host would read. So `register(onTap:)` took
     * a callback that could never be called. insight measured it: the
     * tray entry appeared, the app opened, silence.
     *
     * Here the app is launched by a notification and the host does
     * nothing at all.
     */
    @Test
    @Config(sdk = [33])
    fun aColdStartFromANotificationDeliversTheTapWithoutTheHostForwardingIt() {
        SentoriConfig.set(
            SentoriConfig(
                token = "st_test",
                ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0",
                environment = "test",
            ),
        )
        SentoriNotificationTap.resetForTests()
        val app = ApplicationProvider.getApplicationContext<android.app.Application>()
        shadowOf(app).grantPermissions(android.Manifest.permission.POST_NOTIFICATIONS)

        // What the system hands an Activity opened from a tap.
        val intent = android.content.Intent(app, android.app.Activity::class.java)
            .putExtra("google.message_id", "0:1786545260399414%c9712c12")
            .putExtra("title", "Crash in checkout")
        val activity = Robolectric.buildActivity(android.app.Activity::class.java, intent)
            .setup().get()

        val taps = mutableListOf<Map<String, Any?>>()
        val latch = CountDownLatch(1)
        SentoriPush.register(
            app,
            activity = activity,
            timeoutMs = 300,
            onTap = { taps.add(it) },
        ) { latch.countDown() }
        assertTrue("register never came back", latch.await(30, TimeUnit.SECONDS))

        val deadline = System.currentTimeMillis() + 10_000
        while (taps.isEmpty() && System.currentTimeMillis() < deadline) Thread.sleep(50)

        assertTrue(
            "onTap never fired for the notification that launched the app — the host " +
                "passed a callback that could not be called",
            taps.isNotEmpty(),
        )
        assertEquals("0:1786545260399414%c9712c12", taps[0]["google.message_id"])
    }

    /** An ordinary launch is not a tap. */
    @Test
    @Config(sdk = [33])
    fun anOrdinaryLaunchIsNotReportedAsATap() {
        SentoriNotificationTap.resetForTests()
        val activity = Robolectric.buildActivity(android.app.Activity::class.java).setup().get()
        val taps = mutableListOf<Map<String, Any?>>()
        SentoriPushNotifications.drainState()

        SentoriNotificationTap.consume(activity)
        val state = SentoriPushNotifications.drainState()

        @Suppress("UNCHECKED_CAST")
        val drained = state["taps"] as? List<Map<String, Any?>> ?: emptyList()
        taps.addAll(drained)
        assertTrue(
            "a launch with no notification behind it was reported as a tap, which " +
                "puts a notification in every session that never had one",
            taps.isEmpty(),
        )
    }

    /** The same tap is not reported twice. */
    @Test
    @Config(sdk = [33])
    fun oneTapIsDeliveredOnce() {
        SentoriNotificationTap.resetForTests()
        SentoriPushNotifications.drainState()
        val extras = android.os.Bundle().apply {
            putString("google.message_id", "0:dedupe-me")
            putString("title", "once")
        }

        SentoriNotificationTap.consume(extras)
        SentoriNotificationTap.consume(extras)

        @Suppress("UNCHECKED_CAST")
        val taps = SentoriPushNotifications.drainState()["taps"] as? List<Map<String, Any?>>
            ?: emptyList()
        assertEquals(
            "the launch intent is visible to more than one caller; counting it twice " +
                "turns one open into two",
            1,
            taps.size,
        )
    }

    // ── the tray ──────────────────────────────────────────────────

    /**
     * A data message the user can actually see.
     *
     * Nothing posted a notification before, so a `data` message
     * reached `onMessage` and left the tray empty — insight's device
     * said "No notifications" while the callback had fired. The
     * server sends `data` on purpose, so this is the whole visible
     * half of a push.
     */
    @Test
    @Config(sdk = [33])
    fun aDataMessageWithSomethingToShowReachesTheTray() {
        val app = ApplicationProvider.getApplicationContext<android.app.Application>()
        shadowOf(app).grantPermissions(android.Manifest.permission.POST_NOTIFICATIONS)
        val mgr = app.getSystemService(android.content.Context.NOTIFICATION_SERVICE)
            as android.app.NotificationManager
        mgr.cancelAll()

        SentoriPushNotifications.postNotification(
            app,
            mapOf("google.message_id" to "0:show-me", "title" to "Crash in checkout",
                  "body" to "3 users affected"),
        )

        val posted = shadowOf(mgr).allNotifications
        assertEquals("the user saw nothing", 1, posted.size)
        assertNotNull(
            "no pending intent, so the tap cannot come back",
            posted[0].contentIntent,
        )
    }

    /**
     * Through the service, not around it.
     *
     * The first version of the test above called `postNotification`
     * directly, so removing the call from `onMessageReceived` left it
     * green — a unit test of a function nobody calls proves the
     * function, not the feature. This drives a real `RemoteMessage`
     * into the real service.
     */
    @Test
    @Config(sdk = [33])
    fun theServiceTurnsADataMessageIntoATrayEntryAndACallback() {
        val app = ApplicationProvider.getApplicationContext<android.app.Application>()
        shadowOf(app).grantPermissions(android.Manifest.permission.POST_NOTIFICATIONS)
        val mgr = app.getSystemService(android.content.Context.NOTIFICATION_SERVICE)
            as android.app.NotificationManager
        mgr.cancelAll()
        SentoriPushNotifications.drainState()

        val service = Robolectric.buildService(SentoriFirebaseMessagingService::class.java)
            .create().get()
        val message = com.google.firebase.messaging.RemoteMessage.Builder("to@fcm")
            .setMessageId("0:through-the-service")
            .addData("sentori", "1")
            .addData("title", "Crash in checkout")
            .addData("body", "3 users affected")
            .build()

        service.onMessageReceived(message)

        assertEquals(
            "a data message left the tray empty — the user sees nothing and has " +
                "nothing to tap",
            1,
            shadowOf(mgr).allNotifications.size,
        )
        @Suppress("UNCHECKED_CAST")
        val received = SentoriPushNotifications.drainState()["notifications"]
            as? List<Map<String, Any?>> ?: emptyList()
        assertEquals(1, received.size)
    }

    /**
     * A host with no `android:icon` still gets its tray entry.
     *
     * `applicationInfo.icon` is `0` for an app that declares none —
     * insight's drew its logo from a launch theme — and `notify`
     * throws on an icon of 0. That throw went into a catch written
     * for a different reason and disappeared: the channel existed,
     * the callback fired, the tray was empty, and logcat said
     * nothing. They found it by reasoning backwards from the channel.
     *
     * Robolectric accepts an icon of 0 without complaint, so the
     * first version of this test — post one, assert one arrived —
     * passed with the bug still in place and proved nothing. What is
     * asserted instead is the icon this code hands the framework,
     * which is the part that is ours. That a real device rejects a 0
     * is a fact about Android, and the fix is to never hand it one.
     */
    @Test
    @Config(sdk = [33])
    fun aHostWithNoIconStillGetsItsTrayEntry() {
        val app = ApplicationProvider.getApplicationContext<android.app.Application>()
        shadowOf(app).grantPermissions(android.Manifest.permission.POST_NOTIFICATIONS)
        val mgr = app.getSystemService(android.content.Context.NOTIFICATION_SERVICE)
            as android.app.NotificationManager
        mgr.cancelAll()

        val declared = app.applicationInfo.icon
        app.applicationInfo.icon = 0
        try {
            SentoriPushNotifications.postNotification(
                app,
                mapOf("google.message_id" to "0:no-icon", "title" to "Crash in checkout"),
            )
            val posted = shadowOf(mgr).allNotifications
            assertEquals(1, posted.size)
            assertTrue(
                "a small icon of 0 is what a device throws on, and the throw lands " +
                    "in a catch — no tray entry, no log, nothing to go on",
                posted[0].smallIcon.resId != 0,
            )
        } finally {
            app.applicationInfo.icon = declared
        }
    }

    /**
     * A silent data message stays silent. An app that uses data
     * messages to tell itself something would not thank us for
     * turning each one into a notification to dismiss.
     */
    @Test
    @Config(sdk = [33])
    fun aDataMessageWithNothingToShowIsNotPosted() {
        val app = ApplicationProvider.getApplicationContext<android.app.Application>()
        shadowOf(app).grantPermissions(android.Manifest.permission.POST_NOTIFICATIONS)
        val mgr = app.getSystemService(android.content.Context.NOTIFICATION_SERVICE)
            as android.app.NotificationManager
        mgr.cancelAll()

        SentoriPushNotifications.postNotification(app, mapOf("sync" to "inbox"))

        assertTrue(shadowOf(mgr).allNotifications.isEmpty())
    }

    // ── rotation ──────────────────────────────────────────────────

    /**
     * A listener that records what was posted to it.
     *
     * `com.sun.net.httpserver` is not on the Android compile
     * classpath, and a mocked HTTP client would only prove that the
     * SDK called the thing the test replaced. A socket is what the
     * SDK will actually write to.
     */
    private class Recorder : AutoCloseable {
        val bodies = java.util.Collections.synchronizedList(mutableListOf<String>())
        private val socket = java.net.ServerSocket(0, 4, java.net.InetAddress.getLoopbackAddress())
        val port: Int get() = socket.localPort
        private val thread = Thread {
            while (!socket.isClosed) {
                try {
                    socket.accept().use { c ->
                        val input = c.getInputStream()
                        val head = StringBuilder()
                        while (!head.endsWith("\r\n\r\n")) {
                            val b = input.read()
                            if (b < 0) return@use
                            head.append(b.toChar())
                        }
                        val len = Regex("(?i)content-length: *(\\d+)")
                            .find(head)?.groupValues?.get(1)?.toIntOrNull() ?: 0
                        val body = ByteArray(len)
                        var read = 0
                        while (read < len) {
                            val n = input.read(body, read, len - read)
                            if (n < 0) break
                            read += n
                        }
                        bodies.add(String(body, 0, read, Charsets.UTF_8))
                        val payload =
                            """{"spToken":"019ff000-0000-7000-8000-000000000001","isNew":false}"""
                        c.getOutputStream().write(
                            ("HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\n" +
                                "Content-Length: ${payload.toByteArray().size}\r\n" +
                                "Connection: close\r\n\r\n" + payload).toByteArray()
                        )
                        c.getOutputStream().flush()
                    }
                } catch (_: Throwable) {
                    // the close below is what ends this loop
                }
            }
        }.apply { isDaemon = true; start() }

        override fun close() {
            socket.close()
            thread.interrupt()
        }
    }

    private fun configureAt(port: Int) {
        SentoriConfig.set(
            SentoriConfig(
                token = "st_test",
                ingestUrl = "http://127.0.0.1:$port",
                release = "app@1.0.0",
                environment = "test",
            ),
        )
    }

    /**
     * A vendor rotating its token has to reach the server, and the
     * only thing that proves it is a request arriving.
     *
     * The first version of this asserted that the spToken had not
     * changed — which is exactly what happens when the SDK does
     * nothing at all, so it passed against the code it was written to
     * catch. `onNewToken` used to write the value into a field and
     * send it to nobody; a test that cannot tell that apart from
     * working is not a test.
     */
    @Test
    @Config(sdk = [33])
    fun aRotatedTokenIsReportedToTheServer() {
        Recorder().use { rec ->
            val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
            configureAt(rec.port)

            // A device that has registered, which is the only kind a
            // rotation should act on.
            val first = SentoriPush.registerNativeTokenForTests(ctx, "token-before")
            assertNotNull("the first registration did not return an spToken", first)

            // The vendor rotates. This is what `onNewToken` calls.
            SentoriPush.handleRotatedToken(ctx, "token-after")

            val deadline = System.currentTimeMillis() + 15_000
            while (rec.bodies.size < 2 && System.currentTimeMillis() < deadline) Thread.sleep(50)

            assertEquals(
                "the rotation reached nobody — the server keeps the dead token until " +
                    "the host next calls register, and the device receives nothing",
                2,
                rec.bodies.size,
            )
            assertTrue(
                "the rotation did not carry the new token: ${rec.bodies[1]}",
                rec.bodies[1].contains("token-after"),
            )
            // Same installation both times, which is what keeps the
            // address still rather than writing a second row.
            val install = Regex("\"installId\":\"([^\"]+)\"")
            val a = install.find(rec.bodies[0])?.groupValues?.get(1)
            val b = install.find(rec.bodies[1])?.groupValues?.get(1)
            assertNotNull("the registration carried no installId: ${rec.bodies[0]}", a)
            assertEquals("the rotation reported a different installation", a, b)
        }
    }

    /** A device the host never registered is not registered behind its back. */
    @Test
    @Config(sdk = [33])
    fun aRotationForAnUnregisteredDeviceIsIgnored() {
        Recorder().use { rec ->
            val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
            ctx.getSharedPreferences("com.sentori.push", android.content.Context.MODE_PRIVATE)
                .edit().clear().apply()
            SentoriPush.resetForTests()
            configureAt(rec.port)

            SentoriPush.handleRotatedToken(ctx, "token-nobody-asked-for")
            Thread.sleep(1_000)

            assertTrue(
                "a device that never registered was registered by a vendor callback",
                rec.bodies.isEmpty(),
            )
        }
    }


    /**
     * A device registers at launch; the person signs in ten seconds
     * later. Nothing updated the row, so it carried no user for the
     * life of the install — and a send aimed at that person reached
     * nobody and reported success.
     */
    @Test
    fun signingInAfterRegisteringUpdatesTheDevice() {
        Recorder().use { rec ->
            val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
            configureAt(rec.port)

            assertNotNull(SentoriPush.registerNativeTokenForTests(ctx, "token-1"))
            assertEquals(1, rec.bodies.size)
            assertTrue(
                "the first registration already carried a user: ${rec.bodies[0]}",
                !rec.bodies[0].contains("userKey"),
            )

            Sentori.user("usr_123", null, mapOf("plan" to "pro"))

            val deadline = System.currentTimeMillis() + 15_000
            while (rec.bodies.size < 2 && System.currentTimeMillis() < deadline) Thread.sleep(50)

            // Both halves matter: a count alone passes when the update
            // carries nothing, and a key alone passes when no update
            // happened and this is still the first request.
            assertEquals(
                "signing in reached nobody — the device row keeps no user, so a send " +
                    "aimed at that person matches no device and reports success",
                2,
                rec.bodies.size,
            )
            assertTrue(
                "the update carried no identity: ${rec.bodies[1]}",
                rec.bodies[1].contains("\"userKey\""),
            )
            assertTrue(
                "the update carried no traits: ${rec.bodies[1]}",
                rec.bodies[1].contains("\"plan\""),
            )
        }
    }

    /**
     * `Sentori.user` is a verb an app may call on every screen. One
     * request per call is not free to a host, and the iron rule is
     * that this SDK is.
     */
    @Test
    fun settingTheSamePersonAgainSendsNothing() {
        Recorder().use { rec ->
            val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
            configureAt(rec.port)
            assertNotNull(SentoriPush.registerNativeTokenForTests(ctx, "token-1"))

            Sentori.user("usr_123", null, mapOf("plan" to "pro"))
            val deadline = System.currentTimeMillis() + 15_000
            while (rec.bodies.size < 2 && System.currentTimeMillis() < deadline) Thread.sleep(50)
            val after = rec.bodies.size

            Sentori.user("usr_123", null, mapOf("plan" to "pro"))
            Thread.sleep(500)
            assertEquals("the same person was sent twice", after, rec.bodies.size)
        }
    }

    /** A device that never registered is not registered by a sign-in. */
    @Test
    fun signingInOnADeviceThatNeverRegisteredSendsNothing() {
        Recorder().use { rec ->
            configureAt(rec.port)
            Sentori.user("usr_123", null, mapOf("plan" to "pro"))
            Thread.sleep(500)
            assertEquals(0, rec.bodies.size)
        }
    }

    /**
     * Push is allowed to fail silently. It is never allowed to make
     * the host app fail.
     *
     * The host's own handlers are the sharp edge: they are its code
     * and they run inside our loop. Worse here than anywhere — the
     * drain is a `scheduleWithFixedDelay`, and an exception out of a
     * scheduled task makes the executor **cancel every future run**,
     * silently. One throwing `onMessage` used to end push delivery for
     * the life of the process, with nothing anywhere to say so.
     */
    @Test
    fun aThrowingHandlerDoesNotStopDelivery() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        SentoriConfig.set(
            SentoriConfig(token = "st_test", ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0", environment = "test"),
        )
        val seen = mutableListOf<String>()
        val latch = CountDownLatch(1)
        SentoriPush.register(
            ctx,
            activity = null,
            timeoutMs = 200,
            onMessage = {
                seen.add(it["title"] as? String ?: "?")
                if (it["title"] == "one") error("the host threw")
            },
            onTap = { seen.add("tap") },
        ) { latch.countDown() }
        assertTrue(latch.await(30, TimeUnit.SECONDS))

        // One bad handler must not swallow the rest of the batch.
        SentoriPush.drainForTests(
            mapOf(
                "notifications" to listOf(
                    mapOf("title" to "one"),
                    mapOf("title" to "two"),
                ),
                "taps" to listOf(mapOf<String, Any?>("tapped" to true)),
            ),
        )
        assertEquals(listOf("one", "two", "tap"), seen)

        // And the next batch still arrives, which is what would have
        // been lost when the executor cancelled the task.
        SentoriPush.drainForTests(
            mapOf("notifications" to listOf(mapOf("title" to "three")), "taps" to null),
        )
        assertEquals(listOf("one", "two", "tap", "three"), seen)
    }

    /** A host whose completion throws must not take the worker with it. */
    @Test
    fun aThrowingCompletionIsContained() {
        val ctx = ApplicationProvider.getApplicationContext<android.content.Context>()
        SentoriConfig.set(
            SentoriConfig(token = "st_test", ingestUrl = "http://127.0.0.1:9",
                release = "app@1.0.0", environment = "test"),
        )
        val second = CountDownLatch(1)
        SentoriPush.register(ctx, activity = null, timeoutMs = 200) { error("the host threw") }
        Thread.sleep(300)
        // The worker is a single thread. If the first completion took
        // it down, nothing after it ever runs.
        SentoriPush.register(ctx, activity = null, timeoutMs = 200) { second.countDown() }
        assertTrue("the worker did not survive a throwing completion", second.await(30, TimeUnit.SECONDS))
    }
}
