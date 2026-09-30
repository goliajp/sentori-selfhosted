// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/ios/Sources/Sentori/SentoriLifecycle.swift
// Run `node scripts/sync-native-core.mjs` after editing it.
#if canImport(UIKit)
    import UIKit
#endif
import Foundation

/// What the app was doing when something went wrong.
///
/// Two things were missing without this. The signal ring — the sixty
/// seconds of context that ride an error — had no idea whether the app
/// was in the foreground, had just come back from it, or was under
/// memory pressure, which is the first question anyone asks about a
/// crash they cannot reproduce. And an event queued in the seconds
/// before the user swiped the app away went with it: the flush timer
/// had not fired, so the report of the thing that made them leave was
/// the report that never arrived.
///
/// Observers, not polling. The cost is four `NotificationCenter`
/// registrations at `start()` and nothing at all until the system has
/// something to say.
enum SentoriLifecycle {
    private static var registered = false
    private static var tokens: [NSObjectProtocol] = []

    static func register() {
        guard !registered else { return }
        registered = true

        #if canImport(UIKit)
            let center = NotificationCenter.default

            // Flushing here rather than only recording it: this is the
            // last moment the process is reliably alive, and iOS gives
            // us the call before it suspends.
            observe(center, UIApplication.didEnterBackgroundNotification, "app.background") {
                SentoriTransport.flush()
            }
            observe(center, UIApplication.willEnterForegroundNotification, "app.foreground", nil)
            observe(center, UIApplication.didBecomeActiveNotification, "app.active", nil)

            // A crash a minute after this is very often this.
            observe(
                center, UIApplication.didReceiveMemoryWarningNotification, "app.memoryWarning", nil
            )

            // `willTerminate` does not arrive for every death — a
            // watchdog kill or a jetsam skips it — so it is a bonus,
            // not the mechanism.
            observe(center, UIApplication.willTerminateNotification, "app.terminate") {
                SentoriTransport.flush()
            }
        #endif
    }

    #if canImport(UIKit)
        private static func observe(
            _ center: NotificationCenter,
            _ name: Notification.Name,
            _ signal: String,
            _ then: (() -> Void)?
        ) {
            let token = center.addObserver(forName: name, object: nil, queue: nil) { _ in
                // Iron rule, dimension 3: a callback of ours that
                // threw would surface inside the host's own lifecycle
                // notification, where it looks like their bug.
                SentoriSignalRing.push(kind: "lifecycle", data: ["name": signal])
                then?()
            }
            tokens.append(token)
        }
    #endif

    static func __resetForTests() {
        for token in tokens { NotificationCenter.default.removeObserver(token) }
        tokens = []
        registered = false
    }
}
