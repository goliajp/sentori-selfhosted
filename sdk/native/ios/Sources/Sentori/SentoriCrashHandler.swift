import Foundation
import MachO

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

    /// Test-only seam onto `write`, which the real path reaches from
    /// an `@convention(c)` handler an XCTest cannot raise safely.
    ///
    /// The test that calls this was written against this name and has
    /// never compiled: the sources were loose files in an Expo module
    /// with no module to import, so nothing ever built the test target
    /// and nothing reported that the member was missing. Internal
    /// rather than public — `@testable import` reaches it, an app
    /// does not.
    internal static func persistForTesting(exception: NSException) {
        write(exception: exception)
    }

    /// Test-only seam that writes a crash file verbatim.
    ///
    /// `persistForTesting(exception:)` goes through `write`, which
    /// composes the file from a live exception and a live screen —
    /// neither of which an XCTest can arrange. Delivery tests need to
    /// state the file they are about to deliver, particularly the
    /// pre-death attachments, which only exist when there was a
    /// window to capture.
    internal static func persistRawForTesting(_ event: [String: Any]) {
        guard let dir = pendingDir(),
            let data = try? JSONSerialization.data(withJSONObject: event)
        else { return }
        try? data.write(to: dir.appendingPathComponent("\(UUID().uuidString.lowercased()).json"))
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
                "stack": frames(from: exception.callStackSymbols, addresses: exception.callStackReturnAddresses),
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
    private static func frames(
        from symbols: [String],
        addresses: [NSNumber] = []
    ) -> [[String: Any]] {
        return symbols.enumerated().map { (i, sym) -> [[String: Any]].Element in
            let parts = sym.split(separator: " ", omittingEmptySubsequences: true).map(String.init)
            let module = parts.count > 1 ? parts[1] : "<unknown>"
            let function = parts.count > 3 ? parts.dropFirst(3).joined(separator: " ") : "<anonymous>"
            let inApp = !module.contains("UIKit")
                && !module.contains("Foundation")
                && !module.contains("CoreFoundation")
                && !module.contains("libsystem")
                && !module.contains("libobjc")
            var frame: [String: Any] = [
                "function": function,
                "file": module,
                "line": 0,
                "inApp": inApp,
            ]
            // v5.1 — raw address + owning image (base, LC_UUID) so the
            // server can resolve the frame through the release's dSYM
            // slice. dladdr + load-command walk are async-signal-
            // unsafe in general but fine here: NSException handlers
            // run on a live runtime, same as the UIKit capture below.
            if i < addresses.count {
                let addr = addresses[i].uintValue
                var info = Dl_info()
                if dladdr(UnsafeRawPointer(bitPattern: addr), &info) != 0,
                   let fbase = info.dli_fbase {
                    frame["addr"] = UInt64(addr)
                    frame["imageBase"] = UInt64(UInt(bitPattern: fbase))
                    if let uuid = imageUuid(atBase: fbase) {
                        frame["imageUuid"] = uuid
                    }
                }
            }
            return frame
        }
    }

    /// LC_UUID of the Mach-O image loaded at `base` — the identity a
    /// dSYM slice is matched by. Walks the load commands off the
    /// in-memory header; read-only, bounded, no allocation beyond
    /// the hex string.
    private static func imageUuid(atBase base: UnsafeRawPointer) -> String? {
        let header = base.assumingMemoryBound(to: mach_header_64.self)
        guard header.pointee.magic == MH_MAGIC_64 else { return nil }
        var cursor = base.advanced(by: MemoryLayout<mach_header_64>.size)
        for _ in 0..<header.pointee.ncmds {
            let cmd = cursor.assumingMemoryBound(to: load_command.self)
            if cmd.pointee.cmd == LC_UUID {
                let uuidCmd = cursor.assumingMemoryBound(to: uuid_command.self)
                let u = uuidCmd.pointee.uuid
                let bytes = [u.0, u.1, u.2, u.3, u.4, u.5, u.6, u.7,
                             u.8, u.9, u.10, u.11, u.12, u.13, u.14, u.15]
                return bytes.map { String(format: "%02X", $0) }.joined()
            }
            cursor = cursor.advanced(by: Int(cmd.pointee.cmdsize))
        }
        return nil
    }
}
