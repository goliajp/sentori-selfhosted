import Foundation

/// Static crash handler — captures NSException and writes one JSON file
/// per crash to <Documents>/sentori/pending/<uuid>.json. JS drains that
/// directory on next launch via `Sentori.drainPending()`.
///
/// What this does NOT do (Phase 7 v0.1):
///   - signal-based native crashes (SIGSEGV / SIGABRT etc.) — see ROADMAP
///     "explicitly out" list. Only Objective-C exceptions are caught here.
@objc public final class SentoriCrashHandler: NSObject {

    private static let configKey = "com.sentori.config"
    private static let pendingDirName = "sentori/pending"

    /// Install the global uncaught-exception handler. C function pointer,
    /// so we cannot capture local context (no chaining to a previously
    /// installed handler in v0.1 — RedBox in dev still receives via
    /// JS-side handlers; in release this just replaces the default).
    @objc public static func register() {
        NSSetUncaughtExceptionHandler(SentoriCrashHandler.exceptionHandler)
    }

    private static let exceptionHandler: @convention(c) (NSException) -> Void = { exception in
        SentoriCrashHandler.write(exception: exception)
    }

    /// JS side calls this on `sentori.init(...)` so the crash handler
    /// has release / environment when an exception fires later.
    @objc public static func setConfig(_ config: [String: Any]) {
        UserDefaults.standard.set(config, forKey: configKey)
    }

    /// Read all pending-crash files, return their contents (UTF-8 JSON
    /// strings), and remove them from disk. Best-effort: any I/O error
    /// drops that one file silently.
    @objc public static func consumePending() -> [String] {
        guard let dir = pendingDir() else { return [] }
        let urls = (try? FileManager.default.contentsOfDirectory(
            at: dir, includingPropertiesForKeys: nil)) ?? []
        var out: [String] = []
        for url in urls where url.pathExtension == "json" {
            if let data = try? Data(contentsOf: url),
               let str = String(data: data, encoding: .utf8) {
                out.append(str)
            }
            try? FileManager.default.removeItem(at: url)
        }
        return out
    }

    // MARK: - Internals

    private static func pendingDir() -> URL? {
        guard let docs = FileManager.default.urls(
            for: .documentDirectory, in: .userDomainMask).first else { return nil }
        let dir = docs.appendingPathComponent(pendingDirName)
        try? FileManager.default.createDirectory(
            at: dir, withIntermediateDirectories: true)
        return dir
    }

    private static func config() -> [String: Any] {
        return UserDefaults.standard.dictionary(forKey: configKey) ?? [:]
    }

    private static func write(exception: NSException) {
        let cfg = config()
        let release = (cfg["release"] as? String) ?? "unknown"
        let environment = (cfg["environment"] as? String) ?? "prod"

        var event: [String: Any] = [
            "id": UUID().uuidString.lowercased(),
            "timestamp": iso8601(Date()),
            "kind": "error",
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
            "tags": [String: String](),
            "breadcrumbs": [Any](),
            "error": [
                "type": exception.name.rawValue,
                "message": exception.reason ?? "",
                "stack": frames(from: exception.callStackSymbols),
                "cause": NSNull(),
            ],
            "fingerprint": [String](),
            "traceId": NSNull(),
            "spanId": NSNull(),
        ]

        // Phase 42 sub-E.04/07: capture the screen + view tree right
        // before the app dies. The handler runs synchronously on the
        // thread that threw NSException; UIKit's still valid at this
        // point. Both blobs go in a temp `_pendingAttachments` field
        // — JS strips it on next launch, uploads each via
        // `POST /v1/events/<id>/attachments/<kind>`, then enqueues
        // the cleaned event.
        if let snap = SentoriScreenshotCapture.captureKeyWindow() {
            var pending: [[String: Any]] = []
            if let sc = snap["screenshot"] as? [String: Any],
               let b64 = sc["base64"] as? String {
                pending.append([
                    "kind": "screenshot",
                    "base64": b64,
                    "mediaType": (sc["mediaType"] as? String) ?? "image/jpeg",
                    "source": "ios",
                ])
            }
            if let vt = snap["viewTree"],
               let data = try? JSONSerialization.data(withJSONObject: vt, options: []) {
                pending.append([
                    "kind": "viewTree",
                    "base64": data.base64EncodedString(),
                    "mediaType": "application/json",
                    "source": "ios",
                ])
            }
            if !pending.isEmpty {
                event["_pendingAttachments"] = pending
            }
        }

        guard let dir = pendingDir() else { return }
        let url = dir.appendingPathComponent("\(UUID().uuidString.lowercased()).json")
        if let data = try? JSONSerialization.data(withJSONObject: event, options: []) {
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
            "version": (info["CFBundleShortVersionString"] as? String) ?? "0.0.0",
        ]
        if let build = info["CFBundleVersion"] as? String {
            d["build"] = build
        }
        return d
    }

    /// Best-effort frame parse from `[NSException callStackSymbols]`.
    /// Each line looks roughly like:
    ///   "1   AppName    0x0001a0b0 -[ClassName method:] + 100"
    /// We don't have file/line info in raw symbol output; sourcemap-style
    /// symbolication for native happens server-side (Phase 8+).
    private static func frames(from symbols: [String]) -> [[String: Any]] {
        return symbols.map { sym -> [String: Any] in
            let parts = sym.split(separator: " ", omittingEmptySubsequences: true).map(String.init)
            let module = parts.count > 1 ? parts[1] : "<unknown>"
            let function = parts.count > 3 ? parts.dropFirst(3).joined(separator: " ") : "<anonymous>"
            let inApp = !module.contains("UIKit")
                && !module.contains("Foundation")
                && !module.contains("CoreFoundation")
                && !module.contains("libsystem")
                && !module.contains("libobjc")
            return [
                "function": function,
                "file": module,
                "line": 0,
                "inApp": inApp,
            ]
        }
    }
}
