// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/ios/Sources/Sentori/SentoriReplayDriver.swift
// Run `node scripts/sync-native-core.mjs` after editing it.
import Foundation

/// Runs the replay: tick, capture, ring.
///
/// Off unless the host asks for it. Replay is the most expensive thing
/// this SDK can do, and the rule for anything in that class is that
/// the customer turns it on deliberately rather than discovering it in
/// a profile.
///
/// ## The timer is not on the main thread
///
/// `SentoriReplayCapture.captureWireframe` reads the view hierarchy,
/// which only the main thread may do, so when called from elsewhere it
/// does `DispatchQueue.main.sync`. A timer scheduled on the main
/// queue would therefore dispatch to the queue it is already running
/// on and wait for itself — the app freezes, and the freeze is our
/// fault in a product whose entire pitch is that it costs the host
/// nothing. So the timer lives on its own serial queue and blocks
/// there, where blocking costs nobody a frame.
public enum SentoriReplayDriver {
    private static let queue = DispatchQueue(label: "jp.golia.sentori.replay", qos: .utility)
    private static let lock = NSLock()
    private static var timer: DispatchSourceTimer?
    private static var ring = SentoriReplay.Ring()

    /// Ticks per second. Two is enough to follow a user through a
    /// screen; four is for motion-heavy apps that would rather spend
    /// the CPU.
    public static let defaultHz: Double = 2

    /// Start capturing. Calling it twice is a no-op rather than a
    /// second timer — a host that calls `start()` from two entry
    /// points should not pay twice.
    public static func start(hz: Double = defaultHz, keyframeMs: Double = 4000) {
        lock.lock()
        defer { lock.unlock() }
        guard timer == nil else { return }

        ring = SentoriReplay.Ring(keyframeMs: keyframeMs)
        let interval = 1.0 / max(hz, 0.1)
        let source = DispatchSource.makeTimerSource(queue: queue)
        source.schedule(deadline: .now() + interval, repeating: interval)
        source.setEventHandler { tick() }
        source.resume()
        timer = source
    }

    public static func stop() {
        lock.lock()
        defer { lock.unlock() }
        timer?.cancel()
        timer = nil
    }

    /// The window so far, as the newline-delimited JSON the player
    /// reads, and start again cold. Empty when nothing was captured.
    public static func drain() -> String {
        let entries = ring.drain()
        guard !entries.isEmpty else { return "" }
        return entries.compactMap { entry -> String? in
            guard let data = try? JSONSerialization.data(withJSONObject: entry, options: []) else {
                return nil
            }
            return String(data: data, encoding: .utf8)
        }
        .joined(separator: "\n")
    }

    private static func tick() {
        // Iron rule, dimension 3: this runs on our own queue, but a
        // throw here would end the timer's event handler and every
        // tick after it — a replay that silently stopped, which reads
        // as a quiet app rather than as a broken SDK.
        let json = SentoriReplayCapture.captureWireframe(maskedIds: SentoriMask.ids())
        guard let json, !json.isEmpty,
            let data = json.data(using: .utf8),
            let raw = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return }

        let nodes = (raw["nodes"] as? [[String: Any]] ?? []).map { item in
            SentoriReplay.Node(
                x: item["x"] as? Double ?? 0,
                y: item["y"] as? Double ?? 0,
                w: item["w"] as? Double ?? 0,
                h: item["h"] as? Double ?? 0,
                kind: item["kind"] as? String,
                text: item["text"] as? String,
                color: item["color"] as? String
            )
        }
        ring.push(
            SentoriReplay.Frame(
                ts: raw["ts"] as? Double ?? Date().timeIntervalSince1970 * 1000,
                width: raw["width"] as? Double ?? 0,
                height: raw["height"] as? Double ?? 0,
                nodes: nodes
            )
        )
    }

    static func __isRunningForTests() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return timer != nil
    }

    static func __pushForTests(_ frame: SentoriReplay.Frame) {
        ring.push(frame)
    }

    static func __resetForTests() {
        stop()
        _ = ring.drain()
    }
}
