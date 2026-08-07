// Phase 29 sub-A step 3: XCTest coverage for SentoriThreadSampler.
//
// Run via Xcode (target → SentoriTests, ⌘U) or via xcodebuild from the
// iOS host:
//   xcodebuild test \
//     -scheme SentoriTests \
//     -destination 'platform=iOS Simulator,name=iPhone 15'
//
// On Apple Silicon Mac the simulator runs the arm64 slice and the
// sampler can walk frames; on Intel Mac the simulator slice is x86_64
// and the sampler returns []. Both paths are asserted below.

import XCTest

@testable import SentoriCrashHandler

final class SentoriThreadSamplerTests: XCTestCase {

    override func setUp() {
        super.setUp()
        // Capture the main pthread → mach port mapping. setUp runs on
        // the test runner's main queue, which is the main thread.
        SentoriThreadSampler.installMainThreadHandle()
    }

    /// Background → sampler → main: should collect a non-trivial frame
    /// chain on arm64 simulators.
    func testCaptureFromBackgroundReturnsAtLeastFiveFrames() {
        let exp = expectation(description: "background sample")
        DispatchQueue.global(qos: .userInitiated).async {
            let frames = SentoriThreadSampler.captureMainThreadFrames(maxFrames: 64)
            #if arch(arm64)
                XCTAssertGreaterThanOrEqual(
                    frames.count, 5,
                    "expected ≥ 5 main-thread frames on arm64; got \(frames.count)"
                )
                if !frames.isEmpty {
                    XCTAssertGreaterThan(
                        frames[0].uint64Value, 0,
                        "first PC must be non-zero"
                    )
                }
                XCTAssertLessThanOrEqual(
                    frames.count, 64,
                    "must respect maxFrames cap"
                )
            #else
                // Intel simulator: sampler returns empty by design.
                XCTAssertEqual(frames.count, 0)
            #endif
            exp.fulfill()
        }
        wait(for: [exp], timeout: 5.0)
    }

    /// Sampling from main itself must refuse — would race with our own
    /// register state.
    func testCaptureFromMainReturnsEmpty() {
        let frames = SentoriThreadSampler.captureMainThreadFrames(maxFrames: 64)
        XCTAssertEqual(
            frames.count, 0,
            "sampling from main must return [] (would race with own state)"
        )
    }

    /// `installMainThreadHandle` must be safe to call repeatedly.
    func testInstallIsIdempotent() {
        SentoriThreadSampler.installMainThreadHandle()
        SentoriThreadSampler.installMainThreadHandle()
        SentoriThreadSampler.installMainThreadHandle()

        let exp = expectation(description: "still works after re-install")
        DispatchQueue.global().async {
            let frames = SentoriThreadSampler.captureMainThreadFrames(maxFrames: 16)
            #if arch(arm64)
                XCTAssertGreaterThan(
                    frames.count, 0,
                    "re-installs must not break the captured handle"
                )
            #endif
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2.0)
    }

    /// `maxFrames: 0` returns empty even on arm64.
    func testZeroMaxFramesReturnsEmpty() {
        let exp = expectation(description: "zero max")
        DispatchQueue.global().async {
            let frames = SentoriThreadSampler.captureMainThreadFrames(maxFrames: 0)
            XCTAssertEqual(frames.count, 0)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2.0)
    }
}
