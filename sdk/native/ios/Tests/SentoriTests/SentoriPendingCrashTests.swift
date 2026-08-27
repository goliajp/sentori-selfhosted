import XCTest

@testable import Sentori

/// The crash that killed the app has to survive the app.
///
/// `SentoriCrashHandler` wrote one JSON file per crash and nothing in
/// this package read them: the directory filled up and no crash was
/// ever sent. These cover the conversion from that on-disk shape to
/// the wire, which is where a field goes missing quietly.
final class SentoriPendingCrashTests: XCTestCase {

    override func setUp() {
        super.setUp()
        SentoriTransport.__resetForTests()
        SentoriConfig.__resetForTests()
        // The pending directory is one place on disk shared by every
        // test in the process, and `SentoriCrashHandlerTests` writes
        // there too. Without this, one test drains another's crash and
        // asserts on the wrong exception — which is what happened the
        // first time these two suites ran together.
        _ = SentoriCrashHandler.consumePending()
    }

    override func tearDown() {
        SentoriTransport.__resetForTests()
        SentoriConfig.__resetForTests()
        super.tearDown()
    }

    private let onDisk: [String: Any] = [
        "id": "019ff080-2aeb-7e30-aba1-4431b296d120",
        "timestamp": "2026-08-11T09:00:00.000Z",
        "kind": "error",
        "platform": "ios",
        "release": "com.example@1.4.0+220",
        "environment": "production",
        "device": ["os": "ios", "osVersion": "18.2", "model": "iPhone16,2"],
        "app": ["version": "1.4.0"],
        "error": [
            "type": "NSInvalidArgumentException",
            "message": "boom",
            "stack": [
                ["function": "-[Foo bar]", "file": "Foo.m", "line": 42, "inApp": true],
                ["function": "main"],
            ],
        ],
    ]

    func testEveryFieldSurvivesTheConversion() throws {
        let wire = SentoriPendingCrash.toWire(onDisk)

        XCTAssertEqual(wire["id"] as? String, "019ff080-2aeb-7e30-aba1-4431b296d120")
        XCTAssertEqual(wire["kind"] as? String, "error")
        XCTAssertEqual(wire["platform"] as? String, "ios")
        XCTAssertEqual(wire["release"] as? String, "com.example@1.4.0+220")
        XCTAssertEqual(wire["environment"] as? String, "production")

        // The file says `timestamp`, the wire says `occurredAt`, and
        // the value is the moment the app died — not the moment the
        // next launch noticed. Losing that would put every crash at
        // the time of the following start-up.
        XCTAssertEqual(wire["occurredAt"] as? String, "2026-08-11T09:00:00.000Z")

        let payload = try XCTUnwrap(wire["payload"] as? [String: Any])
        let error = try XCTUnwrap(payload["error"] as? [String: Any])
        XCTAssertEqual(error["type"] as? String, "NSInvalidArgumentException")
        XCTAssertEqual(error["message"] as? String, "boom")

        let stack = try XCTUnwrap(error["stack"] as? [[String: Any]])
        XCTAssertEqual(stack.count, 2)
        XCTAssertEqual(stack[0]["function"] as? String, "-[Foo bar]")
        XCTAssertEqual(stack[0]["line"] as? Int, 42)
        XCTAssertEqual(stack[0]["inApp"] as? Bool, true)
        // A frame with only a function keeps only a function rather
        // than gaining nulls the server would have to ignore.
        XCTAssertEqual(stack[1].count, 1)

        XCTAssertEqual((payload["device"] as? [String: Any])?["model"] as? String, "iPhone16,2")
        XCTAssertEqual((payload["app"] as? [String: Any])?["version"] as? String, "1.4.0")

        // The dashboard reads this to tell "the app died" from "the
        // app noticed".
        XCTAssertEqual(payload["nativeCrash"] as? Bool, true)
    }

    func testAMissingErrorStillProducesAReportableEvent() throws {
        // A crash file written mid-teardown can be missing anything.
        // Dropping the event would lose the only record that the app
        // died at all.
        let wire = SentoriPendingCrash.toWire(["platform": "ios"])
        let payload = try XCTUnwrap(wire["payload"] as? [String: Any])
        let error = try XCTUnwrap(payload["error"] as? [String: Any])

        XCTAssertEqual(error["type"] as? String, "NativeCrash", "names what it is")
        XCTAssertEqual(error["message"] as? String, "")
        XCTAssertEqual((error["stack"] as? [[String: Any]])?.count, 0)
        XCTAssertEqual((wire["id"] as? String)?.count, 36, "minted when the file had none")
        XCTAssertNotNil(wire["occurredAt"])
    }

