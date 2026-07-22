import ExpoModulesCore

/// Expo Module exposing the iOS crash handler to JS.
///
/// JS contract (mirrored in src/native.ts):
///   - setConfig({ token, release, environment }): stash for the crash
///     writer. Token is currently unused at native side; release and
///     environment are baked into the saved event JSON.
///   - drainPending() → string[]: read & delete all pending crash files
///     from <Documents>/sentori/pending and return their JSON bodies.
public class SentoriModule: Module {
    public func definition() -> ModuleDefinition {
        Name("Sentori")

        OnCreate {
            SentoriCrashHandler.register()
            // v0.9.4 #1 — capture cold-start anchor + start frame watch.
            SentoriMobileVitals.registerColdStartAnchor()
            SentoriMobileVitals.startFrameWatch()
        }

        // v0.9.5 #8 — TurboModule exception bridge readout for
        // coerceError to attach native stack to wrapped JSError.
        Function("getRecentNativeException") { () -> [String: Any]? in
            return SentoriNativeExceptionBridge.getRecentException()
        }

        // v0.9.6 #2 — wireframe session replay capture.
        Function("captureWireframe") { (maskedIds: [String]) -> String? in
            return SentoriReplayCapture.captureWireframe(maskedIds: maskedIds)
        }

        // v0.9.12 — diagnostic readout for replay. Returns the last
        // keyWindow resolution path + scene/window counts so a single
        // JS-side button can answer "why is my ring empty?" without
        // re-rolling the pod. See SentoriReplayCapture.swift.
        Function("probeWireframe") { () -> [String: Any] in
            return SentoriReplayCapture.probe()
        }

        // v1.0.0-rc.2 — diagnostic readout for screenshot. Same shape
        // as Android side; lets Insight ship raw state back when the
        // captureScreenshot path returns null.
        Function("probeScreenshot") { () -> [String: Any] in
            return SentoriScreenshotCapture.probe()
        }

        // v0.9.4 #1 — Mobile Vitals exposure.
        Function("markJsBridgeReady") {
            SentoriMobileVitals.markJsBridgeReady()
        }
        Function("getColdStartMs") { () -> Double? in
            return SentoriMobileVitals.getColdStartMs()?.doubleValue
        }
        Function("getFrameCounters") { () -> [String: Any]? in
            return SentoriMobileVitals.getFrameCounters() as? [String: Any]
        }
        Function("resetFrameCounters") {
            SentoriMobileVitals.resetFrameCounters()
        }

        Function("setConfig") { (config: [String: Any]) in
            SentoriCrashHandler.setConfig(config)
        }

        AsyncFunction("drainPending") { () -> [String] in
            return SentoriCrashHandler.consumePending()
        }

        // v0.7.3 — JS-triggered screenshot path with consumer-supplied
        // mask IDs. JS owns the registry of `nativeID`s to redact;
        // native walks the view tree and paints black rectangles in
        // the rendered bitmap. Returns `nil` (resolves to `null` in
        // JS) when there's no key window or render fails. Replaces
        // the previous `react-native-view-shot` peer-dep path.
        AsyncFunction("captureScreenshotWithMask") { (maskedIds: [String]) -> [String: String]? in
            return SentoriScreenshotCapture.captureScreenshotWithMask(maskedIds: maskedIds)
        }

        // Phase 22 sub-E: opt-in iOS hang watchdog. Same JS function
        // name as Android (sub-D) so the host app calls
        // `startAnrWatchdog(...)` once, both platforms react.
        // Defaults: 2 s timeout, 1 s tick interval, debug-build off.
        Function("startAnrWatchdog") { (options: [String: Any]?) in
            let timeoutMs = (options?["timeoutMs"] as? Int) ?? 2000
            let intervalMs = (options?["intervalMs"] as? Int) ?? 1000
            let force = (options?["force"] as? Bool) ?? false
            SentoriHangWatchdog.start(
                timeoutMs: timeoutMs,
                intervalMs: intervalMs,
                force: force
            )
        }

        Function("stopAnrWatchdog") {
            SentoriHangWatchdog.stop()
        }

        // Dev-only helper used by the example app to verify the
        // crash-write / drain round-trip without writing native code in
        // the host app. Schedules a real NSException after a tick so
        // the JS bridge has time to return; the resulting crash hits
        // SentoriCrashHandler and writes a JSON file under
        // <Documents>/sentori/pending/.
        Function("triggerTestNativeCrash") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                NSException(
                    name: NSExceptionName("SentoriTestException"),
                    reason: "Sentori test native crash",
                    userInfo: nil
                ).raise()
            }
        }

        // v2.9 — push notification bridge.
        //
        // Five Functions / AsyncFunctions form the surface that
        // `sdk/react-native/src/push.ts` consumes:
        //
        //   pushGetStatus         — non-prompting status read
        //   pushRequestPermission — triggers the OS prompt if undecided
        //   pushRegister          — UIApplication.registerForRemoteNotifications
        //   pushUnregister        — UIApplication.unregisterForRemoteNotifications
        //   pushDrainState        — token / notifications / taps buffer drain
        //
        // All five route through `SentoriPushNotifications.shared`,
        // which also installs the AppDelegate method swizzle that
        // routes APNs token callbacks into the buffer.

        AsyncFunction("pushGetStatus") { (promise: Promise) in
            SentoriPushNotifications.shared.currentPermission { status in
                promise.resolve(status)
            }
        }

        AsyncFunction("pushRequestPermission") { (promise: Promise) in
            SentoriPushNotifications.shared.requestPermission { status in
                promise.resolve(status)
            }
        }

        Function("pushRegister") {
            SentoriPushNotifications.shared.registerForRemoteNotifications()
        }

        Function("pushUnregister") {
            SentoriPushNotifications.shared.unregisterForRemoteNotifications()
        }

        AsyncFunction("pushDrainState") { () -> [String: Any] in
            return SentoriPushNotifications.shared.drainState()
        }
    }
}
