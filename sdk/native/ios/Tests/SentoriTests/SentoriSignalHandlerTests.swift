import Darwin
import XCTest

@testable import Sentori

/// A C handler needs a C-visible place to record that it ran; a
/// closure that captures cannot be one.
private var hostHandlerCalls = 0
private var hostHandlerSawInfo = false
private var hostHandlerSawSignal: Int32 = 0

final class SentoriSignalHandlerTests: XCTestCase {
    private var dir: URL!
    private var own: URL!

    override func setUp() {
        super.setUp()
        // The handler keeps its record and image map in a directory
        // of its own beside `pending`, because `consumePending`
        // deletes every `.json` it finds. `dir` here plays the part
        // of `pending`; `own` is where the handler actually writes.
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("sentori-signal-\(UUID().uuidString)")
        dir = root.appendingPathComponent("pending")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        own = SentoriSignalHandler.signalDirectory(besidePending: dir)
        SentoriSignalHandler.__resetForTests()
        hostHandlerCalls = 0
        hostHandlerSawInfo = false
        hostHandlerSawSignal = 0
    }

    override func tearDown() {
        SentoriSignalHandler.__resetForTests()
        try? FileManager.default.removeItem(at: dir)
        super.tearDown()
    }

    private func writeSyntheticRecord(signal: Int32, frameCount: Int) -> URL {
        let url = own.appendingPathComponent("signal.sentoricrash")
        let buffer = UnsafeMutablePointer<UnsafeMutableRawPointer?>.allocate(
            capacity: SentoriSignalHandler.maxFrames
        )
        defer { buffer.deallocate() }
        let real = backtrace(buffer, Int32(frameCount))
        url.path.withCString { path in
            SentoriSignalHandler.writeRecord(
                signal: signal, frames: buffer, count: Int(real), to: path
            )
        }
        return url
    }

    func testWhatTheHandlerWritesIsWhatTheNextLaunchReads() {
        // The two halves run in different processes and never meet,
        // so nothing but this says the layout agrees with itself.
        let url = writeSyntheticRecord(signal: SIGTRAP, frameCount: 12)
        let data = try! Data(contentsOf: url)
        guard let record = SentoriSignalHandler.decode(data) else {
            return XCTFail("the record this process just wrote does not decode")
        }
        XCTAssertEqual(record.signal, SIGTRAP)
        XCTAssertFalse(record.addresses.isEmpty)
        // The addresses are real: they resolve to this test.
        let functions = SentoriStack.resolve(record.addresses)
            .compactMap { $0["function"] as? String }
        XCTAssertTrue(
            functions.contains { $0.contains("testWhatTheHandlerWritesIsWhatTheNextLaunchReads") },
            "resolved to \(functions.prefix(6))"
        )
    }

    func testAFileThatIsNotOursIsRefused() {
        // The path is in the app's own documents directory, which a
        // host app writes to as well. Anything there that happens to
        // have our name must not be read as a crash.
        XCTAssertNil(SentoriSignalHandler.decode(Data()))
        XCTAssertNil(SentoriSignalHandler.decode(Data(repeating: 0, count: 64)))
        XCTAssertNil(SentoriSignalHandler.decode(Data("not a crash record at all".utf8)))
    }

    func testATruncatedRecordIsRefusedRatherThanReadPastItsEnd() {
        let url = writeSyntheticRecord(signal: SIGSEGV, frameCount: 20)
        let full = try! Data(contentsOf: url)
        XCTAssertNotNil(SentoriSignalHandler.decode(full))
        // A process killed mid-write leaves exactly this.
        for cut in [1, 8, 17, full.count - 1] where cut < full.count {
            XCTAssertNil(
                SentoriSignalHandler.decode(full.prefix(cut)),
                "a record cut to \(cut) bytes was accepted"
            )
        }
    }

    func testDrainProducesAnEventTheShipperCanSend() {
        // `register` writes the image map, which `drain` needs to
        // attribute the addresses; the test path installs one signal
        // rather than six.
        SentoriSignalHandler.__writeImageMapForTests(to: own)
        _ = writeSyntheticRecord(signal: SIGTRAP, frameCount: 12)
        SentoriSignalHandler.drain(
            pendingDirectory: dir, config: ["release": "app@1.2.3+4", "environment": "test"]
        )
        let files = try! FileManager.default.contentsOfDirectory(atPath: dir.path)
        // The image map is also .json and is rewritten at every
        // register, so it is not one of the events.
        // The map must not be here: `consumePending` deletes every
        // `.json` in this directory, and it deleted the image map on
        // the launch that wrote it — so the next launch had a crash
        // record and no way to attribute its addresses.
        XCTAssertFalse(files.contains("signal.images.json"))
        let events = files.filter { $0.hasSuffix(".json") }
        XCTAssertEqual(events.count, 1)
        XCTAssertFalse(
            files.contains("signal.sentoricrash"),
            "the record must go, or every launch reports the same crash again"
        )

        let json = try! Data(contentsOf: dir.appendingPathComponent(events[0]))
        let raw = try! JSONSerialization.jsonObject(with: json) as! [String: Any]
        let wire = SentoriPendingCrash.toWire(raw)
        XCTAssertEqual(wire["kind"] as? String, "error")
        XCTAssertEqual(wire["platform"] as? String, "ios")
        XCTAssertEqual(wire["release"] as? String, "app@1.2.3+4")
        XCTAssertNotNil(wire["occurredAt"] as? String)
        let payload = wire["payload"] as! [String: Any]
        let error = payload["error"] as! [String: Any]
        XCTAssertEqual(error["type"] as? String, "SIGTRAP")
        XCTAssertFalse((error["stack"] as! [[String: Any]]).isEmpty)
    }