    func testAndroidFilesKeepTheirPlatform() {
        // The same converter runs on both, and a crash filed as `ios`
        // would symbolicate against the wrong artifact.
        XCTAssertEqual(
            SentoriPendingCrash.toWire(["platform": "android"])["platform"] as? String,
            "android"
        )
        // Anything unrecognised is this platform rather than a value
        // the server has no column for.
        XCTAssertEqual(
            SentoriPendingCrash.toWire(["platform": "haiku"])["platform"] as? String,
            "ios"
        )
    }

    func testTheConvertedEventIsEncodable() throws {
        // It goes through `JSONSerialization` on the way out; a value
        // that cannot encode would lose the batch it rides in.
        XCTAssertTrue(JSONSerialization.isValidJSONObject(SentoriPendingCrash.toWire(onDisk)))
    }

    func testStartIsWhatDrainsThem() throws {
        // Through `Sentori.start`, not `ship` directly.
        //
        // The first version of this suite called `ship()` itself, so
        // removing the call from `start` left every test green — the
        // exact shape of the bug being fixed here, where the crash
        // handler wrote files and nothing read them. A unit test of a
        // function nobody calls proves the function, not the feature.
        SentoriTransport.forcedOutcomeForTests = 2  // stay off the network
        SentoriCrashHandler.persistForTesting(
            exception: NSException(name: .init("StartupDrain"), reason: "died", userInfo: nil))

        Sentori.start(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))

        // Generous on purpose. This asserts that draining happens at
        // all, not that it happens quickly — and a ten-second budget
        // for a cold transport, a file read and a queue write is a
        // budget a loaded CI runner loses. It did, twice, and passed
        // on a re-run of the same commit. A liveness test that fails
        // on a slow machine reports the machine.
        let deadline = Date().addingTimeInterval(60)
        while SentoriTransport.__peekPersisted().isEmpty,
            SentoriTransport.__peekQueue().isEmpty,
            Date() < deadline
        {
            Thread.sleep(forTimeInterval: 0.1)
        }
        let all = SentoriTransport.__peekPersisted() + SentoriTransport.__peekQueue()
        XCTAssertTrue(
            all.contains { ev in
                (((ev["payload"] as? [String: Any])?["error"] as? [String: Any])?["type"]
                    as? String) == "StartupDrain"
            },
            "start did not drain the pending crash — the handler writes files nobody reads"
        )
    }

    func testShipEnqueuesWhatTheHandlerWrote() throws {
        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))
        SentoriTransport.forcedOutcomeForTests = 2  // .failed — stay off the network
        SentoriTransport.start()

        // Write one the way the handler does, then drain it.
        SentoriCrashHandler.persistForTesting(
            exception: NSException(name: .init("Boom"), reason: "died", userInfo: nil))
        SentoriPendingCrash.ship()

        // `ship` flushes, and the forced failure spills the batch —
        // so the event is in the spill file, not the queue. Looking
        // in the queue is looking where it has already left, which is
        // how this test failed the first time it ran.
        //
        // Wait for *this* crash, not for the file to stop being
        // empty, and wait long enough for a busy worker.
        //
        // This failed once on CI — one run in twelve, with the same
        // commit green on two others — and it has not been reproduced
        // here, including by running the suites in the order that
        // would collide. So this is a guard against two plausible
        // causes rather than a fix for a diagnosed one, and both are
        // things the test should not have depended on anyway:
        //
        //   1. The spill is one file the whole process shares, so
        //      another test's events satisfy "not empty" and this one
        //      then reads before its own has landed.
        //   2. The send runs on a serial worker. A previous test that
        //      left a real network attempt on it — three tries with a
        //      fifteen-second timeout each, on a runner where a closed
        //      port hangs rather than refuses — delays this one well
        //      past ten seconds without anything being wrong.
        //
        // Asking for the event it wrote, with room for the queue in
        // front of it, depends on neither.
        func ourCrash() -> [String: Any]? {
            SentoriTransport.__peekPersisted().first { ev in
                ((ev["payload"] as? [String: Any])?["nativeCrash"] as? Bool) == true
                    && (((ev["payload"] as? [String: Any])?["error"] as? [String: Any])?["type"]
                        as? String) == "Boom"
            }
        }
        let deadline = Date().addingTimeInterval(60)
        while ourCrash() == nil, Date() < deadline {
            Thread.sleep(forTimeInterval: 0.1)
        }
        let crash = ourCrash()
        XCTAssertNotNil(crash, "the crash the handler wrote never reached the queue")
        XCTAssertEqual(
            ((crash?["payload"] as? [String: Any])?["error"] as? [String: Any])?["type"] as? String,
            "Boom"
        )

        // Drained means drained: a second start must not send it
        // again, or every launch re-reports the same crash.
        SentoriTransport.__resetForTests()
        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))
        SentoriTransport.start()
        SentoriPendingCrash.ship()
        XCTAssertTrue(
            SentoriTransport.__peekQueue().isEmpty && SentoriTransport.__peekPersisted().isEmpty,
            "the file was consumed, not copied — otherwise every launch re-reports the same crash"
        )
    }
}
