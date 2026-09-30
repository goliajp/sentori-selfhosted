import XCTest

@testable import Sentori

final class SentoriReplayDriverTests: XCTestCase {
    override func tearDown() {
        SentoriReplayDriver.__resetForTests()
        super.tearDown()
    }

    func testReplayIsOffUntilAHostAsksForIt() {
        // The most expensive thing this SDK can do. A customer turns
        // it on deliberately or not at all.
        SentoriReplayDriver.__resetForTests()
        XCTAssertFalse(SentoriReplayDriver.__isRunningForTests())
        XCTAssertEqual(SentoriReplayDriver.drain(), "")
    }

    func testStartingTwiceDoesNotRunTwoTimers() {
        SentoriReplayDriver.start(hz: 1)
        SentoriReplayDriver.start(hz: 1)
        XCTAssertTrue(SentoriReplayDriver.__isRunningForTests())
        SentoriReplayDriver.stop()
        XCTAssertFalse(SentoriReplayDriver.__isRunningForTests())
    }

    func testDrainHandsBackNewlineDelimitedJsonAndStartsCold() {
        // The player reads one entry per line. Draining also resets,
        // so the next frame is a keyframe — resuming with a delta
        // would reconstruct against a state the player no longer has.
        SentoriReplayDriver.__pushForTests(
            SentoriReplay.Frame(
                ts: 1000, width: 320, height: 640,
                nodes: [SentoriReplay.Node(x: 0, y: 0, w: 10, h: 10, kind: "a", text: nil, color: nil)]
            )
        )
        SentoriReplayDriver.__pushForTests(
            SentoriReplay.Frame(
                ts: 1500, width: 320, height: 640,
                nodes: [SentoriReplay.Node(x: 0, y: 0, w: 10, h: 10, kind: "b", text: nil, color: nil)]
            )
        )
        let lines = SentoriReplayDriver.drain().components(separatedBy: "\n")
        XCTAssertEqual(lines.count, 2)
        XCTAssertTrue(lines[0].contains("\"key\""))
        XCTAssertTrue(lines[1].contains("\"delta\""))
        XCTAssertEqual(SentoriReplayDriver.drain(), "")
    }

    /// The deadlock this file's comment is about: the capture reads
    /// the view hierarchy, so off the main thread it does
    /// `DispatchQueue.main.sync`. A timer on the main queue would wait
    /// on the queue it is running on.
    func testTheTimerDoesNotRunOnTheMainQueue() {
        let ticked = expectation(description: "a tick ran somewhere that is not main")
        SentoriReplayDriver.start(hz: 20)
        DispatchQueue(label: "probe").asyncAfter(deadline: .now() + 0.3) {
            // If the timer were on main, this main-thread block would
            // never get to run between ticks that each block on main.
            DispatchQueue.main.async { ticked.fulfill() }
        }
        wait(for: [ticked], timeout: 3)
        SentoriReplayDriver.stop()
    }
}
