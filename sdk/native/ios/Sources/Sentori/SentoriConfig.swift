import Foundation

/// Set once by `Sentori.start`; everything else reads it and treats
/// absence as "not initialised, so every verb is a no-op".
///
/// That default is the failure-isolation half of the client zero-cost
/// rule: an app that mis-wires its token gets a silent SDK, never an
/// exception on a path it did not know it had.
@objc(SentoriConfig)
public final class SentoriConfig: NSObject {

    @objc public let token: String
    @objc public let ingestUrl: String
    /// `@objc(releaseName)` because `release` is `NSObject`'s own
    /// selector — exposing a property under it would override memory
    /// management. Swift callers still write `config.release`.
    @objc(releaseName) public let release: String
    @objc public let environment: String
    /// The integrator's own health endpoint, carried on batches and
    /// probed server-side. The app never pings it — a monitoring SDK
    /// that adds traffic to the thing it monitors has picked the wrong
    /// side of the bargain.
    @objc public let backendHealthUrl: String?

    @objc public init(
        token: String,
        ingestUrl: String,
        release: String,
        environment: String,
        backendHealthUrl: String? = nil
    ) {
        self.token = token
        // A trailing slash here becomes `//v1/events:batch`, which
        // some proxies answer and some 404.
        self.ingestUrl =
            ingestUrl.hasSuffix("/") ? String(ingestUrl.dropLast()) : ingestUrl
        self.release = release
        self.environment = environment
        self.backendHealthUrl = backendHealthUrl
    }

    private static let lock = NSLock()
    private static var _current: SentoriConfig?

    @objc public static var current: SentoriConfig? {
        lock.lock()
        defer { lock.unlock() }
        return _current
    }

    static func set(_ config: SentoriConfig?) {
        lock.lock()
        _current = config
        lock.unlock()
    }

    @objc public static var isInitialised: Bool { current != nil }

    static func __resetForTests() { set(nil) }
}

/// Reported in the `Sentori-Sdk` header so a server-side problem can
/// be attributed to a client version.
///
/// Mirrors `sdk/react-native/src/transport.ts`'s constant and is
/// written by `scripts/sync-sdk-version.mjs` — the same mechanism, for
/// the same reason: a version string nothing writes goes stale, and it
/// only ever goes stale in the direction of a lie.
public enum SentoriVersion {
    public static let current = "2.0.0"
}
