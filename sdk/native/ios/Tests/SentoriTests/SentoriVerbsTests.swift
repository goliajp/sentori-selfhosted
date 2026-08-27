import XCTest

@testable import Sentori

/// The five verbs, and the contract an integrator is entitled to:
/// synchronous, an id back every time, never a throw, and a no-op
/// before `start` rather than a crash.
final class SentoriVerbsTests: XCTestCase {

    private enum Boom: Error { case detonated }

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

    private func started() {
        Sentori.start(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))
    }

    private func queued() -> [[String: Any]] { SentoriTransport.__peekQueue() }

    func testEveryVerbBeforeStartIsASilentNoOpThatStillReturnsAnId() {
        // The failure-isolation rule: an app that mis-wired its token
        // must not have call sites that behave differently, and must
        // certainly not crash on the path that reports crashes.
        let ids = [
            Sentori.error(Boom.detonated),
            Sentori.warn("checkout.slow"),
            Sentori.trace("cart.opened"),
            Sentori.assert("total.positive", false),
            Sentori.probe("SEN-482"),
        ]
        for id in ids {
            XCTAssertEqual(id.count, 36, "every verb returns an event id")
        }
        XCTAssertTrue(queued().isEmpty, "and nothing is queued before init")
    }

    func testErrorCarriesTypeMessageAndTheSignalRing() {
        started()
        Sentori.pushSignal(kind: "nav", data: ["to": "/checkout"])
        Sentori.error(Boom.detonated, data: ["cartId": "c_1"])

        let e = queued().last
        XCTAssertEqual(e?["kind"] as? String, "error")
        XCTAssertEqual(e?["platform"] as? String, "ios")
        XCTAssertEqual(e?["release"] as? String, "app@1.0.0")

        let payload = e?["payload"] as? [String: Any]
        XCTAssertEqual((payload?["error"] as? [String: Any])?["type"] as? String, "Boom")
        XCTAssertEqual((payload?["data"] as? [String: Any])?["cartId"] as? String, "c_1")

        // The lead-up is the reason an error is readable at all.
        let signals = payload?["signals"] as? [[String: Any]]
        XCTAssertEqual(signals?.count, 1)
        XCTAssertEqual(signals?.first?["kind"] as? String, "nav")
    }

    func testAPassingAssertIsNeverAnEvent() {
        started()
        Sentori.assert("total.positive", true)
        Sentori.assert("total.positive", true)
        XCTAssertTrue(queued().isEmpty, "passes aggregate; only failures are events")
        XCTAssertEqual(SentoriTransport.__peekAssertStats().first?["passDelta"] as? Int, 2)

        Sentori.assert("total.positive", false)
        XCTAssertEqual(queued().count, 1)
        XCTAssertEqual(queued().first?["kind"] as? String, "assert")
    }

    func testAssertNeverStopsTheProgram() {
        started()
        // The difference from the language's own `assert`, and the
        // reason this one is safe to leave in a release build.
        Sentori.assert("impossible", false)
        Sentori.assert("also impossible", false)
        XCTAssertEqual(queued().count, 2, "reached here, twice")
    }

    func testQuietTraceReachesTheRingAndNotTheQueue() {
        started()
        Sentori.trace("tick", quiet: true)
        XCTAssertTrue(queued().isEmpty, "a quiet trace must stay affordable")
        XCTAssertEqual(SentoriSignalRing.snapshot().count, 1, "but it is still context")

        Sentori.trace("cart.opened")
        XCTAssertEqual(queued().count, 1)
        XCTAssertEqual(queued().first?["name"] as? String, "cart.opened")
    }

    func testUserKeyRidesEventsOnlyAfterUserIsCalled() {
        started()
        Sentori.warn("before")
        XCTAssertNil(queued().last?["userKey"], "no identity, no key — not an empty one")

        Sentori.user(id: "usr_123", email: nil)
        Sentori.warn("after")
        XCTAssertEqual(
            queued().last?["userKey"] as? String,
            SentoriIdentity.hash(keyType: "id", value: "usr_123")
        )
    }

    func testContextMergesAndRidesEveryEvent() {
        started()
        Sentori.context(["tenant": "acme"])
        Sentori.context(["plan": "pro"])
        Sentori.probe("SEN-1")

        let ctx = (queued().last?["payload"] as? [String: Any])?["context"] as? [String: Any]
        XCTAssertEqual(ctx?["tenant"] as? String, "acme")
        XCTAssertEqual(ctx?["plan"] as? String, "pro")
    }

    func testEventIdsAreUuidV7AndSortByTime() {
        let first = Sentori.newEventId()
        Thread.sleep(forTimeInterval: 0.01)
        let second = Sentori.newEventId()

        // Version nibble, then variant. The server keys events on this
        // and the dashboard orders by it, so a v4 would scatter a
        // session across the index.
        XCTAssertEqual(first.count, 36)
        let v = first[first.index(first.startIndex, offsetBy: 14)]
        XCTAssertEqual(v, "7", "version nibble")
        XCTAssertLessThan(first, second, "v7 ids sort by creation time as strings")
    }

    func testGarbageInDataDoesNotReachTheCaller() {
        started()
        // A dictionary JSONSerialization cannot encode. The verb still
        // returns an id and does not throw; the batch is what suffers.
        let id = Sentori.warn("bad", data: ["nan": Double.nan, "date": Date()])
        XCTAssertEqual(id.count, 36)
    }
}
