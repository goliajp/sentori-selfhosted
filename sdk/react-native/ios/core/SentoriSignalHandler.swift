// GENERATED MIRROR — do not edit.
// Source of truth: sdk/native/ios/Sources/Sentori/SentoriSignalHandler.swift
// Run `node scripts/sync-native-core.mjs` after editing it.
import Darwin
import Foundation
import MachO

/// The crashes `NSSetUncaughtExceptionHandler` never sees.
///
/// [SentoriCrashHandler] catches an `NSException`, which is what
/// Objective-C throws. Almost nothing a Swift app dies of is one. A
/// force-unwrapped nil, an array index out of range, an overflowing
/// arithmetic operator — every one of those is a runtime trap, which
/// arrives as `EXC_BREAKPOINT` / `SIGTRAP`, and the app disappeared
/// with no report at all. A bad pointer is `SIGSEGV`, a `fatalError`
/// or a failed C assert is `SIGABRT`. These are the crashes a mobile
/// team actually has.
///
/// ## Why a signal handler and not a crash library
///
/// A host app may already have one. Whoever installs a handler last
/// wins, and a reporting SDK that takes the handler away from the
/// customer's own reporter has made their failure ours — the one
/// thing the zero-cost rule forbids. So this chains: the previous
/// handler is kept and called, and the signal is re-raised with the
/// default disposition so the OS still sees the death and writes its
/// own report.
///
/// ## Why a binary record
///
/// A handler runs on a process that is already dying, where `malloc`
/// may hold a lock the crashing thread took. Anything that allocates
/// can deadlock, and a crash reporter that hangs the process is worse
/// than one that misses the crash. So the handler does the smallest
/// possible thing — `backtrace` into a buffer reserved at install
/// time, `write` it to a path built at install time — and the next
/// launch, where allocation is safe again, turns it into an event.
enum SentoriSignalHandler {
    /// The six a mobile app dies of. `SIGPIPE` is deliberately absent:
    /// a socket closing is not a crash, and handling it would report
    /// a network blip as one.
    static let handled: [Int32] = [SIGSEGV, SIGBUS, SIGILL, SIGFPE, SIGTRAP, SIGABRT]

    /// Frames kept. Deeper than the verb's limit because a crash is
    /// the one place the whole stack is worth the bytes, and the
    /// buffer is reserved up front either way.
    static let maxFrames = 128

    private static var registered = false
    private static var previous: [Int32: sigaction] = [:]

    /// Test-only. The handler's last act is to restore the default
    /// disposition and re-raise, which is correct and kills the
    /// process — so nothing that runs inside a test can observe what
    /// the handler did. With this set, a test can deliver a real
    /// signal through the kernel, let the whole handler run, and read
    /// the record back. Nothing else changes.
    static var __suppressReRaiseForTests = false

    /// Reserved at install time. Touching the allocator inside the
    /// handler is the thing this file exists to avoid.
    private static var frames = UnsafeMutablePointer<UnsafeMutableRawPointer?>
        .allocate(capacity: maxFrames)
    private static var pathBuffer = UnsafeMutablePointer<CChar>.allocate(capacity: 1024)
    private static var altStack = UnsafeMutableRawPointer.allocate(
        byteCount: Int(SIGSTKSZ), alignment: 16
    )

