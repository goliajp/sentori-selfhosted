import Foundation
import MachO

/// Where a call came from.
///
/// `Sentori.error()` used to send an error with no stack at all: a
/// type, a message, and nothing saying where. On the dashboard that
/// is a case you cannot open — the panel that is the point of the
/// issue page has nothing to draw. The crash handler has had frames
/// since v5.1; a reported error had not.
///
/// Split in two on purpose. Capturing return addresses is a walk of
/// the frame pointers and costs microseconds; turning them into
/// symbols is `dladdr` per frame plus a load-command walk per image,
/// which is the part worth keeping off a verb the host calls from a
/// tap handler. The verbs capture; the transport resolves.
enum SentoriStack {
    /// How deep we look. A stack this product can act on is the top of
    /// it — the frames below are the runtime's own scaffolding, and a
    /// bounded number keeps the cost bounded too (footprint, dim 4).
    static let maxDepth = 40

    /// Where a verb parks raw addresses for the worker to resolve.
    /// Private to the SDK and removed before the event goes on the
    /// wire — the server has never heard of it.
    static let pendingKey = "_sentoriStackAddresses"

    /// Replace parked addresses with resolved frames, wherever they
    /// are in an event. Called on the transport's worker: this is the
    /// expensive half, and it has no business on the thread the host
    /// called a verb from.
    static func resolvePending(in event: [String: Any]) -> [String: Any] {
        guard var error = event["payload"] as? [String: Any] else { return event }
        guard var inner = error["error"] as? [String: Any] else { return event }
        guard let parked = inner[pendingKey] as? [NSNumber] else { return event }
        inner.removeValue(forKey: pendingKey)
        inner["stack"] = resolve(parked)
        error["error"] = inner
        var out = event
        out["payload"] = error
        return out
    }

    /// Return addresses for the calling thread, ours dropped.
    ///
    /// `skip` is how many Sentori frames sit between the host's call
    /// site and here; getting it wrong costs a frame of context, never
    /// correctness.
    static func capture(skip: Int) -> [NSNumber] {
        let raw = Thread.callStackReturnAddresses
        guard raw.count > skip else { return [] }
        return Array(raw.dropFirst(skip).prefix(maxDepth))
    }

    /// Addresses → wire frames, with the image identity the server
    /// needs to reach the release's dSYM.
    ///
    /// `line: 0` rather than a guess: there is no line number on this
    /// side, symbolication happens on the server, and inventing one
    /// would put a number on the screen that means nothing.
    static func resolve(_ addresses: [NSNumber]) -> [[String: Any]] {
        addresses.map { boxed in
            let addr = boxed.uintValue
            var frame: [String: Any] = [
                "function": "<unresolved>",
                "file": "<unknown>",
                "line": 0,
                "inApp": true,
            ]
            var info = Dl_info()
            guard dladdr(UnsafeRawPointer(bitPattern: addr), &info) != 0 else { return frame }
            if let name = info.dli_sname {
                frame["function"] = demangle(String(cString: name))
            }
            if let path = info.dli_fname {
                let image = (String(cString: path) as NSString).lastPathComponent
                frame["file"] = image
                frame["inApp"] = isApp(image)
            }
            frame["addr"] = UInt64(addr)
            if let fbase = info.dli_fbase {
                frame["imageBase"] = UInt64(UInt(bitPattern: fbase))
                if let uuid = SentoriImage.uuid(atBase: fbase) {
                    frame["imageUuid"] = uuid
                }
            }
            return frame
        }
    }

    /// A frame belongs to the host app unless it is plainly the
    /// platform's. Getting this wrong only changes which frames the
    /// dashboard folds away by default, so the test is deliberately
    /// crude rather than a list that goes stale.
    static func isApp(_ image: String) -> Bool {
        let system = ["UIKit", "Foundation", "CoreFoundation", "libsystem", "libobjc",
                      "libdispatch", "libswiftCore", "SwiftUI", "QuartzCore", "GraphicsServices"]
        return !system.contains { image.hasPrefix($0) }
    }

    /// Swift mangled names are unreadable on a dashboard and the
    /// runtime can undo them. Objective-C selectors are already
    /// readable and come back unchanged.
    static func demangle(_ symbol: String) -> String {
        guard symbol.hasPrefix("$s") || symbol.hasPrefix("_$s") else { return symbol }
        var length = 0
        guard let out = swift_demangle(symbol, symbol.utf8.count, nil, &length, 0) else {
            return symbol
        }
        defer { free(out) }
        return String(cString: out)
    }
}

/// `swift_demangle` ships in the Swift runtime but has no header.
/// Declared here rather than reached for through a bridging header,
/// which a Swift package has no place to put.
@_silgen_name("swift_demangle")
private func swift_demangle(
    _ mangledName: UnsafePointer<CChar>?,
    _ mangledNameLength: Int,
    _ outputBuffer: UnsafeMutablePointer<CChar>?,
    _ outputBufferSize: UnsafeMutablePointer<Int>?,
    _ flags: UInt32
) -> UnsafeMutablePointer<CChar>?

/// LC_UUID of a loaded Mach-O image — the identity a dSYM slice is
/// matched by. Read-only walk of the in-memory load commands.
enum SentoriImage {
    static func uuid(atBase base: UnsafeRawPointer) -> String? {
        let header = base.assumingMemoryBound(to: mach_header_64.self)
        guard header.pointee.magic == MH_MAGIC_64 else { return nil }
        var cursor = base.advanced(by: MemoryLayout<mach_header_64>.size)
        for _ in 0..<header.pointee.ncmds {
            let command = cursor.assumingMemoryBound(to: load_command.self).pointee
            if command.cmd == LC_UUID {
                let uuidCommand = cursor.assumingMemoryBound(to: uuid_command.self).pointee
                return hex(uuidCommand.uuid)
            }
            cursor = cursor.advanced(by: Int(command.cmdsize))
        }
        return nil
    }

    /// Lowercase, no dashes — the form the CLI stores a dSYM slice
    /// under, so the two sides compare as written.
    private static func hex(_ uuid: uuid_t) -> String {
        let bytes = [uuid.0, uuid.1, uuid.2, uuid.3, uuid.4, uuid.5, uuid.6, uuid.7,
                     uuid.8, uuid.9, uuid.10, uuid.11, uuid.12, uuid.13, uuid.14, uuid.15]
        return bytes.map { String(format: "%02x", $0) }.joined()
    }
}