    func testAFrameCarriesAnImageIdentityThatOutlivesTheProcess() {
        // The addresses in a record belong to a process that no
        // longer exists: after a relaunch ASLR has moved everything,
        // and `dladdr` in the new process would answer confidently
        // about the wrong image. What survives is the UUID plus the
        // offset — which is what a dSYM is indexed by.
        let images = SentoriSignalHandler.imageMap()
        XCTAssertFalse(images.isEmpty, "this process has loaded no Mach-O images?")
        XCTAssertTrue(images.allSatisfy { $0.end > $0.base })
        XCTAssertTrue(images.allSatisfy { $0.uuid.count == 32 })

        let here = UInt64(UInt(bitPattern: #dsohandle))
        guard let owner = SentoriSignalHandler.attribute(here, in: images) else {
            return XCTFail("the image this test is in owns none of its own addresses")
        }
        XCTAssertEqual(owner.base, here)

        // No image may span the whole address space. The first
        // version of the span computation counted __PAGEZERO — a
        // 4 GB hole at address 0 that every main executable declares
        // and nothing occupies — so the executable contained every
        // other image and every frame was attributed to it.
        for image in images {
            XCTAssertLessThan(
                image.end - image.base, 1 << 32,
                "\(image.name) claims \(image.end - image.base) bytes"
            )
        }

        let frames = SentoriSignalHandler.frames(for: [NSNumber(value: here)], in: images)
        XCTAssertEqual(frames[0]["imageUuid"] as? String, owner.uuid)
        XCTAssertEqual(frames[0]["imageBase"] as? UInt64, owner.base)
        XCTAssertEqual(frames[0]["addr"] as? UInt64, here)
        // No invented name: the dSYM fills this in, and a plausible
        // wrong one would be believed.
        XCTAssertEqual(frames[0]["function"] as? String, "<unresolved>")
    }

    func testTheInnermostImageWins() {
        // dyld really does nest images, so overlap is not a bug to
        // rule out — it is a case to decide. The narrower, later one
        // is the answer: a framework loaded inside an executable's
        // span is where the frame actually is.
        let outer = SentoriSignalHandler.LoadedImage(
            base: 0x1000, end: 0x9000, uuid: String(repeating: "a", count: 32), name: "outer"
        )
        let inner = SentoriSignalHandler.LoadedImage(
            base: 0x4000, end: 0x5000, uuid: String(repeating: "b", count: 32), name: "inner"
        )
        let images = [outer, inner]
        XCTAssertEqual(SentoriSignalHandler.attribute(0x4500, in: images)?.name, "inner")
        XCTAssertEqual(SentoriSignalHandler.attribute(0x2000, in: images)?.name, "outer")
        XCTAssertNil(SentoriSignalHandler.attribute(0x9000, in: images))
    }

    func testAnAddressNoImageOwnsIsNotAttributedToOne() {
        let images = SentoriSignalHandler.imageMap()
        let frames = SentoriSignalHandler.frames(for: [NSNumber(value: UInt64(1))], in: images)
        XCTAssertNil(frames[0]["imageUuid"])
    }

    func testARecordThatCannotBeReadIsStillRemoved() {
        // Otherwise a corrupt file is retried at every launch for the
        // life of the install.
        let url = own.appendingPathComponent("signal.sentoricrash")
        try! Data("garbage".utf8).write(to: url)
        SentoriSignalHandler.drain(pendingDirectory: dir, config: [:])
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
    }

    func testTheHostsOwnHandlerStillRuns() {
        // A reporting SDK that quietly replaced the customer's crash
        // reporter would look like nothing at all until the day they
        // needed it.
        var host = sigaction()
        host.__sigaction_u.__sa_handler = { _ in hostHandlerCalls += 1 }
        sigemptyset(&host.sa_mask)
        host.sa_flags = 0
        sigaction(SIGUSR2, &host, nil)

        SentoriSignalHandler.__installForTests(SIGUSR2, pendingDirectory: dir)
        SentoriSignalHandler.callPrevious(SIGUSR2)
        XCTAssertEqual(hostHandlerCalls, 1)

        signal(SIGUSR2, SIG_DFL)
    }

    /// The whole handler, driven by the kernel.
    ///
    /// Every other test here exercises a piece: `writeRecord`,
    /// `decode`, `callPrevious`, the image map. None of them proves
    /// that `sigaction` installed anything, that the kernel calls what
    /// we registered, or that the pieces compose. The handler's last
    /// act is to re-raise and die, which is why this was never
    /// observed — so the re-raise is suppressed, and everything before
    /// it runs exactly as it would in a real crash.
    func testARealSignalReachesTheHandlerAndLeavesARecord() throws {
        SentoriSignalHandler.__writeImageMapForTests(to: own)
        SentoriSignalHandler.__installForTests(SIGUSR2, pendingDirectory: dir)
        SentoriSignalHandler.__suppressReRaiseForTests = true

        // Delivered by the kernel, not called by us.
        raise(SIGUSR2)

        let url = own.appendingPathComponent("signal.sentoricrash")
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: url.path),
            "a signal was delivered and the handler wrote nothing"
        )
        let record = try XCTUnwrap(SentoriSignalHandler.decode(Data(contentsOf: url)))
        XCTAssertEqual(record.signal, SIGUSR2)
        XCTAssertFalse(record.addresses.isEmpty)

        // And the next launch turns it into the event the shipper sends.
        SentoriSignalHandler.drain(
            pendingDirectory: dir, config: ["release": "app@1.0.0+1", "environment": "test"]
        )
        let files = try FileManager.default.contentsOfDirectory(atPath: dir.path)
        // The map must not be here: `consumePending` deletes every
        // `.json` in this directory, and it deleted the image map on
        // the launch that wrote it — so the next launch had a crash
        // record and no way to attribute its addresses.
        XCTAssertFalse(files.contains("signal.images.json"))
        let events = files.filter { $0.hasSuffix(".json") }
        XCTAssertEqual(events.count, 1, "the record did not become an event")
        let raw = try JSONSerialization.jsonObject(
            with: Data(contentsOf: dir.appendingPathComponent(events[0]))
        ) as! [String: Any]
        let wire = SentoriPendingCrash.toWire(raw)
        let payload = wire["payload"] as! [String: Any]
        let error = payload["error"] as! [String: Any]
        XCTAssertFalse((error["stack"] as! [[String: Any]]).isEmpty)
        // The frames carry what the server symbolicates from.
        let frames = error["stack"] as! [[String: Any]]
        // Without these the server has nothing to match a dSYM
        // against, and a native crash stays a column of hex forever.
        XCTAssertTrue(
            frames.contains { $0["imageUuid"] != nil },
            "no frame carried an image identity, so none of this can be symbolicated"
        )
        XCTAssertTrue(frames.contains { $0["addr"] != nil })
        XCTAssertTrue(frames.contains { $0["imageBase"] != nil })
    }