    /// Where the record and the image map live.
    ///
    /// Not the pending directory. `SentoriCrashHandler.consumePending`
    /// reads and **deletes every `.json`** in there — that is its
    /// contract, and it is the right one for a directory of crashes
    /// waiting to be sent. The image map was written into it and was
    /// deleted on the same launch that wrote it, so the next launch
    /// had a crash record and nothing to attribute its addresses
    /// with, and every frame arrived without the image identity a
    /// dSYM is matched by.
    static func signalDirectory(besidePending pendingDirectory: URL) -> URL {
        let dir = pendingDirectory.deletingLastPathComponent().appendingPathComponent("signal")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    static func register(pendingDirectory: URL) {
        guard !registered else { return }
        let own = signalDirectory(besidePending: pendingDirectory)

        // Written now, while allocating is safe. The addresses in a
        // crash record are absolute addresses in a process that no
        // longer exists: after a relaunch ASLR has moved everything,
        // so `dladdr` in the new process would answer confidently
        // about the wrong image. What survives the restart is the
        // offset within an image plus that image's UUID — which is
        // exactly what a dSYM is indexed by — and computing it needs
        // the map this process was using when it died.
        writeImageMap(to: own)

        let path = own.appendingPathComponent("signal.sentoricrash").path
        guard path.utf8.count < 1024 else { return }
        path.withCString { source in
            _ = strlcpy(pathBuffer, source, 1024)
        }

        // A stack overflow is a SIGSEGV whose handler cannot run on
        // the stack that just overflowed. Without this the most
        // common infinite-recursion crash is the one crash we never
        // record.
        var stack = stack_t(ss_sp: altStack, ss_size: Int(SIGSTKSZ), ss_flags: 0)
        sigaltstack(&stack, nil)

        for signal in handled {
            var action = sigaction()
            action.__sigaction_u.__sa_sigaction = { signalNumber, info, context in
                SentoriSignalHandler.onSignal(signalNumber, info, context)
            }
            action.sa_flags = SA_SIGINFO | SA_ONSTACK
            sigemptyset(&action.sa_mask)
            var old = sigaction()
            if sigaction(signal, &action, &old) == 0 {
                previous[signal] = old
            }
        }
        registered = true
    }

    /// Async-signal-safe: `backtrace`, `open`, `write`, `close`. No
    /// allocation, no Objective-C, no Foundation.
    private static func onSignal(
        _ signalNumber: Int32,
        _ info: UnsafeMutablePointer<siginfo_t>?,
        _ context: UnsafeMutableRawPointer?
    ) {
        let count = backtrace(frames, Int32(maxFrames))
        if count > 0 {
            writeRecord(signal: signalNumber, frames: frames, count: Int(count), to: pathBuffer)
        }
        chainAndReRaise(signalNumber, info, context)
    }

    /// The whole of what the handler does to disk, so a test can run
    /// it. The real path reaches it from an `@convention(c)` handler
    /// no XCTest can raise without taking the test process with it.
    static func writeRecord(
        signal: Int32,
        frames: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
        count: Int,
        to path: UnsafePointer<CChar>
    ) {
        let fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0o600)
        guard fd >= 0 else { return }
        var header = Record(signal: signal, frameCount: UInt32(count))
        withUnsafeBytes(of: &header) { bytes in
            _ = write(fd, bytes.baseAddress, bytes.count)
        }
        _ = write(fd, frames, count * MemoryLayout<UInt>.size)
        close(fd)
    }

    /// Give the host's own handler its turn, then die the way we
    /// would have without us. Restoring the default first means the
    /// re-raise is not caught by this handler again.
    private static func chainAndReRaise(
        _ signalNumber: Int32,
        _ info: UnsafeMutablePointer<siginfo_t>?,
        _ context: UnsafeMutableRawPointer?
    ) {
        callPrevious(signalNumber, info, context)
        var reset = sigaction()
        reset.__sigaction_u.__sa_handler = unsafeBitCast(SIG_DFL, to: sig_t.self)
        sigemptyset(&reset.sa_mask)
        reset.sa_flags = 0
        if __suppressReRaiseForTests { return }
        sigaction(signalNumber, &reset, nil)
        raise(signalNumber)
    }

