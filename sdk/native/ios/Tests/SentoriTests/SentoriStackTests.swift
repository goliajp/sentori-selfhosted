import XCTest

@testable import Sentori

final class SentoriStackTests: XCTestCase {
    func testCaptureReturnsAddressesAndIsBounded() {
        let addresses = SentoriStack.capture(skip: 0)
        XCTAssertFalse(addresses.isEmpty, "a test runs on a real stack; zero frames means the walk failed")
        XCTAssertLessThanOrEqual(addresses.count, SentoriStack.maxDepth)
    }

    func testSkipDropsOurOwnFrames() {
        // Comparing counts would prove nothing: both are capped at
        // maxDepth on a stack this deep. What `skip` means is which
        // frame comes first.
        let full = SentoriStack.capture(skip: 0)
        let skipped = SentoriStack.capture(skip: 2)
        XCTAssertGreaterThan(full.count, 2)
        XCTAssertEqual(skipped.first, full[2])
    }

    func testResolveNamesTheFunctionItWasCalledFrom() {
        let frames = SentoriStack.resolve(SentoriStack.capture(skip: 0))
        XCTAssertFalse(frames.isEmpty)
        let functions = frames.compactMap { $0["function"] as? String }
        XCTAssertTrue(
            functions.contains { $0.contains("testResolveNamesTheFunctionItWasCalledFrom") },
            "the frame this call was made from is not in \(functions.prefix(6))"
        )
        // Every frame carries the shape the wire expects, resolved or not.
        for frame in frames {
            XCTAssertNotNil(frame["function"] as? String)
            XCTAssertNotNil(frame["file"] as? String)
            XCTAssertNotNil(frame["inApp"] as? Bool)
            XCTAssertEqual(frame["line"] as? Int, 0)
        }
    }

    func testAFrameCarriesTheImageItsAddressBelongsTo() {
        // Without these the server cannot reach the release's dSYM,
        // and a native stack stays a column of hex.
        let frames = SentoriStack.resolve(SentoriStack.capture(skip: 0))
        let withImage = frames.filter { $0["imageUuid"] != nil && $0["imageBase"] != nil }
        XCTAssertFalse(withImage.isEmpty, "no frame carried an image identity")
        let uuid = withImage[0]["imageUuid"] as? String
        XCTAssertEqual(uuid?.count, 32, "a UUID is 32 hex characters, no dashes: \(uuid ?? "nil")")
        XCTAssertEqual(uuid, uuid?.lowercased())
    }

    func testSystemFramesAreNotInApp() {
        XCTAssertFalse(SentoriStack.isApp("UIKitCore"))
        XCTAssertFalse(SentoriStack.isApp("libsystem_kernel.dylib"))
        XCTAssertTrue(SentoriStack.isApp("MyApp"))
    }

    func testAMangledNameComesBackReadable() {
        // A dashboard showing `$s7Sentori...` is showing nothing.
        // A real mangled name, not one written by hand: the runtime
        // returns anything it cannot parse unchanged, so an invented
        // string makes this test pass by failing to be a test.
        XCTAssertEqual(SentoriStack.demangle("$sSi"), "Swift.Int")
        // An Objective-C selector is already readable and must survive.
        XCTAssertEqual(SentoriStack.demangle("-[UIViewController viewDidLoad]"),
                       "-[UIViewController viewDidLoad]")
    }

    /// Iron rule, dimension 1: a verb the host calls from a tap
    /// handler may not cost it a frame. Capture is the part that runs
    /// on the caller's thread.
    ///
    /// The budget is the project's own single-tick red line, 5 ms,
    /// and not the 0.04 ms this actually costs on an idle machine.
    /// A tight bound here measures how busy the runner is: this was
    /// 0.5 ms and went red in the published mirror's CI at 0.725 ms,
    /// while the same call takes 0.004 ms locally. Two hundred times
    /// the headroom is still enough to catch a regression that puts
    /// real work on this path, which is the only thing worth failing
    /// for.
    func testCaptureCostsFarLessThanAFrame() {
        let iterations = 1000
        let start = CFAbsoluteTimeGetCurrent()
        for _ in 0..<iterations { _ = SentoriStack.capture(skip: 0) }
        let perCall = (CFAbsoluteTimeGetCurrent() - start) / Double(iterations) * 1000
        print("SentoriStack.capture: \(String(format: "%.4f", perCall)) ms/call")
        XCTAssertLessThan(perCall, 5.0, "capture costs \(perCall) ms on the calling thread")
    }

