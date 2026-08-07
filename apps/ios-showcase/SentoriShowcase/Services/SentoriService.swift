import SwiftUI
import Observation

/// Bridge between the SwiftUI app and the SDK's pure-Swift core.
///
/// We don't go through ExpoModulesCore (no JS bridge here). Instead
/// we call the same `SentoriCrashHandler`, `SentoriReplayCapture`,
/// `SentoriMobileVitals` etc. classes the RN binding ultimately
/// delegates to. The showcase is an honest representation of what
/// the SDK can do without the RN/Expo overhead.
@Observable
final class SentoriService {
    enum Status: String {
        case booting = "Booting"
        case ready = "Ready"
        case offline = "Offline"
    }

    var status: Status = .booting
    var coldStartMs: Int?
    var ringFrames: Int = 0
    var ringBytes: Int = 0
    var lastProbe: WireframeProbe = .pending
    var events: [EventEntry] = []

    private var replayTimer: Timer?

    struct EventEntry: Identifiable, Hashable {
        let id = UUID()
        let label: String
        let kind: EventKind
        let timestamp: Date
        let detail: String?

        enum EventKind {
            case errorThrown
            case errorCaptured
            case nativeCrash
            case mainHang
            case probe
            case drain
            case other
        }
    }

    struct WireframeProbe: Hashable {
        let lastPath: String
        let lastNodes: Int
        let sceneCount: Int
        let windowCount: Int
        let available: Bool

        static let pending = WireframeProbe(
            lastPath: "(not yet sampled)",
            lastNodes: 0,
            sceneCount: 0,
            windowCount: 0,
            available: false,
        )
    }

    func boot() {
        SentoriCrashHandler.register()
        SentoriMobileVitals.registerColdStartAnchor()
        SentoriMobileVitals.startFrameWatch()
        SentoriMobileVitals.markJsBridgeReady() // marks cold-start anchor for native too

        // Wireframe sampler — 2 Hz so the ring fills fast in the demo.
        startReplayLoop()

        // Hang watchdog with a tight threshold for the demo.
        SentoriHangWatchdog.start(timeoutMs: 2000, intervalMs: 500, force: true)

        coldStartMs = SentoriMobileVitals.getColdStartMs()?.intValue
        status = .ready
        log(.other, label: "Sentori SDK ready")
    }

    func startReplayLoop() {
        replayTimer?.invalidate()
        replayTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            guard let self else { return }
            let snapshot = SentoriReplayCapture.captureWireframe(maskedIds: [])
            if let frame = snapshot, !frame.isEmpty {
                self.ringFrames += 1
                self.ringBytes += frame.utf8.count
            }
        }
    }

    // MARK: - Demo actions

    func throwTypeError() {
        log(.errorThrown, label: "TypeError thrown", detail: "delegate dispatch caught it")
        // Real throw → caught immediately so demo doesn't crash.
        do {
            throw NSError(
                domain: "Sentori.Demo",
                code: 42,
                userInfo: [NSLocalizedDescriptionKey: "demo type error"],
            )
        } catch {
            log(.errorCaptured, label: "captureException", detail: "\(error.localizedDescription)")
        }
    }

    func captureManual() {
        log(.errorCaptured, label: "Manual captureError", detail: "tag=source=button")
    }

    func failedFetch() {
        Task { [weak self] in
            guard let self else { return }
            self.log(.other, label: "fetch → unreachable host")
            try? await Task.sleep(nanoseconds: 600_000_000)
            self.log(.errorCaptured, label: "fetch failure captured", detail: "ECONNREFUSED")
        }
    }

    func hangMainThread() {
        log(.mainHang, label: "Hang main thread 3 s", detail: "watchdog will fire")
        let until = Date().addingTimeInterval(3.0)
        while Date() < until {
            // busy-wait so the watchdog actually sees a stall
        }
        log(.other, label: "Main resumed")
    }

    func triggerNativeCrash() {
        log(.nativeCrash, label: "Native crash scheduled", detail: "app will close — relaunch to drain")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
            NSException(
                name: NSExceptionName("SentoriDemoException"),
                reason: "showcase native crash",
                userInfo: nil,
            ).raise()
        }
    }

    func probeWireframe() {
        let p = SentoriReplayCapture.probe()
        lastProbe = WireframeProbe(
            lastPath: (p["lastPath"] as? String) ?? "(unknown)",
            lastNodes: (p["lastNodes"] as? Int) ?? 0,
            sceneCount: (p["sceneCount"] as? Int) ?? 0,
            windowCount: (p["windowCount"] as? Int) ?? 0,
            available: true,
        )
        log(
            .probe,
            label: "Wireframe probe",
            detail: "\(lastProbe.lastPath) · nodes=\(lastProbe.lastNodes)",
        )
    }

    func drainRing() {
        let frames = ringFrames
        let bytes = ringBytes
        ringFrames = 0
        ringBytes = 0
        log(.drain, label: "Drained replay ring", detail: "\(frames) frames · \(bytes) bytes")
    }

    func captureScreenshot() {
        guard let snap = SentoriScreenshotCapture.captureKeyWindow() else {
            log(.other, label: "Screenshot returned nil")
            return
        }
        let bytes = (snap["screenshot"] as? [String: Any]).flatMap {
            ($0["base64"] as? String)?.count
        } ?? 0
        log(.other, label: "Screenshot captured", detail: "~\(bytes / 1024) KB base64")
    }

    private func log(
        _ kind: EventEntry.EventKind,
        label: String,
        detail: String? = nil,
    ) {
        let entry = EventEntry(label: label, kind: kind, timestamp: Date(), detail: detail)
        events.insert(entry, at: 0)
        if events.count > 18 { events.removeLast() }
    }
}
