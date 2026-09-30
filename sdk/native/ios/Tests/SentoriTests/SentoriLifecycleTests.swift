#if canImport(UIKit)
    import UIKit
#endif
import XCTest

@testable import Sentori

final class SentoriLifecycleTests: XCTestCase {
    override func setUp() {
        super.setUp()
        SentoriLifecycle.__resetForTests()
        SentoriSignalRing.clear()
    }

    override func tearDown() {
        SentoriLifecycle.__resetForTests()
        super.tearDown()
    }

    #if canImport(UIKit)
        func testBackgroundingLandsInTheSignalRing() {
            SentoriLifecycle.register()
            NotificationCenter.default.post(
                name: UIApplication.didEnterBackgroundNotification, object: nil
            )
            let names = SentoriSignalRing.snapshot().compactMap {
                ($0["data"] as? [String: Any])?["name"] as? String
            }
            XCTAssertTrue(names.contains("app.background"), "ring holds \(names)")
        }

        func testMemoryPressureIsRecorded() {
            // The question anyone asks about a crash they cannot
            // reproduce, and the ring could not answer it.
            SentoriLifecycle.register()
            NotificationCenter.default.post(
                name: UIApplication.didReceiveMemoryWarningNotification, object: nil
            )
            let names = SentoriSignalRing.snapshot().compactMap {
                ($0["data"] as? [String: Any])?["name"] as? String
            }
            XCTAssertTrue(names.contains("app.memoryWarning"), "ring holds \(names)")
        }

        func testRegisteringTwiceDoesNotDoubleTheSignals() {
            // `start()` called twice is a thing hosts do — in a hot
            // reload, or from two entry points — and a ring filling
            // twice as fast evicts twice as much real context.
            SentoriLifecycle.register()
            SentoriLifecycle.register()
            NotificationCenter.default.post(
                name: UIApplication.didEnterBackgroundNotification, object: nil
            )
            let backgrounds = SentoriSignalRing.snapshot().filter {
                ($0["data"] as? [String: Any])?["name"] as? String == "app.background"
            }
            XCTAssertEqual(backgrounds.count, 1)
        }
    #endif
}
