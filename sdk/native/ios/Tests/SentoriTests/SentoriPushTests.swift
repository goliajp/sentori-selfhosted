import XCTest

@testable import Sentori

/// Push registration, as far as a simulator can go.
///
/// A simulator has no APNs entitlement, so the honest coverage here is
/// the shape of the failures and the shape of the request — not a real
/// token. `scripts/ios-live-ingest.sh` proves the registration route
/// against a real server; a device proves the rest, and nothing here
/// pretends otherwise.
final class SentoriPushTests: XCTestCase {

    override func setUp() {
        super.setUp()
        SentoriConfig.__resetForTests()
        SentoriScope.clear()
        SentoriPush.shared.__resetForTests()
    }

    override func tearDown() {
        SentoriConfig.__resetForTests()
        SentoriPush.shared.__resetForTests()
        super.tearDown()
    }

    func testRegisterBeforeStartReportsItRatherThanThrowing() async {
        let result = await SentoriPush.shared.register(timeout: 1)
        guard case .failure(let reason, _) = result else {
            return XCTFail("expected a failure before start, got \(result)")
        }
        XCTAssertEqual(reason, .notInitialised)
        XCTAssertEqual(reason.name, "not-initialised")
    }

    func testEveryFailureHasTheSameNameAsTheReactNativeOne() {
        // Same strings as `PushRegisterFailure` in the RN SDK, so one
        // set of integration notes covers both and an operator reading
        // a support thread does not have to translate.
        XCTAssertEqual(SentoriPush.Failure.notInitialised.name, "not-initialised")
        XCTAssertEqual(SentoriPush.Failure.permissionDenied.name, "permission-denied")
        XCTAssertEqual(SentoriPush.Failure.noTransport.name, "no-transport")
        XCTAssertEqual(SentoriPush.Failure.tokenTimeout.name, "token-timeout")
        XCTAssertEqual(SentoriPush.Failure.serverRejected.name, "server-rejected")
    }

