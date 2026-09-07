import Foundation

/// v0.9.5 #8 — partial-fix for the "TurboModule swallows NSException
/// into a generic JSError" gap.
///
/// We can't easily swizzle the C++ ObjCTurboModule call site that does
/// the swallowing. What we *can* offer is an escape hatch: host code
/// inside a TurboModule method wraps its native call in `@try @catch`
/// and calls `SentoriNativeExceptionBridge.record(exception)` from the
/// catch block. We stash the exception (name + reason + callStackSymbols)
/// in a static ring with timestamps. When the JS side then receives
/// the generic JSError that RN wraps it into, `coerceError` checks the
/// ring for an exception within the last 1 s and attaches the native
/// stack to the JS error event.
///
/// Usage from a host TurboModule (Swift example):
///
///   @objc func mySensitiveMethod() {
///     do {
///       try riskyNativeOperation()
///     } catch let nsException as NSException {
///       SentoriNativeExceptionBridge.record(nsException)
///       throw nsException
///     }
///   }
///
/// Or Objective-C:
///
///   @try {
///     riskyOp();
///   } @catch (NSException *e) {
///     [SentoriNativeExceptionBridge recordException:e];
///     @throw;
///   }

@objc public final class SentoriNativeExceptionBridge: NSObject {

    private static let RING_SIZE = 8
    private static let WINDOW_MS: Double = 1000

    private struct Stash {
        let timestamp: Date
        let name: String
        let reason: String
        let stack: [String]
    }

    private static var ring: [Stash] = []
    private static let lock = NSLock()

    /// Called from a `@catch` inside a TurboModule method. Records
    /// the exception's name + reason + callStackSymbols for ~1 s so
    /// the JS-side coerceError can pick it up.
    @objc public static func recordException(_ exception: NSException) {
        let stash = Stash(
            timestamp: Date(),
            name: exception.name.rawValue,
            reason: exception.reason ?? "",
            stack: exception.callStackSymbols
        )
        lock.lock()
        defer { lock.unlock() }
        ring.append(stash)
        while ring.count > RING_SIZE {
            ring.removeFirst()
        }
    }

    /// Called by JS-side bridge. Returns the most recent exception
    /// within the last 1 s, or nil. Does NOT remove from the ring —
    /// the same NSException may surface as multiple JSError frames
    /// across the bridge. Ring is cleared by `purge()` on a timer.
    @objc public static func getRecentException() -> [String: Any]? {
        lock.lock()
        defer { lock.unlock() }
        purgeLocked()
        guard let latest = ring.last else { return nil }
        return [
            "name": latest.name,
            "reason": latest.reason,
            "stack": latest.stack,
            "ageMs": Int(Date().timeIntervalSince(latest.timestamp) * 1000),
        ]
    }

    private static func purgeLocked() {
        let now = Date()
        ring.removeAll { now.timeIntervalSince($0.timestamp) * 1000 > WINDOW_MS }
    }
}
