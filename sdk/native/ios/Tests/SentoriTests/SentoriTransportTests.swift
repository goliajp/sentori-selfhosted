import XCTest

@testable import Sentori

/// The transport carries the client zero-cost rule, and in Swift it
/// carries it without a JS event loop underneath. These assert the
/// parts an integrator would feel: the call returns immediately, it
/// never throws, and neither the queue nor the spill file can grow
/// without limit.
final class SentoriTransportTests: XCTestCase {

    override func setUp() {
        super.setUp()
        SentoriTransport.__resetForTests()
        SentoriConfig.__resetForTests()
        SentoriScope.clear()
        SentoriSignalRing.clear()
    }

    override func tearDown() {
        SentoriTransport.__resetForTests()
        SentoriConfig.__resetForTests()
        super.tearDown()
    }


    /// Poll a condition to a deadline, failing with the reason rather
    /// than sleeping a guessed interval and asserting afterwards.
    private func waitUntil(
        _ what: String,
        timeout: TimeInterval = 30,
        _ condition: () -> Bool
    ) {
        let deadline = Date().addingTimeInterval(timeout)
        while !condition(), Date() < deadline {
            Thread.sleep(forTimeInterval: 0.1)
        }
        XCTAssertTrue(
            condition(),
            "\(what) — not after \(Int(timeout))s; persisted: "
                + "\(SentoriTransport.__peekPersisted().count), "
                + "queued: \(SentoriTransport.__peekQueue().count)"
        )
    }

    private func configure(url: String = "http://127.0.0.1:9") {
        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: url,
                release: "app@1.0.0",
                environment: "test"
            ))
    }

    func testEnqueueBeforeInitIsASilentNoOpRatherThanACrash() {
        // No config: the whole SDK is a no-op. This is the state a
        // mis-wired token leaves an app in, and it must be boring.
        SentoriTransport.enqueue(["kind": "error"])
        SentoriTransport.flush()
        XCTAssertEqual(SentoriTransport.__peekQueue().count, 1, "queued, but nothing sent")
    }

    func testEnqueueReturnsWithoutWaitingOnTheNetwork() {
        configure()
        SentoriTransport.start()

        // 10 events trips the batch size and starts a send to a port
        // nothing listens on. The call must still return in well under
        // the connect timeout — the whole point is that the caller's
        // thread is never the one waiting.
        let start = Date()
        for i in 0..<10 { SentoriTransport.enqueue(["kind": "error", "seq": i]) }
        let elapsed = Date().timeIntervalSince(start)

        XCTAssertLessThan(
            elapsed, 0.05,
            "enqueue took \(elapsed)s — it is doing work that belongs on the worker"
        )
    }

    func testTheQueueIsBounded() {
        configure()
        // Deliberately not started: nothing drains, so this measures
        // the cap rather than a race with the flusher.
        for i in 0..<2000 { SentoriTransport.enqueue(["kind": "trace", "seq": i]) }

        let queued = SentoriTransport.__peekQueue()
        XCTAssertLessThanOrEqual(
            queued.count, 500,
            "an unbounded queue is a leak with a nicer name"
        )
        // Oldest go first: the crash happening now matters more than
        // the one from ten minutes ago.
        XCTAssertEqual(queued.last?["seq"] as? Int, 1999)
    }

    func testAssertOutcomesAggregateInsteadOfBecomingEvents() {
        configure()
        for _ in 0..<5 { SentoriTransport.countAssert(name: "cart.total", ok: true, release: "r1") }
        SentoriTransport.countAssert(name: "cart.total", ok: false, release: "r1")
        SentoriTransport.countAssert(name: "cart.total", ok: true, release: "r2")

        XCTAssertTrue(
            SentoriTransport.__peekQueue().isEmpty,
            "a passing assert must never become an event — that is the heartbeat flood"
        )
        let stats = SentoriTransport.__peekAssertStats()
        XCTAssertEqual(stats.count, 2, "one row per (name, release)")

        let r1 = stats.first { $0["release"] as? String == "r1" }
        XCTAssertEqual(r1?["passDelta"] as? Int, 5)
        XCTAssertEqual(r1?["failDelta"] as? Int, 1)
    }

    func testAFailedSendSpillsToDiskAndDrainsOnTheNextStart() {
        configure()
        // Force the send to fail instead of arranging for it. Two
        // attempts to provoke a real failure — a closed port, then a
        // one-second timeout — each behaved differently on CI than
        // here, and both times the test reported a broken transport
        // when the transport was fine. This test is about the spill
        // and the drain; `ios-live-ingest` is what exercises the
        // network.
        SentoriTransport.forcedOutcomeForTests = 2  // .failed
        SentoriTransport.requestTimeout = 1
        SentoriTransport.start()
        for i in 0..<3 { SentoriTransport.enqueue(["kind": "error", "seq": i]) }
        SentoriTransport.flush()

        // Poll rather than sleep. A fixed wait encodes a guess about
        // how fast the machine is, and the guess was wrong on a CI
        // runner roughly half the speed of this one: the drain had not
        // reached the worker after a second, and the test failed on a
        // transport that was working.
        waitUntil("events spill when the send fails") {
            SentoriTransport.__peekPersisted().count == 3
        }

        // A fresh process: the spill goes back through the normal path
        // and the file is cleared, so a second failure spills once
        // rather than doubling.
        SentoriConfig.__resetForTests()
        SentoriTransport.start()
        waitUntil("the spill comes back on the next start") {
            SentoriTransport.__peekQueue().count == 3
        }
        XCTAssertTrue(SentoriTransport.__peekPersisted().isEmpty, "and the file was cleared")
    }

    func testGarbageNeverThrows() {
        configure()
        SentoriTransport.start()
        // Values JSONSerialization cannot encode. The verb contract is
        // that nothing here reaches the host, so the worst outcome is
        // a batch that does not go out.
        SentoriTransport.enqueue(["kind": "error", "bad": Double.nan])
        SentoriTransport.enqueue(["kind": "error", "date": Date()])
        SentoriTransport.enqueue([:])
        SentoriTransport.flush()
        // Reaching here without a crash is the assertion.
        XCTAssertTrue(true)
    }
}