    func testTheHostsHandlerGetsTheRealSiginfo() {
        // Every serious crash reporter installs an SA_SIGINFO handler
        // and reads the `siginfo_t` — `si_addr` is the faulting
        // address, which is most of what a segfault report is. This
        // passed `nil` until it was caught, which is a null
        // dereference inside the customer's crash handler, during a
        // crash: their reporter would die where it was meant to
        // record, and the only symptom would be crashes quietly
        // ceasing to be reported after they installed us.
        var host = sigaction()
        host.__sigaction_u.__sa_sigaction = { number, info, _ in
            hostHandlerSawSignal = number
            hostHandlerSawInfo = info != nil
        }
        sigemptyset(&host.sa_mask)
        host.sa_flags = SA_SIGINFO
        sigaction(SIGUSR2, &host, nil)

        SentoriSignalHandler.__installForTests(SIGUSR2, pendingDirectory: dir)

        var info = siginfo_t()
        info.si_signo = SIGUSR2
        withUnsafeMutablePointer(to: &info) { pointer in
            SentoriSignalHandler.callPrevious(SIGUSR2, pointer, nil)
        }
        XCTAssertEqual(hostHandlerSawSignal, SIGUSR2)
        XCTAssertTrue(
            hostHandlerSawInfo,
            "the host's SA_SIGINFO handler was handed a nil siginfo_t"
        )

        signal(SIGUSR2, SIG_DFL)
    }

    func testTheMessageSaysWhatActuallyHappened() {
        // "SIGTRAP" starts no investigation. What it means does.
        XCTAssertEqual(SentoriSignalHandler.name(of: SIGTRAP), "SIGTRAP")
        XCTAssertTrue(SentoriSignalHandler.message(for: SIGTRAP).contains("force-unwrapped"))
        XCTAssertTrue(SentoriSignalHandler.message(for: SIGABRT).contains("fatalError"))
        // SIGPIPE is not handled: a socket closing is not a crash.
        XCTAssertFalse(SentoriSignalHandler.handled.contains(SIGPIPE))
    }
}
