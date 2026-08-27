import Foundation

#if canImport(UIKit)
    import UIKit
#endif

/// What the device is, as `payload.device` on every event.
///
/// Everything here is cheap and cached: this runs on the calling
/// thread inside every verb, and a verb that stats the filesystem or
/// asks UIKit for a fresh value each time is not the O(1) the
/// contract promises.
///
/// The React Native SDK builds this in two halves — a thin JS-visible
/// subset immediately, enriched from native later — because crossing
/// the bridge is asynchronous. Here there is no bridge, so it is one
/// value computed once.
enum SentoriDevice {

    /// Computed on first use and kept. Model and OS version do not
    /// change during a process; screen size can, so it is read live
    /// but only from an already-resident value.
    private static let base: [String: Any] = {
        var out: [String: Any] = ["os": "ios"]

        #if canImport(UIKit)
            out["osVersion"] = UIDevice.current.systemVersion
        #endif

        // `utsname.machine` is the hardware identifier ("iPhone16,2"),
        // not the marketing name. Mapping it to "iPhone 15 Pro Max"
        // would need a table that goes stale every September; the
        // dashboard does that lookup server-side where it can be
        // updated without a client release.
        var system = utsname()
        uname(&system)
        let model = withUnsafePointer(to: &system.machine) {
            $0.withMemoryRebound(to: CChar.self, capacity: 1) { String(cString: $0) }
        }
        if !model.isEmpty { out["model"] = model }

        return out
    }()

    static func snapshot() -> [String: Any] {
        var out = base

        #if canImport(UIKit)
            // `UIScreen.main` is deprecated in favour of asking a
            // scene, which needs the main thread and a live window —
            // neither is available to a verb called from a background
            // queue during a crash. The bounds are the same value.
            let screen = UIScreen.main
            out["screen"] = [
                "width": screen.bounds.width,
                "height": screen.bounds.height,
                "scale": screen.scale,
            ]
        #endif

        return out
    }
}
