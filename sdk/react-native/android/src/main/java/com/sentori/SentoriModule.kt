package com.sentori

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/**
 * Expo Module exposing the Android crash handler to JS. Same JS contract
 * as the iOS module:
 *   - setConfig({ token, release, environment })
 *   - drainPending() -> List<String>  (JSON bodies)
 *   - startAnrWatchdog({ timeoutMs?, intervalMs?, force? })
 *   - stopAnrWatchdog()
 */
class SentoriModule : Module() {
    override fun definition() = ModuleDefinition {
        Name("Sentori")

        OnCreate {
            val ctx = appContext.reactContext ?: return@OnCreate
            SentoriCrashHandler.register(ctx)
            // v0.9.4 #1 — start frame watch. Cold-start is captured
            // anchored to Process.getStartElapsedRealtime so no
            // separate registerColdStartAnchor() call is needed.
            SentoriMobileVitals.startFrameWatch()
        }

        // v0.9.5 #8 — TurboModule exception bridge readout.
        Function("getRecentNativeException") {
            SentoriNativeExceptionBridge.getRecent()
        }

        // v0.9.6 #2 — wireframe session replay capture.
        Function("captureWireframe") { maskedIds: List<String> ->
            SentoriReplayCapture.captureWireframe(maskedIds)
        }

        // v0.9.12 — diagnostic readout for replay. See iOS side.
        Function("probeWireframe") {
            SentoriReplayCapture.probe()
        }

        // v1.0.0-rc.2 — diagnostic readout for screenshot. Surfaces
        // foreground-Activity provenance, decor-view state, and the
        // last failure reason so the JS side can ship raw state back
        // when screenshot returns null.
        Function("probeScreenshot") {
            SentoriScreenshotCapture.probe()
        }

        // v0.9.4 #1 — Mobile Vitals exposure.
        Function("markJsBridgeReady") {
            SentoriMobileVitals.markJsBridgeReady()
        }
        Function("getColdStartMs") {
            SentoriMobileVitals.getColdStartMs()
        }
        Function("getFrameCounters") {
            SentoriMobileVitals.getFrameCounters()
        }
        Function("resetFrameCounters") {
            SentoriMobileVitals.resetFrameCounters()
        }

        Function("setConfig") { config: Map<String, Any?> ->
            SentoriCrashHandler.setConfig(config)
        }

        AsyncFunction("drainPending") {
            SentoriCrashHandler.consumePending()
        }

        // v0.7.3 — JS-triggered screenshot path with consumer-supplied
        // mask IDs. JS owns the registry of `nativeID`s to redact;
        // native walks the view tree and paints black rectangles in
        // the rendered bitmap. Returns `null` when no activity / API
        // < 24 / capture timed out. Replaces the previous
        // `react-native-view-shot` peer-dep path.
        AsyncFunction("captureScreenshotWithMask") { maskedIds: List<String> ->
            SentoriScreenshotCapture.captureScreenshotWithMask(maskedIds)
        }

        // Watchdog is opt-in from JS so the host app picks the
        // trade-off — stricter detection vs noise from the Metro
        // debugger pausing the main thread. Pass `force: true` to
        // run in debug builds.
        Function("startAnrWatchdog") { options: Map<String, Any?>? ->
            appContext.reactContext?.let { ctx ->
                val timeoutMs = (options?.get("timeoutMs") as? Number)?.toLong() ?: 5_000L
                val intervalMs = (options?.get("intervalMs") as? Number)?.toLong() ?: 1_000L
                val force = (options?.get("force") as? Boolean) ?: false
                SentoriAnrWatchdog.start(ctx, timeoutMs, intervalMs, force)
            }
        }

        Function("stopAnrWatchdog") {
            SentoriAnrWatchdog.stop()
        }

        // Dev-only helper — schedules an uncaught RuntimeException after
        // a tick so the JS bridge has time to return; the crash is then
        // captured by SentoriCrashHandler and written to
        // <filesDir>/sentori/pending/.
        Function("triggerTestNativeCrash") {
            android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                throw RuntimeException("Sentori test native crash")
            }, 50)
        }

        // v2.10 — push notification bridge.
        //
        // Same surface as iOS so the JS layer (sdk/react-native/src/push.ts)
        // doesn't branch on Platform.OS. The Android flow uses
        // FirebaseMessaging behind a runtime `Class.forName` gate so
        // non-push hosts pay nothing.

        AsyncFunction("pushGetStatus") {
            val ctx = appContext.reactContext ?: return@AsyncFunction "unavailable"
            SentoriPushNotifications.currentPermission(ctx)
        }

        AsyncFunction("pushRequestPermission") { promise: expo.modules.kotlin.Promise ->
            val activity = appContext.currentActivity
            if (activity == null) {
                val ctx = appContext.reactContext
                // No Activity to run the permission flow against; fall
                // back to the (non-prompting) current status. Same
                // behaviour as iOS when there's no UI scene.
                promise.resolve(
                    if (ctx != null) SentoriPushNotifications.currentPermission(ctx) else "unavailable"
                )
                return@AsyncFunction
            }
            SentoriPushNotifications.requestPermission(activity) { status ->
                promise.resolve(status)
            }
        }

        Function("pushRegister") {
            appContext.reactContext?.let {
                SentoriPushNotifications.registerForRemoteNotifications(it)
            }
        }

        Function("pushUnregister") {
            appContext.reactContext?.let {
                SentoriPushNotifications.unregisterForRemoteNotifications(it)
            }
        }

        AsyncFunction("pushDrainState") {
            SentoriPushNotifications.drainState()
        }
    }
}