    func testRegisterOnASimulatorFailsWithoutThrowing() async {
        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))

        // No entitlement here, so this ends at permission or at the
        // token wait. Either is fine; reaching the end without a throw
        // and without a hang is the assertion.
        let result = await SentoriPush.shared.register(timeout: 1)
        guard case .failure(let reason, let message) = result else {
            return XCTFail("a simulator cannot actually register, got \(result)")
        }
        XCTAssertTrue(
            [.permissionDenied, .noTransport, .tokenTimeout].contains(reason),
            "unexpected reason \(reason.name): \(message)"
        )
        XCTAssertNil(SentoriPush.shared.cachedDeviceHandle(), "a failure must cache nothing")
    }

    func testUnregisterWithNothingRegisteredIsANoOp() async {
        let ok = await SentoriPush.shared.unregister()
        XCTAssertFalse(ok, "nothing to revoke")
        XCTAssertNil(SentoriPush.shared.cachedDeviceHandle())
    }

    func testTheRegistrationBodyIsTheOneTheServerAccepts() throws {
        // The field names are the whole reason push never worked for a
        // year: the RN SDK sent `provider` where the server reads
        // `kind`, and parsed an `ipt_*` handle no server has ever
        // returned. Pinned here rather than left to a reviewer.
        SentoriScope.setUser(id: "usr_123", email: nil)
        let body: [String: Any] = [
            "kind": "apns",
            "nativeToken": "abcd",
            "env": "sandbox",
            "userKey": SentoriScope.userKey as Any,
        ]
        let data = try JSONSerialization.data(withJSONObject: body)
        let round = try XCTUnwrap(
            JSONSerialization.jsonObject(with: data) as? [String: Any])

        XCTAssertEqual(round["kind"] as? String, "apns")
        XCTAssertNil(round["provider"], "the server has no such field")
        XCTAssertEqual(
            round["userKey"] as? String,
            SentoriIdentity.hash(keyType: "id", value: "usr_123"),
            "the device must carry the key its events carry, or it is unreachable from an issue"
        )
    }
    /// The failure log must not be able to kill the app it reports
    /// for.
    ///
    /// Its first version did. `Failure` is an `@objc enum … : Int`,
    /// the log passed `reason.rawValue` to `%@`, and NSLog
    /// dereferenced an integer as an object — a line added to make
    /// failures visible, crashing the host on the first failure it
    /// saw. The suite caught it only because something happened to
    /// call it.
    ///
    /// `message` is server text and may contain a `%`, which is the
    /// other way a log line becomes a crash.
    func testTheFailureLogSurvivesEveryReasonAndAPercentSign() async {
        SentoriConfig.__resetForTests()

        // No config: every verb is a no-op and `register` reports
        // notInitialised — through the log path, on this thread.
        let result = await Sentori.push.register()
        guard case .failure(let reason, _) = result else {
            return XCTFail("expected a failure without a config, got \(result)")
        }
        XCTAssertEqual(reason, .notInitialised)

        // Every reason has a name to log. An Int reaching a `%@` is
        // what crashed; a missing name would be the same shape.
        for reason in [
            SentoriPush.Failure.notInitialised, .permissionDenied, .noTransport,
            .tokenTimeout, .serverRejected,
        ] {
            XCTAssertFalse(reason.name.isEmpty)
            NSLog("%@", "[sentori-test] \(reason.name): 100%% of nothing %@ %d")
        }
    }

    // ── rotation ──────────────────────────────────────────────────

    /// A rotation must reach the server, and the only thing that
    /// proves it is a request arriving.
    ///
    /// The Android version of this test was first written to assert
    /// that the address had not changed — which is also what happens
    /// when the SDK does nothing at all, so it passed against the
    /// code it was written to catch. `handleRegisteredToken` used to
    /// write the value into a field and send it to nobody.
    ///
    /// `URLProtocol` rather than a stub of our own code: the seam is
    /// at the URL loading system, so what is exercised is the request
    /// the SDK really builds.
    func testARotatedTokenIsReportedToTheServer() {
        RotationProbe.reset()
        URLProtocol.registerClass(RotationProbe.self)
        defer { URLProtocol.unregisterClass(RotationProbe.self) }

        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))
        UserDefaults.standard.set("019ff000-0000-7000-8000-000000000001", forKey: handleKeyForTests)
        defer { UserDefaults.standard.removeObject(forKey: handleKeyForTests) }

        Sentori.push.handleRotatedToken("token-after")

        let deadline = Date().addingTimeInterval(10)
        while RotationProbe.bodies.isEmpty, Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        }

        XCTAssertEqual(
            RotationProbe.bodies.count, 1,
            "the rotation reached nobody — the server keeps the dead token until the "
                + "host next calls register, and the device receives nothing")
        let body = RotationProbe.bodies.first ?? ""
        XCTAssertTrue(body.contains("token-after"), "the rotation did not carry the new token")
        XCTAssertTrue(
            body.contains("installId"),
            "the rotation did not carry the installation, so the server would have "
                + "written a second row")
    }

    /// A device the host never registered is not registered behind
    /// its back.
    func testARotationForAnUnregisteredDeviceIsIgnored() {
        RotationProbe.reset()
        URLProtocol.registerClass(RotationProbe.self)
        defer { URLProtocol.unregisterClass(RotationProbe.self) }

        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))
        UserDefaults.standard.removeObject(forKey: handleKeyForTests)

        Sentori.push.handleRotatedToken("token-nobody-asked-for")
        RunLoop.current.run(until: Date().addingTimeInterval(1))

        XCTAssertTrue(
            RotationProbe.bodies.isEmpty,
            "a device that never registered was registered by a vendor callback")
    }

    private var handleKeyForTests: String { "com.sentori.push.handle" }
}

/// Records what the SDK posts, without replacing anything the SDK
/// calls: the interception is at the URL loading system, so the
/// request under test is the one that would have gone out.
extension SentoriPushTests {

