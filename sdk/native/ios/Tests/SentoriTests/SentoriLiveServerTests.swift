import XCTest

@testable import Sentori

/// Against a real Sentori server, not a mock.
///
/// Everything else in this suite asserts the shape this SDK *builds*.
/// That is worth nothing if the server rejects it, and a mock would
/// agree with whatever mistake the SDK is making — which is how a
/// wire format quietly diverges. The one thing that cannot agree with
/// a mistake is the server.
///
/// Skipped unless `SENTORI_TEST_BASE` and `SENTORI_TEST_TOKEN` are
/// set, so a plain `xcodebuild test` stays offline. CI's
/// `ios-live-ingest` job sets them against a `docker compose` stack.
/// A skip says so out loud: a test that silently passes when it did
/// not run is worse than no test.
final class SentoriLiveServerTests: XCTestCase {

    private var base: String!
    private var token: String!

    /// Written by whoever brought a server up — `scripts/ios-live-ingest.sh`
    /// locally and in CI. Not environment variables: neither a plain
    /// `xcodebuild` invocation nor `SIMCTL_CHILD_*` reaches the test
    /// runner inside the simulator, so the first version of this
    /// skipped every time while reporting TEST SUCCEEDED. A file the
    /// runner can read is the part of the host filesystem a simulator
    /// definitely has.
    private static var configURL: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()  // SentoriTests
            .deletingLastPathComponent()  // Tests
            .deletingLastPathComponent()  // ios
            .deletingLastPathComponent()  // native
            .appendingPathComponent("fixtures/live-server.json")
    }

    override func setUpWithError() throws {
        guard let data = try? Data(contentsOf: Self.configURL),
            let j = try? JSONSerialization.jsonObject(with: data) as? [String: String],
            let b = j["base"], let t = j["token"], !b.isEmpty, !t.isEmpty
        else {
            throw XCTSkip(
                "no sdk/native/fixtures/live-server.json — run scripts/ios-live-ingest.sh")
        }
        base = b
        token = t
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

    func testTheServerAcceptsWhatThisSDKSends() throws {
        Sentori.start(
            SentoriConfig(
                token: token,
                ingestUrl: base,
                release: "swift-e2e@1.0.0",
                environment: "test"
            ))
        Sentori.user(id: "usr_swift_1", email: nil)
        Sentori.context(["tenant": "acme"])
        Sentori.pushSignal(kind: "nav", data: ["to": "/checkout"])

        let id = Sentori.error(
            NSError(domain: "SwiftE2E", code: 42, userInfo: [NSLocalizedDescriptionKey: "boom"]),
            data: ["cartId": "c_1"]
        )
        Sentori.warn("checkout.slow", data: ["ms": 3200])
        Sentori.assert("total.positive", false)
        Sentori.probe("SEN-482")
        SentoriTransport.flush()

        // Poll for delivery rather than sleeping and checking that
        // nothing has failed *yet*.
        //
        // The first version waited three seconds and asserted the
        // spill file was empty and the queue was drained. Both are
        // true while three retries are still in flight, and both are
        // true when the server answers 4xx — the transport treats a
        // client error as handled rather than retrying it forever. So
        // it passed on CI against a server that had stored nothing,
        // and only the script's readback noticed.
        // 30s was not enough on a cold macOS runner: one CI run failed
        // here with an empty queue AND an empty spill file — nothing
        // had been handed to the transport yet, which is the shape of
        // "still starting up", not of "refused". A rerun of the same
        // commit passed. 90s costs nothing on a healthy run, since the
        // loop exits on the first delivery.
        let deadline = Date().addingTimeInterval(90)
        while SentoriTransport.__peekDelivered() == 0, Date() < deadline {
            Thread.sleep(forTimeInterval: 0.25)
        }

        XCTAssertGreaterThan(
            SentoriTransport.__peekDelivered(), 0,
            "no batch was delivered within 90s — spilled: "
                + "\(SentoriTransport.__peekPersisted().count), "
                + "queued: \(SentoriTransport.__peekQueue().count)"
        )
        XCTAssertTrue(
            SentoriTransport.__peekPersisted().isEmpty,
            "a batch spilled to disk, so the server refused it"
        )
        XCTAssertEqual(id.count, 36)

        // The multipart body against a real parser.
        //
        // Everything else about attachments is asserted as a string,
        // and a string that looks right is exactly what React Native
        // shipped for a release while every upload failed on the
        // device. The body is hand-built; the only proof it is
        // well-formed is a server that read it.
        let uploaded = expectation(description: "attachment upload")
        var status = false
        SentoriAttachment.upload(
            eventId: id,
            kind: "screenshot",
            // A one-pixel JPEG, so the server stores an image rather
            // than bytes that merely decode.
            base64: onePixelJpegBase64,
            mediaType: "image/jpeg"
        ) { ok in
            status = ok
            uploaded.fulfill()
        }
        wait(for: [uploaded], timeout: 30)
        XCTAssertTrue(
            status,
            "the server refused the multipart body — it parses this, or attachments "
                + "die silently on every device"
        )
    }

    /// The smallest valid JPEG, base64. Content matters here: the
    /// server stores what it decodes, and a payload that is not an
    /// image would pass the upload and fail the viewer.
    private let onePixelJpegBase64 =
        "/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0a"
        + "HBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/wAALCAABAAEBAREA/8QAFAABAAAAAAAA"
        + "AAAAAAAAAAAACf/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AKp//2Q=="
}