    /// Whatever the host installed before us gets its turn. Separated
    /// so a test can prove the host's handler still runs — a
    /// reporting SDK that silently replaced the customer's own crash
    /// reporter would be the failure-contagion the zero-cost rule
    /// exists to forbid, and it would look like nothing at all.
    /// `info` and `context` are the ones the kernel handed us, passed
    /// through untouched.
    ///
    /// They used to be `nil`. Every serious crash reporter installs an
    /// `SA_SIGINFO` handler and reads the `siginfo_t` — `si_addr` is
    /// the faulting address, which is most of what a segfault report
    /// is. Handing it nil is a null dereference *inside the host's
    /// crash handler, during a crash*: their reporter would die where
    /// it was supposed to record, and the only visible symptom would
    /// be crashes that stopped being reported after they installed us.
    static func callPrevious(
        _ signalNumber: Int32,
        _ info: UnsafeMutablePointer<siginfo_t>? = nil,
        _ context: UnsafeMutableRawPointer? = nil
    ) {
        if var old = previous[signalNumber] {
            let flags = Int32(old.sa_flags)
            if flags & SA_SIGINFO != 0 {
                old.__sigaction_u.__sa_sigaction?(signalNumber, info, context)
            } else if let handler = old.__sigaction_u.__sa_handler {
                // SIG_DFL and SIG_IGN are sentinel values cast to a
                // function pointer, not functions — calling either
                // would jump to address 0 or 1. Function pointers do
                // not compare in Swift, so the comparison is on the
                // raw bits.
                let bits = unsafeBitCast(handler, to: UInt.self)
                let ignore = unsafeBitCast(SIG_IGN, to: UInt.self)
                let dfl = unsafeBitCast(SIG_DFL, to: UInt.self)
                if bits != ignore, bits != dfl { handler(signalNumber) }
            }
        }
    }

    /// Fixed layout, written and read by this file only. A version
    /// byte because a record outlives the launch that wrote it: an
    /// app that crashes and is then updated hands the new binary a
    /// file the old one wrote.
    struct Record {
        static let magic: UInt32 = 0x534E_5452  // "SNTR"
        static let version: UInt32 = 1

        var magicValue: UInt32 = Record.magic
        var versionValue: UInt32 = Record.version
        var signal: Int32
        var frameCount: UInt32
    }

    /// Every Mach-O image this process has loaded: where it starts,
    /// where it ends, and the UUID a dSYM slice is stored under.
    struct LoadedImage: Codable {
        let base: UInt64
        let end: UInt64
        let uuid: String
        let name: String
    }

    static func imageMap() -> [LoadedImage] {
        var images: [LoadedImage] = []
        for index in 0..<_dyld_image_count() {
            guard let header = _dyld_get_image_header(index) else { continue }
            let base = UInt(bitPattern: header)
            let raw = UnsafeRawPointer(header)
            guard let uuid = SentoriImage.uuid(atBase: raw) else { continue }
            let name = String(cString: _dyld_get_image_name(index))
            images.append(
                LoadedImage(
                    base: UInt64(base),
                    end: UInt64(base) + UInt64(imageSize(raw)),
                    uuid: uuid,
                    name: (name as NSString).lastPathComponent
                )
            )
        }
        return images.sorted { $0.base < $1.base }
    }

    /// The `char segname[16]` of a segment command, as Swift sees it.
    private typealias SegmentName = (
        CChar, CChar, CChar, CChar, CChar, CChar, CChar, CChar,
        CChar, CChar, CChar, CChar, CChar, CChar, CChar, CChar
    )

    /// How far an image reaches, so an address lands in the image it
    /// belongs to rather than in whichever one happens to start below
    /// it.
    ///
    /// A segment's `vmaddr` is where the linker wanted it, not where
    /// it is — the header's address is the runtime one, and the two
    /// differ by the slide. So the span is measured between segments
    /// (highest end minus lowest start) and applied to the runtime
    /// base, rather than taking `vmaddr + vmsize` as an address.
    private static func imageSize(_ base: UnsafeRawPointer) -> UInt {
        let header = base.assumingMemoryBound(to: mach_header_64.self)
        guard header.pointee.magic == MH_MAGIC_64 else { return 0 }
        var cursor = base.advanced(by: MemoryLayout<mach_header_64>.size)
        var low = UInt64.max
        var high: UInt64 = 0
        for _ in 0..<header.pointee.ncmds {
            let command = cursor.assumingMemoryBound(to: load_command.self).pointee
            if command.cmd == LC_SEGMENT_64 {
                let segment = cursor.assumingMemoryBound(to: segment_command_64.self).pointee
                // __PAGEZERO is a 4 GB hole at address 0 that every
                // main executable declares and nothing occupies.
                // Counting it made the executable's span four
                // gigabytes, so it contained every other image and
                // every frame was attributed to it.
                if segment.vmsize > 0, !isPageZero(segment.segname) {
                    low = min(low, segment.vmaddr)
                    high = max(high, segment.vmaddr &+ segment.vmsize)
                }
            }
            cursor = cursor.advanced(by: Int(command.cmdsize))
        }
        guard low != UInt64.max, high > low else { return 0 }
        return UInt(high - low)
    }

