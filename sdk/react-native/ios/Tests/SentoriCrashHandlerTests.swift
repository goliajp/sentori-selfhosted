// Phase 16 sub-E: XCTest coverage for SentoriCrashHandler (回填 Phase 7).
//
// Run via Xcode (target → SentoriTests, ⌘U) or via xcodebuild from the
// iOS host:
//   xcodebuild test \
//     -scheme SentoriTests \
//     -destination 'platform=iOS Simulator,name=iPhone 15'
//
// The handler writes one JSON file per crash to
//   <Documents>/sentori/pending/<uuid>.json
// We can't easily re-raise NSException (it tears down the test
// process), so we exercise the persistence helper directly instead.

import XCTest
@testable import SentoriCrashHandler

final class SentoriCrashHandlerTests: XCTestCase {
    private var pendingDir: URL!

    override func setUpWithError() throws {
        let docs = try FileManager.default.url(
            for: .documentDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        pendingDir = docs.appendingPathComponent("sentori/pending", isDirectory: true)
        try? FileManager.default.removeItem(at: pendingDir)
        try FileManager.default.createDirectory(at: pendingDir, withIntermediateDirectories: true)
    }

    func testWritePendingProducesValidEventJson() throws {
        // Synthesize a payload like the @convention(c) handler would.
        let exception = NSException(
            name: NSExceptionName("XCTestSyntheticException"),
            reason: "boom",
            userInfo: nil
        )
        SentoriCrashHandler.persistForTesting(exception: exception)

        let files = try FileManager.default.contentsOfDirectory(atPath: pendingDir.path)
            .filter { $0.hasSuffix(".json") }
        XCTAssertEqual(files.count, 1, "expected exactly one pending file, got \(files.count)")

        let url = pendingDir.appendingPathComponent(files[0])
        let data = try Data(contentsOf: url)
        let payload = try JSONSerialization.jsonObject(with: data, options: []) as? [String: Any]
        XCTAssertNotNil(payload, "pending file is not valid JSON")

        // Spot-check the protocol shape (Phase 1 schema).
        XCTAssertEqual(payload?["kind"] as? String, "error")
        XCTAssertEqual(payload?["platform"] as? String, "ios")
        XCTAssertNotNil(payload?["id"] as? String)
        XCTAssertNotNil(payload?["timestamp"] as? String)
        let error = payload?["error"] as? [String: Any]
        XCTAssertEqual(error?["type"] as? String, "XCTestSyntheticException")
        XCTAssertEqual(error?["message"] as? String, "boom")
    }
}
