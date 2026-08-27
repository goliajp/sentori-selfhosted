import Foundation

/// The surface a host app writes against.
///
/// Five verbs, three of them a question the person integrating already
/// asks themselves:
///
///     Sentori.error(err)              what went wrong?
///     Sentori.warn("checkout.slow")   where did the user struggle?
///     Sentori.trace("cart.opened")    what happened here?
///     Sentori.assert("total", ok)     should this hold?
///     Sentori.probe("SEN-482")        is that bug back?
///
/// Every one is synchronous, returns the event id it minted, and
/// never throws — including before `start`, where each is a no-op
/// that still hands back an id. An app that mis-wires its token gets
/// a silent SDK, not an exception on a path it did not know it had.
///
/// Ported from `sdk/react-native/src/verbs.ts`; the wire shape is the
/// one `docs/protocol.md` describes and the server's e2e asserts.
@objc(Sentori)
public final class Sentori: NSObject {

    // ── lifecycle ─────────────────────────────────────────────────

    /// Configure and start. Safe to call once; a second call replaces
    /// the config and leaves the queue alone.
    ///
    /// Nothing here reaches out to the network: the first request
    /// happens when there is something to send.
    @objc public static func start(_ config: SentoriConfig) {
        SentoriConfig.set(config)
        SentoriTransport.start()

        // The crash handler runs from inside an uncaught-exception
        // handler where almost nothing is safe, so it reads its
        // release and environment from `UserDefaults` rather than
        // from a live object. Without this a crash ships as
        // `unknown` / `prod` and matches no release, which means no
        // symbolication.
        SentoriCrashHandler.setConfig([
            "release": config.release,
            "environment": config.environment,
        ])
        SentoriCrashHandler.register()

        // The crash that killed the last launch. Until now the
        // handler wrote files into a directory nothing emptied — a
        // crash reporter that captured crashes and never sent one.
        SentoriPendingCrash.ship()
    }

    /// Identify the person using the app. Only a hash of `id` (or
    /// `email` when there is no id) travels; the raw values stay here.
    ///
    /// Call this before registering for push if you want the device
    /// to be reachable from an issue — a device with no user key
    /// receives broadcasts and nothing else.
    @objc public static func user(id: String?, email: String?) {
        SentoriScope.setUser(id: id, email: email)
    }

    /// Merge keys into the ambient context that rides every event.
    @objc public static func context(_ patch: [String: Any]) {
        SentoriScope.patchContext(patch)
    }

    /// Record a signal for the last-sixty-seconds ring that ships
    /// with an error. Any `kind` is accepted — the server does not
    /// enumerate them. The dashboard reads `http` as
    /// `{ method, url, status, ms }` and `trace` as a quiet
    /// breadcrumb.
    ///
    /// This SDK deliberately does not swizzle `URLSession`:
    /// intercepting the host's traffic is the host's decision, not
    /// ours to make silently.
    @objc public static func pushSignal(kind: String, data: [String: Any]? = nil) {
        SentoriSignalRing.push(kind: kind, data: data)
    }

    // ── the verbs ─────────────────────────────────────────────────

    /// Something went wrong. Takes any `Error`; `NSError` keeps its
    /// domain and code, and a Swift error keeps its type name.
    @discardableResult
    @objc public static func error(_ err: Error, data: [String: Any]? = nil) -> String {
        return emit(
            kind: "error",
            name: nil,
            error: describe(err),
            data: data,
            withSignals: true
        )
    }

    /// Something went wrong, described rather than thrown.
    @discardableResult
    @objc public static func error(
        message: String, type: String = "Error", data: [String: Any]? = nil
    ) -> String {
        return emit(
            kind: "error",
            name: nil,
            error: ["type": type, "message": message],
            data: data,
            withSignals: true
        )
    }

    /// The user struggled here. Not a crash — a place the product
    /// hurt, named by you.
    @discardableResult
    @objc public static func warn(_ name: String, data: [String: Any]? = nil) -> String {
        return emit(kind: "warn", name: name, error: nil, data: data, withSignals: true)
    }

    /// This happened. Always lands in the signal ring; `quiet` keeps
    /// it out of the event stream, which is how a high-frequency
    /// breadcrumb stays affordable.
    @discardableResult
    @objc public static func trace(
        _ name: String, data: [String: Any]? = nil, quiet: Bool = false
    ) -> String {
        var signal: [String: Any] = ["name": name]
        if let data { for (k, v) in data { signal[k] = v } }
        SentoriSignalRing.push(kind: "trace", data: signal)
        if quiet { return newEventId() }
        return emit(kind: "trace", name: name, error: nil, data: data, withSignals: false)
    }