    /// `segname` is a fixed 16-byte tuple, not a string.
    private static func isPageZero(_ segname: SegmentName) -> Bool {
        withUnsafeBytes(of: segname) { raw in
            let name = raw.prefix(while: { $0 != 0 })
            return name.elementsEqual("__PAGEZERO".utf8)
        }
    }

    private static func writeImageMap(to directory: URL) {
        let url = directory.appendingPathComponent("signal.images.json")
        if let data = try? JSONEncoder().encode(imageMap()) {
            try? data.write(to: url)
        }
    }

    /// An absolute address from the dead process, placed in the image
    /// it belonged to. `nil` when nothing owns it, which is the
    /// honest answer — a frame attributed to the wrong image
    /// symbolicates to a real-looking line in unrelated code.
    static func attribute(_ address: UInt64, in images: [LoadedImage]) -> LoadedImage? {
        images.last { address >= $0.base && address < $0.end }
    }

    /// The record the last launch left, as addresses. `nil` when there
    /// is none, or when the file is not one of ours.
    static func decode(_ data: Data) -> (signal: Int32, addresses: [NSNumber])? {
        let headerSize = MemoryLayout<Record>.size
        guard data.count >= headerSize else { return nil }
        let header = data.withUnsafeBytes { $0.loadUnaligned(as: Record.self) }
        guard header.magicValue == Record.magic, header.versionValue == Record.version,
            header.frameCount > 0, header.frameCount <= UInt32(maxFrames)
        else { return nil }
        let wordSize = MemoryLayout<UInt>.size
        let wanted = Int(header.frameCount) * wordSize
        guard data.count >= headerSize + wanted else { return nil }
        var addresses: [NSNumber] = []
        addresses.reserveCapacity(Int(header.frameCount))
        for index in 0..<Int(header.frameCount) {
            let offset = headerSize + index * wordSize
            let word = data.withUnsafeBytes { raw -> UInt in
                raw.loadUnaligned(fromByteOffset: offset, as: UInt.self)
            }
            addresses.append(NSNumber(value: UInt64(word)))
        }
        return (header.signal, addresses)
    }

    /// Turn the last launch's record into the JSON the pending-crash
    /// shipper already knows how to send, and remove the record.
    ///
    /// Runs at `start()`, where allocating and symbolicating are
    /// safe. Removing before writing: a record that cannot be turned
    /// into an event must not be retried every launch forever.
    static func drain(pendingDirectory: URL, config: [String: String]) {
        let own = signalDirectory(besidePending: pendingDirectory)
        let url = own.appendingPathComponent("signal.sentoricrash")
        guard let data = try? Data(contentsOf: url) else { return }
        try? FileManager.default.removeItem(at: url)
        guard let record = decode(data) else { return }

        let mapURL = own.appendingPathComponent("signal.images.json")
        let images =
            (try? Data(contentsOf: mapURL))
            .flatMap { try? JSONDecoder().decode([LoadedImage].self, from: $0) } ?? []

        let event = crashEvent(
            signal: record.signal,
            addresses: record.addresses,
            images: images,
            release: config["release"] ?? "",
            environment: config["environment"] ?? ""
        )
        let out = pendingDirectory.appendingPathComponent("\(UUID().uuidString.lowercased()).json")
        if let json = try? JSONSerialization.data(withJSONObject: event, options: []) {
            try? json.write(to: out)
        }
    }