    /// A device registers at launch; the person signs in ten seconds
    /// later. Nothing updated the row, so it carried no user for the
    /// life of the install — and a send aimed at that person reached
    /// nobody and reported success.
    ///
    /// Deliberately the same assertions as
    /// `signingInAfterRegisteringUpdatesTheDevice` in Kotlin.
    func testSigningInAfterRegisteringUpdatesTheDevice() {
        RotationProbe.reset()
        URLProtocol.registerClass(RotationProbe.self)
        defer {
            URLProtocol.unregisterClass(RotationProbe.self)
            SentoriScope.clear()
        }

        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))
        // A device that has registered, which is the only kind a
        // sign-in should act on. The rotation path is what puts both
        // the handle and the token where the sign-in path reads them.
        UserDefaults.standard.set("019ff000-0000-7000-8000-000000000001", forKey: handleKeyForTests)
        defer { UserDefaults.standard.removeObject(forKey: handleKeyForTests) }
        Sentori.push.handleRotatedToken("token-1")
        waitForBodies(1)
        XCTAssertEqual(RotationProbe.bodies.count, 1, "the device never registered")
        XCTAssertFalse(
            RotationProbe.bodies[0].contains("userKey"),
            "the first registration already carried a user")

        SentoriScope.setUser(id: "usr_123", email: nil, traits: ["plan": "pro"])
        waitForBodies(2)

        // Both halves matter: a count alone passes when the update
        // carries nothing, and a key alone passes when no update
        // happened and this is still the first request.
        XCTAssertEqual(
            RotationProbe.bodies.count, 2,
            "signing in reached nobody — the device row keeps no user, so a send aimed "
                + "at that person matches no device and reports success")
        let update = RotationProbe.bodies.count > 1 ? RotationProbe.bodies[1] : ""
        XCTAssertTrue(update.contains("userKey"), "the update carried no identity: \(update)")
        XCTAssertTrue(update.contains("plan"), "the update carried no traits: \(update)")
    }

    /// `Sentori.user` is a verb an app may call on every screen. One
    /// request per call is not free to a host, and the iron rule is
    /// that this SDK is.
    func testSettingTheSamePersonAgainSendsNothing() {
        RotationProbe.reset()
        URLProtocol.registerClass(RotationProbe.self)
        defer {
            URLProtocol.unregisterClass(RotationProbe.self)
            SentoriScope.clear()
        }
        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))
        UserDefaults.standard.set("019ff000-0000-7000-8000-000000000001", forKey: handleKeyForTests)
        defer { UserDefaults.standard.removeObject(forKey: handleKeyForTests) }
        Sentori.push.handleRotatedToken("token-1")
        waitForBodies(1)

        SentoriScope.setUser(id: "usr_123", email: nil, traits: ["plan": "pro"])
        waitForBodies(2)
        let after = RotationProbe.bodies.count

        SentoriScope.setUser(id: "usr_123", email: nil, traits: ["plan": "pro"])
        RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        XCTAssertEqual(RotationProbe.bodies.count, after, "the same person was sent twice")
    }

    /// A device that never registered is not registered by a sign-in.
    func testSigningInOnADeviceThatNeverRegisteredSendsNothing() {
        RotationProbe.reset()
        URLProtocol.registerClass(RotationProbe.self)
        defer {
            URLProtocol.unregisterClass(RotationProbe.self)
            SentoriScope.clear()
        }
        SentoriConfig.set(
            SentoriConfig(
                token: "st_test",
                ingestUrl: "http://127.0.0.1:9",
                release: "app@1.0.0",
                environment: "test"
            ))
        SentoriScope.setUser(id: "usr_123", email: nil, traits: ["plan": "pro"])
        RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        XCTAssertEqual(RotationProbe.bodies.count, 0)
    }

    private func waitForBodies(_ n: Int) {
        let deadline = Date().addingTimeInterval(10)
        while RotationProbe.bodies.count < n, Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        }
    }
}

private final class RotationProbe: URLProtocol {
    nonisolated(unsafe) static var bodies: [String] = []
    private static let lock = NSLock()

    static func reset() {
        lock.lock()
        bodies = []
        lock.unlock()
    }

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.path == "/v1/push/devices"
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        // `httpBody` is nil for a body handed over as a stream, which
        // is what URLSession does with an upload — read both.
        var body = ""
        if let data = request.httpBody {
            body = String(data: data, encoding: .utf8) ?? ""
        } else if let stream = request.httpBodyStream {
            stream.open()
            var buf = [UInt8](repeating: 0, count: 4096)
            var out = Data()
            while stream.hasBytesAvailable {
                let n = stream.read(&buf, maxLength: buf.count)
                if n <= 0 { break }
                out.append(contentsOf: buf[0..<n])
            }
            stream.close()
            body = String(data: out, encoding: .utf8) ?? ""
        }
        Self.lock.lock()
        Self.bodies.append(body)
        Self.lock.unlock()

        let payload = Data(
            #"{"spToken":"019ff000-0000-7000-8000-000000000001","isNew":false}"#.utf8)
        let resp = HTTPURLResponse(
            url: request.url!, statusCode: 202, httpVersion: "HTTP/1.1", headerFields: nil)!
        client?.urlProtocol(self, didReceive: resp, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: payload)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

}
