import Foundation
import UIKit
import QuartzCore

/// v0.9.4 #1 — Mobile Vitals: cold start measurement + slow/frozen
/// frame counters.
///
/// Cold start is read at `applicationDidFinishLaunching` time. We use
/// `mach_absolute_time` because it's monotonic + survives clock
/// adjustments. The JS bridge reads via `getColdStartMs()` once at
/// `sentori.init` and the value rides along with the first event.
///
/// Frame counters hook `CADisplayLink` on the main run loop. The
/// callback compares actual vs expected timestamp; > 16.67ms = slow
/// (one missed VSync at 60fps), > 700ms = frozen (per Sentry's
/// definition for parity). Counters reset on navigation transition
/// — `resetFrameCounters()` from JS side.
@objc public final class SentoriMobileVitals: NSObject {

    private static var coldStartCapturedAt: UInt64 = 0
    private static var jsBridgeReadyAt: UInt64 = 0
    private static var coldStartMs: Double? = nil

    private static var slowFrames: Int = 0
    private static var frozenFrames: Int = 0
    private static var displayLink: CADisplayLink? = nil
    private static var lastFrameTimestamp: CFTimeInterval = 0

    private static let SLOW_FRAME_MS: Double = 16.67
    private static let FROZEN_FRAME_MS: Double = 700.0

    /// Call from app delegate or earliest reachable point. Stores
    /// the cold-start anchor. Safe to call multiple times (only the
    /// first effective time wins).
    @objc public static func registerColdStartAnchor() {
        if coldStartCapturedAt == 0 {
            coldStartCapturedAt = mach_absolute_time()
        }
    }

    /// Called by the bridge when JS init() runs. The delta from the
    /// app-delegate anchor → here is the cold-start budget the
    /// user perceived.
    @objc public static func markJsBridgeReady() {
        if jsBridgeReadyAt != 0 { return }
        jsBridgeReadyAt = mach_absolute_time()
        if coldStartCapturedAt > 0 {
            var info = mach_timebase_info()
            mach_timebase_info(&info)
            let elapsed = (jsBridgeReadyAt - coldStartCapturedAt) * UInt64(info.numer) / UInt64(info.denom)
            // ns → ms
            coldStartMs = Double(elapsed) / 1_000_000.0
        }
    }

    @objc public static func getColdStartMs() -> NSNumber? {
        if let ms = coldStartMs {
            return NSNumber(value: ms)
        }
        return nil
    }

    /// Start frame budget watch. Idempotent. Hooks CADisplayLink on
    /// the main run loop's common modes so it ticks even during
    /// scroll views.
    @objc public static func startFrameWatch() {
        if displayLink != nil { return }
        DispatchQueue.main.async {
            let link = CADisplayLink(target: self, selector: #selector(onFrame(_:)))
            link.add(to: .main, forMode: .common)
            displayLink = link
        }
    }

    @objc public static func stopFrameWatch() {
        displayLink?.invalidate()
        displayLink = nil
    }

    @objc public static func getFrameCounters() -> NSDictionary? {
        return [
            "slow": slowFrames,
            "frozen": frozenFrames,
        ]
    }

    @objc public static func resetFrameCounters() {
        slowFrames = 0
        frozenFrames = 0
    }

    @objc private static func onFrame(_ link: CADisplayLink) {
        let now = link.timestamp
        if lastFrameTimestamp != 0 {
            let deltaMs = (now - lastFrameTimestamp) * 1000.0
            if deltaMs >= FROZEN_FRAME_MS {
                frozenFrames += 1
            } else if deltaMs >= SLOW_FRAME_MS {
                slowFrames += 1
            }
        }
        lastFrameTimestamp = now
    }
}