    /// The shape `SentoriPendingCrash.toWire` reads.
    static func crashEvent(
        signal: Int32,
        addresses: [NSNumber],
        images: [LoadedImage],
        release: String,
        environment: String
    ) -> [String: Any] {
        [
            "id": Sentori.newEventId(),
            "kind": "error",
            "timestamp": Sentori.iso8601(Date()),
            "platform": "ios",
            "release": release,
            "environment": environment,
            "error": [
                "type": name(of: signal),
                "message": message(for: signal),
                "stack": frames(for: addresses, in: images),
            ],
        ]
    }

    /// Frames the server can symbolicate: an image UUID and an offset
    /// within it. No function name — `dladdr` cannot answer for a
    /// process that is gone, and a name it invented would be worse
    /// than the blank the dSYM fills in.
    static func frames(for addresses: [NSNumber], in images: [LoadedImage]) -> [[String: Any]] {
        addresses.map { boxed in
            let address = boxed.uint64Value
            var frame: [String: Any] = [
                "function": "<unresolved>",
                "file": "<unknown>",
                "line": 0,
                "inApp": true,
                "addr": address,
            ]
            guard let image = attribute(address, in: images) else { return frame }
            frame["file"] = image.name
            frame["imageBase"] = image.base
            frame["imageUuid"] = image.uuid
            frame["inApp"] = SentoriStack.isApp(image.name)
            return frame
        }
    }

    /// The name a dashboard groups on, so two force-unwraps in the
    /// same place are one issue.
    static func name(of signal: Int32) -> String {
        switch signal {
        case SIGSEGV: return "SIGSEGV"
        case SIGBUS: return "SIGBUS"
        case SIGILL: return "SIGILL"
        case SIGFPE: return "SIGFPE"
        case SIGTRAP: return "SIGTRAP"
        case SIGABRT: return "SIGABRT"
        default: return "SIG\(signal)"
        }
    }

    /// Said in the terms the person reading it thinks in. "SIGTRAP"
    /// means nothing to an app developer; "a Swift runtime check
    /// failed" is the sentence that starts the investigation.
    static func message(for signal: Int32) -> String {
        switch signal {
        case SIGTRAP:
            return "a Swift runtime check failed — a force-unwrapped nil, an index out of range, or an overflowing operation"
        case SIGSEGV: return "bad memory access"
        case SIGBUS: return "misaligned or unmapped memory access"
        case SIGILL: return "illegal instruction"
        case SIGFPE: return "arithmetic fault"
        case SIGABRT: return "the process aborted — a fatalError, a precondition, or a failed C assert"
        default: return "the process was killed by signal \(signal)"
        }
    }

    /// Install for one signal only, so a test can prove the chaining
    /// without arming the six that would take the test process down.
    static func __installForTests(_ signalNumber: Int32, pendingDirectory: URL) {
        let path = signalDirectory(besidePending: pendingDirectory)
            .appendingPathComponent("signal.sentoricrash").path
        path.withCString { _ = strlcpy(pathBuffer, $0, 1024) }
        var action = sigaction()
        action.__sigaction_u.__sa_sigaction = { number, info, context in
            SentoriSignalHandler.onSignal(number, info, context)
        }
        action.sa_flags = SA_SIGINFO | SA_ONSTACK
        sigemptyset(&action.sa_mask)
        var old = sigaction()
        if sigaction(signalNumber, &action, &old) == 0 {
            previous[signalNumber] = old
        }
    }

    static func __writeImageMapForTests(to directory: URL) { writeImageMap(to: directory) }

    static func __resetForTests() {
        for (signal, var old) in previous { sigaction(signal, &old, nil) }
        previous = [:]
        registered = false
        __suppressReRaiseForTests = false
    }
}