    /// This should hold. A passing assert never becomes an event —
    /// it increments a counter that rides the next batch, so a
    /// liveness check does not cost a request. Only failures are
    /// events.
    ///
    /// Unlike the language's own `assert`, this never stops the
    /// program. That difference is the whole point: a monitoring SDK
    /// that can halt the app it monitors has picked the wrong side of
    /// the bargain.
    @discardableResult
    @objc public static func assert(
        _ name: String, _ ok: Bool, data: [String: Any]? = nil
    ) -> String {
        if let release = SentoriConfig.current?.release {
            SentoriTransport.countAssert(name: name, ok: ok, release: release)
        }
        if ok { return newEventId() }
        return emit(kind: "assert", name: name, error: nil, data: data, withSignals: true)
    }

    /// Is that bug back? A tripwire: reaching this call is the
    /// signal. It changes no control flow and returns no verdict.
    @discardableResult
    @objc public static func probe(_ ref: String, data: [String: Any]? = nil) -> String {
        return emit(kind: "probe", name: ref, error: nil, data: data, withSignals: false)
    }

    // ── assembly ──────────────────────────────────────────────────

    private static func emit(
        kind: String,
        name: String?,
        error: [String: Any]?,
        data: [String: Any]?,
        withSignals: Bool
    ) -> String {
        let id = newEventId()
        // Before init every verb is a no-op that still returns an id.
        // The host's call sites do not change shape depending on
        // whether the SDK came up.
        guard let config = SentoriConfig.current else { return id }

        var payload: [String: Any] = [:]
        if let error { payload["error"] = error }
        if let data { payload["data"] = data }
        if let ctx = SentoriScope.context { payload["context"] = ctx }
        if withSignals {
            let signals = SentoriSignalRing.snapshot()
            if !signals.isEmpty { payload["signals"] = signals }
        }
        payload["device"] = SentoriDevice.snapshot()

        var event: [String: Any] = [
            "id": id,
            "kind": kind,
            "occurredAt": iso8601(Date()),
            "platform": "ios",
            "release": config.release,
            "environment": config.environment,
            "payload": payload,
        ]
        if let name { event["name"] = name }
        if let userKey = SentoriScope.userKey { event["userKey"] = userKey }

        SentoriTransport.enqueue(event)
        return id
    }

    /// `NSError` carries a domain and code worth keeping; anything
    /// else gets its type name, which for a Swift enum error is the
    /// case as written.
    private static func describe(_ err: Error) -> [String: Any] {
        let ns = err as NSError
        var out: [String: Any] = [
            "type": String(describing: type(of: err)),
            "message": ns.localizedDescription,
        ]
        if ns.domain != "" {
            out["domain"] = ns.domain
            out["code"] = ns.code
        }
        return out
    }

    private static let formatter: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    static func iso8601(_ date: Date) -> String { formatter.string(from: date) }

    /// UUIDv7: 48 bits of milliseconds then random, so ids sort by
    /// creation time. The server stores them as the event's primary
    /// key and the dashboard orders by it, so a v4 here would scatter
    /// a session's events across the index.
    static func newEventId() -> String {
        var bytes = [UInt8](repeating: 0, count: 16)
        for i in 0..<16 { bytes[i] = UInt8.random(in: 0...255) }

        let ms = UInt64(Date().timeIntervalSince1970 * 1000)
        bytes[0] = UInt8((ms >> 40) & 0xff)
        bytes[1] = UInt8((ms >> 32) & 0xff)
        bytes[2] = UInt8((ms >> 24) & 0xff)
        bytes[3] = UInt8((ms >> 16) & 0xff)
        bytes[4] = UInt8((ms >> 8) & 0xff)
        bytes[5] = UInt8(ms & 0xff)
        bytes[6] = (bytes[6] & 0x0f) | 0x70  // version 7
        bytes[8] = (bytes[8] & 0x3f) | 0x80  // variant

        let hex = bytes.map { String(format: "%02x", $0) }.joined()
        let s = Array(hex)
        return String(s[0..<8]) + "-" + String(s[8..<12]) + "-" + String(s[12..<16]) + "-"
            + String(s[16..<20]) + "-" + String(s[20..<32])
    }
}

extension String {
    fileprivate init(_ slice: ArraySlice<Character>) { self.init(String(Array(slice))) }
}
