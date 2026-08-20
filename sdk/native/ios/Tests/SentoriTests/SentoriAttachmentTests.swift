import XCTest

@testable import Sentori

/// The screenshot taken as the app died, and the view tree behind it.
///
/// Both were already being captured and written into the crash file
/// on both platforms — and then nothing uploaded them. The event
/// arrived, the evidence did not, and the dashboard showed a crash
/// with an empty viewport that looked like a capture bug.
///
/// Two things can go wrong here and neither shows up as an error:
/// uploading before the server has the event (a 404 nobody reads),
/// and refusing a kind the server would have accepted.
final class SentoriAttachmentTests: XCTestCase {

    private var uploads: [(id: String, kind: String, body: String)] = []

    override func setUp() {
        super.setUp()
        SentoriTransport.__resetForTests()
        SentoriConfig.__resetForTests()
        _ = SentoriCrashHandler.consumePending()
        uploads = []
        SentoriAttachment.recorderForTests = { [weak self] id, kind, body in
            self?.uploads.append((id: id, kind: kind, body: body))
        }
    }

    override func tearDown() {
        SentoriAttachment.recorderForTests = nil
        SentoriTransport.__resetForTests()
        SentoriConfig.__resetForTests()
        super.tearDown()
    }

    private func configure() {
        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "com.example@1.0.0+1",
                environment: "test"
            ))
    }

    /// Poll rather than sleep: a fixed wait encodes a guess about how
    /// fast the machine is, and CI is slower than this one.
    ///
    /// The default is generous because a passing wait returns
    /// immediately and only a failing one spends the budget. Five
    /// seconds bought nothing and cost two red builds.
    private func waitUntil(
        _ what: String, timeout: TimeInterval = 30, _ cond: () -> Bool,
        file: StaticString = #filePath, line: UInt = #line
    ) {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if cond() { return }
            RunLoop.current.run(until: Date().addingTimeInterval(0.02))
        }
        XCTFail("\(what) — not after \(Int(timeout))s", file: file, line: line)
    }

    /// A crash file with a screenshot in it, as the handler writes it.
    private func crashFile(id: String) -> [String: Any] {
        [
            "id": id,
            "timestamp": "2026-08-11T09:00:00.000Z",
            "kind": "error",
            "platform": "ios",
            "release": "com.example@1.0.0+1",
            "environment": "test",
            "device": ["os": "ios", "osVersion": "18.2", "model": "iPhone16,2"],
            "app": ["version": "1.0.0"],
            "error": ["type": "NSInvalidArgumentException", "message": "boom"],
            "_pendingAttachments": [
                [
                    "kind": "screenshot", "base64": "aGVsbG8=",
                    "mediaType": "image/jpeg", "source": "ios",
                ],
                [
                    "kind": "viewTree", "base64": "eyJhIjoxfQ==",
                    "mediaType": "application/json", "source": "ios",
                ],
            ],
        ]
    }

    // ── the allowlist ─────────────────────────────────────────────

    /// The first version of this list had three entries — `replay`,
    /// `screens`, `screenshot` — and the crash handler writes
    /// `viewTree`. The delivery path would have dropped, on the way
    /// out, the very evidence it exists to deliver.
    func testEveryKindTheCrashHandlerWritesIsAccepted() {
        for kind in ["screenshot", "viewTree"] {
            XCTAssertTrue(
                SentoriAttachment.known.contains(kind),
                "the crash handler writes '\(kind)' and this list would refuse it")
        }
    }

    /// Kept in step with `KINDS` in
    /// `self-hosted/server/src/handlers/sdk/events_attachments.rs`,
    /// where anything else is a 400 and the CHECK constraint behind
    /// it would refuse the row anyway.
    func testTheAllowlistIsTheServersList() {
        XCTAssertEqual(
            SentoriAttachment.known,
            [
                "logTail", "replay", "screens", "screenshot",
                "sessionTrail", "stateSnapshot", "viewTree",
            ])
        XCTAssertEqual(SentoriAttachment.knownSources, ["android", "ios", "js"])
    }

    func testAnUnknownKindIsDroppedRatherThanPosted() {
        configure()
        let done = expectation(description: "completion")
        SentoriAttachment.upload(
            eventId: "e1", kind: "heapDump", base64: "aGk=", mediaType: "application/octet-stream"
        ) { ok in
            XCTAssertFalse(ok)
            done.fulfill()
        }
        wait(for: [done], timeout: 1)
        XCTAssertTrue(uploads.isEmpty)
    }

    func testNothingIsPostedWithoutAConfig() {
        let done = expectation(description: "completion")
        SentoriAttachment.upload(
            eventId: "e1", kind: "screenshot", base64: "aGk=", mediaType: "image/jpeg"
        ) { ok in
            XCTAssertFalse(ok)
            done.fulfill()
        }
        wait(for: [done], timeout: 1)
        XCTAssertTrue(uploads.isEmpty)
    }

    // ── the body ──────────────────────────────────────────────────

    /// Built by hand, so it is worth reading back. The failure mode
    /// is a 2xx that stored the wrong bytes: without the transfer
    /// encoding the server keeps the base64 *text* as the image,
    /// which renders as nothing and reads as a capture problem.
    func testMultipartBodyIsWhatTheServerParses() {
        let body = SentoriAttachment.multipartBody(
            boundary: "BOUND", kind: "screenshot", mediaType: "image/jpeg",
            base64: "aGVsbG8=", source: "ios")

        XCTAssertTrue(body.hasPrefix("--BOUND\r\n"))
        XCTAssertTrue(body.hasSuffix("--BOUND--\r\n"))
        XCTAssertTrue(
            body.contains(
                "Content-Disposition: form-data; name=\"file\"; filename=\"screenshot.bin\"\r\n"))
        XCTAssertTrue(body.contains("Content-Type: image/jpeg\r\n"))
        XCTAssertTrue(body.contains("Content-Transfer-Encoding: base64\r\n"))
        XCTAssertTrue(body.contains("\r\n\r\naGVsbG8=\r\n"))
        XCTAssertTrue(body.contains("Content-Disposition: form-data; name=\"source\"\r\n\r\nios\r\n"))

        // Every part is separated by the boundary and the whole thing
        // ends with the closing one: two opening delimiters here, no
        // more and no fewer.
        XCTAssertEqual(body.components(separatedBy: "--BOUND\r\n").count - 1, 2)
    }

    // ── the ordering ──────────────────────────────────────────────

    /// The one that matters. An upload keyed on an event the server
    /// has not seen is a 404, and it wins that race every time: the
    /// batch waits for a flush and the upload does not.
    func testAttachmentsUploadOnlyAfterTheBatchLands() {
        configure()
        SentoriTransport.start()
        SentoriTransport.forcedOutcomeForTests = 0  // .delivered

        let id = "019ff080-2aeb-7e30-aba1-4431b296d120"
        SentoriCrashHandler.persistRawForTesting(crashFile(id: id))
        SentoriPendingCrash.ship()

        // `ship` enqueues and flushes on a worker, and the uploads
        // only run once that send comes back delivered.
        waitUntil("both blobs upload after the batch lands", timeout: 60) {
            self.uploads.count == 2
        }

        XCTAssertEqual(uploads.count, 2, "both blobs in the file should have gone up")
        XCTAssertEqual(Set(uploads.map { $0.kind }), ["screenshot", "viewTree"])
        for upload in uploads {
            XCTAssertEqual(
                upload.id, id, "an attachment keyed on anything but the event id is a 404")
        }
        XCTAssertTrue(uploads.contains { $0.body.contains("aGVsbG8=") })
        XCTAssertTrue(uploads.contains { $0.body.contains("Content-Type: application/json") })
    }

    /// If the batch was refused or spilled to disk, the events are not
    /// on the server and there is nothing to attach to. Uploading
    /// anyway spends the user's bandwidth on a guaranteed 404.
    func testNothingUploadsWhenTheBatchNeverLands() {
        configure()
        SentoriTransport.start()
        SentoriTransport.forcedOutcomeForTests = 2  // .failed

        let spilledId = "019ff080-2aeb-7e30-aba1-4431b296d121"
        SentoriCrashHandler.persistRawForTesting(crashFile(id: spilledId))
        SentoriPendingCrash.ship()

        // The event has to reach the spill before the absence of an
        // upload means anything — otherwise this passes by being early.
        //
        // Ask for this crash, not for a count. The spill is one file
        // the whole process shares, so `== 1` is an assertion about
        // what every other test left behind; and the send retries
        // three times with a doubling sleep on a serial worker, which
        // on a loaded runner is nowhere near five seconds. Both of
        // those failed on CI while passing here, twice, in different
        // tests — the same mistake wearing two hats.
        waitUntil("the failed batch spills", timeout: 60) {
            SentoriTransport.__peekPersisted().contains { $0["id"] as? String == spilledId }
        }

        XCTAssertTrue(uploads.isEmpty, "a spilled batch has no event on the server to attach to")
    }

    // ── the hook itself ───────────────────────────────────────────

    /// Registered for events that are still queued, it waits.
    func testAHookWaitsUntilItsOwnEventsLand() {
        configure()
        SentoriTransport.start()
        SentoriTransport.forcedOutcomeForTests = 0  // .delivered

        var fired = 0
        SentoriTransport.afterDelivery(of: ["a", "b"]) { fired += 1 }

        SentoriTransport.enqueue(["id": "a", "kind": "error"])
        SentoriTransport.flush()
        waitUntil("the first batch lands") { SentoriTransport.__peekDelivered() >= 1 }
        XCTAssertEqual(fired, 0, "half its events have landed — it is not due yet")

        SentoriTransport.enqueue(["id": "b", "kind": "error"])
        SentoriTransport.flush()
        waitUntil("the hook fires once both have landed") { fired == 1 }
    }

    /// Registered for events the server refuses outright, it is
    /// dropped: there is nothing on the server to attach to, and
    /// uploading anyway spends a device's bandwidth on a 404.
    func testAHookIsDiscardedWhenTheServerRefusesItsEvents() {
        configure()
        SentoriTransport.start()
        SentoriTransport.forcedOutcomeForTests = 1  // .dropped

        var fired = false
        SentoriTransport.afterDelivery(of: ["a"]) { fired = true }
        XCTAssertEqual(SentoriTransport.__peekWaiting(), 1)

        SentoriTransport.enqueue(["id": "a", "kind": "error"])
        SentoriTransport.flush()

        waitUntil("the refusal discards the waiter") {
            SentoriTransport.__peekWaiting() == 0
        }
        XCTAssertFalse(fired, "the events it waited on were refused")
    }

    /// The shape of the bug CI caught. A hook registered after its
    /// batch has already been accepted never fires — which is why
    /// `ship` registers before it enqueues, not after it flushes.
    func testAHookRegisteredTooLateNeverFires() {
        configure()
        SentoriTransport.start()
        SentoriTransport.forcedOutcomeForTests = 0

        SentoriTransport.enqueue(["id": "a", "kind": "error"])
        SentoriTransport.flush()
        waitUntil("the batch lands") { SentoriTransport.__peekDelivered() >= 1 }

        var fired = false
        SentoriTransport.afterDelivery(of: ["a"]) { fired = true }
        SentoriTransport.enqueue(["id": "z", "kind": "error"])
        SentoriTransport.flush()
        waitUntil("a later batch lands") { SentoriTransport.__peekDelivered() >= 2 }

        XCTAssertFalse(
            fired,
            "it fired on someone else's batch — an upload keyed on an event that "
                + "batch never carried is a 404")
    }

    /// The blobs travel in the file and never on the wire. An event
    /// carrying a base64 screenshot inside its JSON is a megabyte in
    /// a batch that is meant to be a few kilobytes.
    func testPendingAttachmentsNeverGoOnTheWire() {
        let wire = SentoriPendingCrash.toWire(crashFile(id: "e1"))
        XCTAssertNil(wire["_pendingAttachments"])
        let payload = wire["payload"] as? [String: Any]
        XCTAssertNil(payload?["_pendingAttachments"])

        let encoded = String(
            data: try! JSONSerialization.data(withJSONObject: wire), encoding: .utf8)!
        XCTAssertFalse(
            encoded.contains("aGVsbG8="), "the screenshot bytes must not ride inside the event")
    }

    /// A crash file with no screenshot — the common case, since the
    /// capture can fail — must still deliver its event and must not
    /// leave a delivery hook queued behind it.
    func testACrashWithoutAttachmentsUploadsNothingAndStillShips() {
        configure()
        SentoriTransport.start()
        SentoriTransport.forcedOutcomeForTests = 0  // .delivered

        var file = crashFile(id: "019ff080-2aeb-7e30-aba1-4431b296d122")
        file.removeValue(forKey: "_pendingAttachments")
        SentoriCrashHandler.persistRawForTesting(file)
        SentoriPendingCrash.ship()

        // `>= 1`, not `== 1`. An earlier test spills a batch to the
        // one Documents directory the whole process shares, `start`
        // drains it, and the count this test reads is then two — a
        // test failing on what another test left behind rather than on
        // anything it did. CI found that; this machine did not.
        waitUntil("the crash is delivered", timeout: 60) {
            SentoriTransport.__peekDelivered() >= 1
        }

        XCTAssertTrue(uploads.isEmpty)
    }
}
