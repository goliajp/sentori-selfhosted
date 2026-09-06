import Foundation

/// What the user was doing for the last sixty seconds, shipped inside
/// `payload.signals` when an error or warn goes out.
///
/// Bounded and overwrite-oldest. `push` is on the hot path — a tap
/// handler, a navigation — so it does one append and at most one
/// removal, under a lock, and nothing else.
///
/// The window matches the replay buffer deliberately: at 30 s of
/// signals against 60 s of replay, the left half of a case timeline
/// had frames and no events, which reads as "nothing happened" rather
/// than "we were not looking" (insight, round 4).
@objc(SentoriSignalRing)
public final class SentoriSignalRing: NSObject {

    private struct Entry {
        let at: Date
        let kind: String
        let data: [String: Any]?
    }

    private static let defaultCapacity = 100
    private static let defaultWindow: TimeInterval = 60

    private static let lock = NSLock()
    private static var entries: [Entry] = []
    private static var capacity = defaultCapacity
    private static var window = defaultWindow

    @objc public static func configure(capacity: Int, windowSeconds: TimeInterval) {
        lock.lock()
        if capacity > 0 { self.capacity = capacity }
        if windowSeconds > 0 { window = windowSeconds }
        lock.unlock()
    }

    /// Record one signal. Any `kind` is accepted — the server does not
    /// enumerate them, so a host can push its own without waiting for
    /// an SDK release. The five event kinds are a different list and
    /// are validated.
    @objc public static func push(kind: String, data: [String: Any]? = nil) {
        lock.lock()
        entries.append(Entry(at: Date(), kind: kind, data: data))
        if entries.count > capacity {
            entries.removeFirst(entries.count - capacity)
        }
        lock.unlock()
    }

    /// The ring relative to `now`, windowed, oldest first — the shape
    /// `payload.signals` carries. `t` is seconds before now, negative,
    /// to one decimal.
    @objc public static func snapshot(now: Date = Date()) -> [[String: Any]] {
        lock.lock()
        let all = entries
        let cutoff = now.addingTimeInterval(-window)
        let win = window
        lock.unlock()
        _ = win

        return all.compactMap { e in
            guard e.at >= cutoff else { return nil }
            var out: [String: Any] = [
                "t": (e.at.timeIntervalSince(now) * 10).rounded() / 10,
                "kind": e.kind,
            ]
            if let data = e.data { out["data"] = data }
            return out
        }
    }

    @objc public static func clear() {
        lock.lock()
        entries.removeAll()
        lock.unlock()
    }
}
