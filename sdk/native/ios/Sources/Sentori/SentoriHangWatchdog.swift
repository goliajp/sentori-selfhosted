import Foundation

/// iOS hang detector — mirrors the Android ANR watchdog (Phase 22 sub-D).
///
/// A background thread posts a tick onto the main queue every
/// `intervalMs` and waits `timeoutMs` for it to run. If the main run
/// loop didn't drain the tick in time we capture the main thread's
/// call stack and write a Sentori event with `kind = "anr"` (the
/// dashboard already groups Android ANR + iOS hang under that kind —
/// the user-visible distinction lives in `tags.source`).
///
/// Single-shot per hang: once we report, we wait for the tick to
/// land before re-arming so a 30-second freeze doesn't dump six
/// events. Daemon thread (DispatchSourceTimer in a background
/// queue) so it can't keep the process alive on shutdown.
///
/// Disabled in DEBUG builds by default — the Xcode debugger
/// pauses the main thread routinely and we don't want a flood. The
/// host app can override via `start(force: true)`.
@objc public final class SentoriHangWatchdog: NSObject {

    private static let pendingDirName = "sentori/pending"
    private static let configKey = "com.sentori.config"

    private static var running: Bool = false
    private static var queue: DispatchQueue?
    private static var timer: DispatchSourceTimer?
    private static let lock = NSLock()

    /// Start the watchdog. Idempotent. **Must be called from the main
    /// thread** so the sampler can capture main's mach port; if called
    /// from a background thread the sampler stays uninstalled and the
    /// watchdog falls back to `Thread.callStackSymbols`.
    @objc public static func start(timeoutMs: Int, intervalMs: Int, force: Bool) {
        lock.lock()
        defer { lock.unlock() }
        if running { return }
        if isDebug() && !force { return }

        // Capture main's mach port for the Phase 29 sub-A sampler. No-op
        // if we're not on main; sampler will then return [] and we
        // gracefully fall back at capture time.
        SentoriThreadSampler.installMainThreadHandle()

        let q = DispatchQueue(label: "com.sentori.hangWatchdog", qos: .utility)
        let t = DispatchSource.makeTimerSource(queue: q)
        let interval = DispatchTimeInterval.milliseconds(intervalMs)
        let timeoutNs = UInt64(max(0, timeoutMs)) * NSEC_PER_MSEC

        // The "tick" path: post a Bool flip onto the main queue and
        // record the time we did so. Read both fields in the worker
        // tick to decide whether the main loop is alive.
        let state = HangState()

        t.schedule(deadline: .now() + interval, repeating: interval)
        t.setEventHandler {
            // If the previous tick is still pending after the timeout
            // window, the main thread is wedged. Capture once, then
            // hold off until the main loop catches up.
            if state.armed.value, let armedAt = state.armedAt.value {
                let elapsedNs = DispatchTime.now().uptimeNanoseconds &- armedAt.uptimeNanoseconds
                if elapsedNs >= timeoutNs && !state.reportedThisHang.value {
                    state.reportedThisHang.value = true
                    captureHang(durationMs: Int(elapsedNs / NSEC_PER_MSEC))
                }
                return
            }

            // Main is responsive — re-arm.
            state.armed.value = true
            state.armedAt.value = DispatchTime.now()
            state.reportedThisHang.value = false
            DispatchQueue.main.async {
                state.armed.value = false
                state.armedAt.value = nil
            }
        }
        t.resume()
        timer = t
        queue = q
        running = true
    }

    @objc public static func stop() {
        lock.lock()
        defer { lock.unlock() }
        timer?.cancel()
        timer = nil
        queue = nil
        running = false
    }

    private static func isDebug() -> Bool {
        #if DEBUG
            return true
        #else
            return false
        #endif
    }

    // MARK: - capture

