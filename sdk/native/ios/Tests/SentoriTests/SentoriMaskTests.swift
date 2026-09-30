import XCTest

@testable import Sentori

final class SentoriMaskTests: XCTestCase {
    override func tearDown() {
        SentoriMask.__resetForTests()
        super.tearDown()
    }

    func testNothingIsMaskedUntilAHostRegistersAQuery() {
        SentoriMask.__resetForTests()
        XCTAssertEqual(SentoriMask.ids(), [])
    }

    func testTheRegisteredIdsComeBack() {
        SentoriMask.register { ["card-number", "camera-feed"] }
        XCTAssertEqual(SentoriMask.ids(), ["card-number", "camera-feed"])
    }

    func testRegisteringNilClears() {
        SentoriMask.register { ["x"] }
        SentoriMask.register(nil)
        XCTAssertEqual(SentoriMask.ids(), [])
    }

    func testTheQueryIsAskedEveryTimeRatherThanCached() {
        // A host returning a list that changes with the screen is the
        // normal case; caching the first answer would mask the first
        // screen's fields on every screen after it, and leave the
        // current screen's exposed.
        var screen = 0
        SentoriMask.register { screen == 0 ? ["login"] : ["checkout"] }
        XCTAssertEqual(SentoriMask.ids(), ["login"])
        screen = 1
        XCTAssertEqual(SentoriMask.ids(), ["checkout"])
    }
}