    /// The design claim, stated as a ratio so it does not depend on
    /// how fast the machine is: capture is what a verb pays, resolve
    /// is what the worker pays, and the whole reason they are
    /// separate is that the second is far more expensive. If they
    /// ever converge, deferring bought nothing.
    func testCaptureIsOrdersOfMagnitudeCheaperThanResolve() {
        let addresses = SentoriStack.capture(skip: 0)
        let rounds = 200

        var start = CFAbsoluteTimeGetCurrent()
        for _ in 0..<rounds { _ = SentoriStack.capture(skip: 0) }
        let captureMs = (CFAbsoluteTimeGetCurrent() - start) / Double(rounds) * 1000

        start = CFAbsoluteTimeGetCurrent()
        for _ in 0..<rounds { _ = SentoriStack.resolve(addresses) }
        let resolveMs = (CFAbsoluteTimeGetCurrent() - start) / Double(rounds) * 1000

        print("capture \(captureMs) ms vs resolve \(resolveMs) ms")
        XCTAssertGreaterThan(
            resolveMs, captureMs * 10,
            "resolve (\(resolveMs) ms) is not meaningfully dearer than capture "
                + "(\(captureMs) ms) — deferring it bought nothing"
        )
    }

    /// Reported, not asserted on a tight bound: this runs on the
    /// transport's worker, and the number moves with how busy the
    /// machine is. A 5 ms ceiling here failed at 6.4 ms the first
    /// time the whole suite ran beside it — which is also what
    /// decided that this work does not belong on the caller's thread.
    func testResolveCostIsReportedNotBudgeted() {
        let addresses = SentoriStack.capture(skip: 0)
        let iterations = 200
        let start = CFAbsoluteTimeGetCurrent()
        for _ in 0..<iterations { _ = SentoriStack.resolve(addresses) }
        let perCall = (CFAbsoluteTimeGetCurrent() - start) / Double(iterations) * 1000
        print("SentoriStack.resolve(\(addresses.count) frames): \(String(format: "%.4f", perCall)) ms/call")
    }

    /// The invariant the iron rule actually needs: a verb parks
    /// addresses and does not symbolicate. A regression here would
    /// not show up as a failing budget on an idle CI machine — it
    /// would show up as a dropped frame on a customer's device.
    func testTheVerbParksAddressesRatherThanResolvingThem() {
        let event: [String: Any] = [
            "payload": [
                "error": [
                    "type": "E",
                    "message": "m",
                    SentoriStack.pendingKey: SentoriStack.capture(skip: 0),
                ]
            ]
        ]
        let before = ((event["payload"] as! [String: Any])["error"] as! [String: Any])
        XCTAssertNil(before["stack"], "the verb symbolicated on the caller's thread")
        XCTAssertNotNil(before[SentoriStack.pendingKey])

        let after = SentoriStack.resolvePending(in: event)
        let error = ((after["payload"] as! [String: Any])["error"] as! [String: Any])
        XCTAssertNil(
            error[SentoriStack.pendingKey],
            "raw addresses would have gone on the wire, where nothing can read them"
        )
        XCTAssertFalse((error["stack"] as! [[String: Any]]).isEmpty)
    }

    func testAnEventWithNoParkedStackPassesThroughUnchanged() {
        let event: [String: Any] = ["payload": ["error": ["type": "E", "message": "m"]]]
        let out = SentoriStack.resolvePending(in: event)
        let error = ((out["payload"] as! [String: Any])["error"] as! [String: Any])
        XCTAssertNil(error["stack"])
        XCTAssertEqual(error["type"] as? String, "E")
    }
}

/// The two halves of the crash handler and the error verb now share
/// one image walk. They used to have two, and they disagreed on the
/// case of the hex — which the server normalises, so the difference
/// would never have surfaced as a failure, only as two answers to the
/// same question.
final class SentoriImageTests: XCTestCase {
    func testTheUuidIsTheFormTheServerMatchesOn() {
        var info = Dl_info()
        let here = SentoriStack.capture(skip: 0)
        XCTAssertFalse(here.isEmpty)
        guard dladdr(UnsafeRawPointer(bitPattern: here[0].uintValue), &info) != 0,
              let base = info.dli_fbase,
              let uuid = SentoriImage.uuid(atBase: base)
        else {
            return XCTFail("no image identity for the frame this test runs in")
        }
        XCTAssertEqual(uuid.count, 32)
        XCTAssertEqual(uuid, uuid.lowercased())
        XCTAssertFalse(uuid.contains("-"))
    }
}