    private static func captureHang(durationMs: Int) {
        let cfg = UserDefaults.standard.dictionary(forKey: configKey) ?? [:]
        let release = (cfg["release"] as? String) ?? "unknown"
        let environment = (cfg["environment"] as? String) ?? "prod"

        // Phase 29 sub-A: try the Mach-based main-thread sampler first.
        // It walks main's frame pointer chain via thread_get_state +
        // vm_read_overwrite (see PRIVACY_AND_REVIEW.md). Returns [] on
        // non-arm64 platforms or if installMainThreadHandle was never
        // called from main; we then fall back to this thread's own
        // stack — biased toward dispatch machinery but better than
        // nothing.
        let pcs = SentoriThreadSampler.captureMainThreadFrames(maxFrames: 64)
        let frames: [[String: Any]]
        let stackSource: String
        if !pcs.isEmpty {
            frames = pcs.map { pc -> [String: Any] in
                return [
                    "function": "<unsymbolicated>",
                    "file": "<unknown>",
                    "line": 0,
                    "instructionAddress": String(format: "0x%llx", pc.uint64Value),
                    "arch": "arm64",
                    "inApp": true,
                ]
            }
            stackSource = "sentori.hangWatchdog.sampler"
        } else {
            frames = Thread.callStackSymbols.map { sym -> [String: Any] in
                let parts = sym.split(
                    separator: " ", omittingEmptySubsequences: true
                ).map(String.init)
                let module = parts.count > 1 ? parts[1] : "<unknown>"
                let function =
                    parts.count > 3
                    ? parts.dropFirst(3).joined(separator: " ")
                    : "<anonymous>"
                return [
                    "function": function,
                    "file": module,
                    "line": 0,
                    "inApp": !module.contains("UIKit")
                        && !module.contains("Foundation")
                        && !module.contains("CoreFoundation")
                        && !module.contains("libsystem")
                        && !module.contains("libobjc"),
                ]
            }
            stackSource = "sentori.hangWatchdog.no-sampler"
        }

        let event: [String: Any] = [
            "id": UUID().uuidString.lowercased(),
            "timestamp": iso8601(Date()),
            "kind": "anr",
            "platform": "ios",
            "release": release,
            "environment": environment,
            "device": [
                "os": "ios",
                "osVersion": osVersion(),
                "model": deviceModel(),
            ],
            "app": appInfo(),
            "user": NSNull(),
            "tags": ["source": stackSource],
            "breadcrumbs": [Any](),
            "error": [
                "type": "ApplicationNotResponding",
                "message": "Main thread blocked for ≥ \(durationMs) ms",
                "stack": frames,
                "cause": NSNull(),
            ],
            "fingerprint": [String](),
            "traceId": NSNull(),
            "spanId": NSNull(),
        ]

        guard
            let docs = FileManager.default.urls(
                for: .documentDirectory, in: .userDomainMask
            ).first
        else { return }
        let dir = docs.appendingPathComponent(pendingDirName)
        try? FileManager.default.createDirectory(
            at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent(
            "\(UUID().uuidString.lowercased()).json")
        if let data = try? JSONSerialization.data(
            withJSONObject: event, options: [])
        {
            try? data.write(to: url)
        }
    }

    private static func iso8601(_ date: Date) -> String {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f.string(from: date)
    }

    private static func osVersion() -> String {
        let v = ProcessInfo.processInfo.operatingSystemVersion
        return "\(v.majorVersion).\(v.minorVersion).\(v.patchVersion)"
    }

    private static func deviceModel() -> String {
        var s = utsname()
        uname(&s)
        return withUnsafePointer(to: &s.machine) { ptr in
            ptr.withMemoryRebound(to: CChar.self, capacity: 1) {
                String(validatingUTF8: $0) ?? "unknown"
            }
        }
    }

    private static func appInfo() -> [String: Any] {
        let info = Bundle.main.infoDictionary ?? [:]
        var d: [String: Any] = [
            "version": (info["CFBundleShortVersionString"] as? String)
                ?? "0.0.0"
        ]
        if let build = info["CFBundleVersion"] as? String {
            d["build"] = build
        }
        return d
    }
}

/// Mutable boxes shared between the watchdog tick and the main-queue
/// ack. Class so the closures can mutate without `inout` and so the
/// references survive across the `setEventHandler` capture.
private final class HangState {
    let armed = Box(false)
    let armedAt = Box<DispatchTime?>(nil)
    let reportedThisHang = Box(false)
}

private final class Box<T> {
    var value: T
    init(_ v: T) { self.value = v }
}
