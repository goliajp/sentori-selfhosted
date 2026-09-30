import Foundation

/// Push, as an app writes it.
///
///     let r = await Sentori.push.register()
///     if case .failure(let reason) = r { … }
///
/// The pieces underneath already existed — permission, the
/// `AppDelegate` swizzle, the token buffer, the tap and foreground
/// callbacks — and were reachable only through the React Native
/// bridge. What was missing everywhere but JavaScript is the part
/// that puts a token on the server, which is the only reason a
/// registered device is reachable at all.
///
/// Asked for by insight (2026-08-11): two apps with no React Native,
/// blocked, and unwilling to reimplement the HTTP contract because a
/// second implementation drifts silently on the one path where nobody
/// is watching.
@objc(SentoriPush)
public final class SentoriPush: NSObject {

    /// Why a registration did not produce a device handle. Each value
    /// asks the host for something different, which is the only
    /// reason to distinguish them — matching `PushRegisterFailure` in
    /// the React Native SDK exactly, so the same integration notes
    /// apply on both.
    @objc public enum Failure: Int {
        /// `Sentori.start` has not run. A wiring bug.
        case notInitialised
        /// The user said no. Not an error: do not retry on a timer,
        /// and offer it again from a settings screen.
        case permissionDenied
        /// No push entitlement in this build, or a simulator without
        /// one. Nothing to do at runtime.
        case noTransport
        /// The OS never handed back a token inside the window.
        /// Usually provisioning; retrying later is reasonable.
        case tokenTimeout
        /// Sentori answered non-2xx. Settings ▸ Push is where to look.
        case serverRejected

        public var name: String {
            switch self {
            case .notInitialised: return "not-initialised"
            case .permissionDenied: return "permission-denied"
            case .noTransport: return "no-transport"
            case .tokenTimeout: return "token-timeout"
            case .serverRejected: return "server-rejected"
            }
        }
    }

    /// Registration never throws. A denied permission is an ordinary
    /// answer, and an opt-in that throws inside someone's view model
    /// is the failure this SDK's contract with its host is written
    /// against.
    public enum Result {
        /// The `device_tokens` row id. Revoking takes it, and so does
        /// a targeted send.
        case success(handle: String)
        case failure(reason: Failure, message: String)

        public var handle: String? {
            if case .success(let h) = self { return h }
            return nil
        }
    }

    @objc public static let shared = SentoriPush()

    private let lock = NSLock()
    private var cachedHandle: String?
    private var onMessage: (([String: Any]) -> Void)?
    private var onTap: (([String: Any]) -> Void)?
    private var drainTimer: DispatchSourceTimer?

    private static let handleKey = "com.sentori.push.handle"

    /// Which installation this is, kept for as long as the app is
    /// installed.
    ///
    /// The server keys the device row on it, so APNs issuing a new
    /// token — a reinstall, a restore from backup — becomes an update
    /// of the row that already exists rather than a new row with a
    /// new address. Before this, a rotation silently retired whatever
    /// `spToken` a backend was holding.
    ///
    /// Minted here rather than issued by the server because it has to
    /// exist before the first registration, and it never leaves the
    /// device except in that registration: it is not the address, and
    /// an identifier that can claim a row must not be one that
    /// travels through logs and other people's databases.
    private static let installKey = "com.sentori.push.install"

    /// The last token APNs issued, so a sign-in can send the
    /// registration again without waiting on the OS for another.
    ///
    /// It is already in this app's container; keeping it costs nothing
    /// new. The handle stays the capability — this is only an input to
    /// producing one.
    private static let tokenKey = "com.sentori.push.native"

    /// What the server was last told, so a repeat is not sent.
    private var lastSentIdentity: String?

    static var installId: String {
        if let existing = UserDefaults.standard.string(forKey: installKey) {
            return existing
        }
        let fresh = UUID().uuidString.lowercased()
        UserDefaults.standard.set(fresh, forKey: installKey)
        return fresh
    }

    /// Ask for permission, get a token, register it.
    ///
    /// Safe to call on every launch: the OS returns its cached
    /// decision without re-prompting, and the server upserts on
    /// `(project, provider, token)`.
    ///
    /// Call `Sentori.user` first if the device should be reachable
    /// from an issue. Without it the registration carries no user key
    /// and the device receives broadcasts only — the dashboard shows
    /// that as "N devices, 0 addressable", which is the one symptom
    /// with no other explanation.
    @discardableResult
    public func register(
        timeout: TimeInterval = 8,
        onMessage: (([String: Any]) -> Void)? = nil,
        onTap: (([String: Any]) -> Void)? = nil
    ) async -> Result {
        guard let config = SentoriConfig.current else {
            return report(.failure(reason: .notInitialised, message: "Sentori.start has not run"))
        }

        // Bind before asking for anything: iOS replays a tap that
        // happened while the app was not running as soon as the
        // delegate attaches, and a callback set afterwards misses it.
        lock.lock()
        self.onMessage = onMessage
        self.onTap = onTap
        lock.unlock()

        let status = await currentOrRequestPermission()
        // "unavailable" is the native layer saying there is no app
        // bundle to ask on behalf of — a test host, a CLI, some
        // extensions. Not a decision the user made.
        guard let status, status != "unavailable" else {
            return report(
                .failure(reason: .noTransport, message: "no push support in this build"))
        }
        guard ["granted", "provisional", "ephemeral"].contains(status) else {
            return report(
                .failure(reason: .permissionDenied, message: "push permission '\(status)'"))
        }

        SentoriPushNotifications.shared.registerForRemoteNotifications()
        guard let token = await waitForToken(timeout: timeout) else {
            return report(
                .failure(
                    reason: .tokenTimeout,
                    message: "no device token within \(Int(timeout))s"
                ))
        }

        switch await registerWithServer(token: token, config: config) {
        case .success(let handle):
            remember(handle: handle, token: token)
            startDrain()
            return .success(handle: handle)
        case .failure(let reason, let message):
            return report(.failure(reason: reason, message: message))
        }
    }

    /// Say it out loud, once, where the person wiring this up is
    /// looking.
    ///
    /// A failed `register` reported only to the server is invisible
    /// on the machine where the mistake was made: the integrator has
    /// to finish connecting the dashboard before it can tell them
    /// they have not finished connecting the dashboard. insight found
    /// their first-launch failure by adding a `Log.w` of their own
    /// and taking it out again.
    ///
    /// Warning, never error. A red line in someone else's console
    /// reads as "your app is broken", and a host team that believes
    /// that pulls the SDK out.
    private func report(_ result: Result) -> Result {
        if case .failure(let reason, let message) = result {
            // One `%@` and one already-built string, on purpose.
            //
            // The first version was `NSLog("… (%@): %@", reason.rawValue, message)`
            // and it killed the test host on the first call: `Failure`
            // is an `@objc enum … : Int`, so `rawValue` is an Int, and
            // `%@` dereferenced it as an object pointer. A logging
            // line, added to make failures visible, crashing the app
            // it reports for — the isolation rule broken as completely
            // as it can be.
            //
            // `message` is not a format string either. It can carry
            // server text, and server text can contain a `%`.
            NSLog("%@", "[sentori] push register failed (\(reason.name)): \(message)")
        }
        return result
    }

    /// APNs has issued this device a new token; tell the server now
    /// rather than at the next launch.
    ///
    /// The swizzled AppDelegate callback wrote the value into a field
    /// and nothing sent it. So from the moment a token rotated until
    /// the host next called `register`, the server held a dead token:
    /// sends went out, APNs answered with an unregistered device,
    /// quarantine retired the row, and it came back at the next launch
    /// under a different address. For an app that stays resident,
    /// "the next launch" is not a bounded wait.
    ///
    /// Only re-registers a device that has registered before — a
    /// stored `spToken` is the evidence. A token arriving for a device
    /// the host never registered is not something to act on unasked.
    func handleRotatedToken(_ token: String) {
        guard let config = SentoriConfig.current,
            UserDefaults.standard.string(forKey: Self.handleKey) != nil
        else { return }
        Task {
            switch await registerWithServer(token: token, config: config) {
            case .success(let handle):
                remember(handle: handle, token: token)
            case .failure(let reason, let message):
                NSLog(
                    "%@",
                    "[sentori] push re-register after token rotation failed "
                        + "(\(reason.name)): \(message)")
            }
        }
    }

    /// Keep the address and the token it came from, and start
    /// following the person.
    ///
    /// Three callers — the first registration, a rotation, and a
    /// sign-in — and three copies of four lines is how two of them end
    /// up writing different keys.
    private func remember(handle: String, token: String) {
        lock.lock()
        cachedHandle = handle
        lock.unlock()
        UserDefaults.standard.set(handle, forKey: Self.handleKey)
        UserDefaults.standard.set(token, forKey: Self.tokenKey)
        // Installing this replays a sign-in that happened while the
        // registration was in flight — it announced to nobody, and
        // `SentoriScope` holds it until someone is listening.
        SentoriScope.setIdentityListener { [weak self] in self?.identityChanged() }
    }

    /// One string standing for "who this device belongs to", used
    /// only to answer "has it changed since the last request".
    ///
    /// Keys sorted: a `[String: Any]` prints in hash order, and two
    /// dictionaries holding the same pairs can print them in different
    /// orders. Comparing those descriptions would report a change that
    /// did not happen and send a registration per call of a verb a
    /// host may call on every screen — which is the cost this
    /// comparison exists to avoid.
    private static func identityString(userKey: String?, traits: [String: Any]?) -> String {
        let rendered = (traits ?? [:])
            .keys.sorted()
            .map { "\($0)=\(String(describing: traits?[$0] ?? ""))" }
            .joined(separator: "\u{1}")
        return "\(userKey ?? "-")\u{0}\(rendered)"
    }

    /// Send the registration again because the person changed.
    ///
    /// Only for a device that has already registered — a stored token
    /// is the evidence, the same rule `handleRotatedToken` uses.
    /// Returns immediately: `Sentori.user` is synchronous and stays
    /// that way.
    private func identityChanged() {
        guard let config = SentoriConfig.current,
            let token = UserDefaults.standard.string(forKey: Self.tokenKey),
            UserDefaults.standard.string(forKey: Self.handleKey) != nil
        else { return }

        // `Sentori.user` is a verb an app may call on every screen,
        // and one request per call is not free to a host.
        let identity = Self.identityString(
            userKey: SentoriScope.userKey, traits: SentoriScope.traits)
        lock.lock()
        let repeated = identity == lastSentIdentity
        if !repeated { lastSentIdentity = identity }
        lock.unlock()
        if repeated { return }

        Task {
            switch await registerWithServer(token: token, config: config) {
            case .success(let handle):
                remember(handle: handle, token: token)
            case .failure(let reason, let message):
                // The row still names the previous person, so the next
                // change has to be allowed to try again.
                lock.lock()
                lastSentIdentity = nil
                lock.unlock()
                NSLog(
                    "%@",
                    "[sentori] updating the device after a sign-in failed "
                        + "(\(reason.name)): \(message)")
            }
        }
    }

    /// The handle from an earlier `register`, without a round trip.
    @objc public func cachedDeviceHandle() -> String? {
        lock.lock()
        let h = cachedHandle
        lock.unlock()
        return h ?? UserDefaults.standard.string(forKey: Self.handleKey)
    }

    /// Current permission without prompting. `nil` when there is no
    /// push support in this build.
    public func permissionStatus() async -> String? {
        await withCheckedContinuation { cont in
            SentoriPushNotifications.shared.currentPermission { cont.resume(returning: $0) }
        }
    }

    /// Revoke the handle server-side and stop local delivery.
    /// Idempotent — repeat calls do nothing.
    @discardableResult
    public func unregister() async -> Bool {
        let handle = cachedDeviceHandle()
        lock.lock()
        cachedHandle = nil
        onMessage = nil
        onTap = nil
        drainTimer?.cancel()
        drainTimer = nil
        lock.unlock()
        UserDefaults.standard.removeObject(forKey: Self.handleKey)
        UserDefaults.standard.removeObject(forKey: Self.tokenKey)
        // Nothing to follow the person to any more; a later sign-in
        // must not resurrect a device the host just revoked.
        SentoriScope.setIdentityListener(nil)
        lock.lock()
        lastSentIdentity = nil
        lock.unlock()
        SentoriPushNotifications.shared.unregisterForRemoteNotifications()

        guard let handle, let config = SentoriConfig.current,
            let url = URL(string: "\(config.ingestUrl)/v1/push/devices/\(handle)")
        else { return false }

        var request = URLRequest(url: url)
        request.httpMethod = "DELETE"
        request.setValue("Bearer \(config.token)", forHTTPHeaderField: "Authorization")
        request.timeoutInterval = 15
        let ok = await withCheckedContinuation { (cont: CheckedContinuation<Bool, Never>) in
            URLSession.shared.dataTask(with: request) { _, response, _ in
                let code = (response as? HTTPURLResponse)?.statusCode ?? 0
                cont.resume(returning: (200..<300).contains(code))
            }.resume()
        }
        return ok
    }

    // ── internals ─────────────────────────────────────────────────

    private func currentOrRequestPermission() async -> String? {
        let current = await permissionStatus()
        // `nil` means no native push at all; asking would not change
        // that. Anything already decided is returned as decided,
        // because a second prompt is not something iOS shows anyway.
        guard let current else { return nil }
        if current == "undetermined" || current == "notDetermined" {
            return await withCheckedContinuation { cont in
                SentoriPushNotifications.shared.requestPermission { cont.resume(returning: $0) }
            }
        }
        return current
    }

    /// The token arrives asynchronously through the AppDelegate
    /// swizzle and lands in the native buffer, so this polls it. Any
    /// notifications or taps that arrive alongside are delivered
    /// rather than dropped on the floor.
    private func waitForToken(timeout: TimeInterval) async -> String? {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let state = SentoriPushNotifications.shared.drainState()
            flush(state)
            if let token = state["token"] as? String { return token }
            if state["error"] != nil { return nil }
            try? await Task.sleep(nanoseconds: 200_000_000)
        }
        return nil
    }

    private func registerWithServer(token: String, config: SentoriConfig) async -> Result {
        guard let url = URL(string: "\(config.ingestUrl)/v1/push/devices") else {
            return .failure(reason: .serverRejected, message: "bad ingest url")
        }
        var body: [String: Any] = [
            // `kind`, not `provider`. The React Native SDK sent
            // `provider` for a year and earned a 422 for every
            // registration it ever attempted.
            "kind": "apns",
            "nativeToken": token,
            // Sandbox and production APNs are different hosts, and a
            // token minted against one is rejected by the other.
            "env": isDebugBuild ? "sandbox" : "production",
            // Which installation this is. The server keys the row on
            // it, so a rotated token updates this device rather than
            // creating a second one under a new address.
            "installId": Self.installId,
        ]
        // Read once. The body and the record of what the body carried
        // have to describe the same person, and two reads of a value
        // the host can change from any thread do not.
        let userKey = SentoriScope.userKey
        let traits = SentoriScope.traits
        // The same salted-nothing hash every event carries, so the
        // dashboard can address this device by the person who hit an
        // issue. Absent until the host calls `Sentori.user`.
        if let userKey { body["userKey"] = userKey }
        // Attributes of the person rather than of the device, kept
        // apart so a build channel called "pro" cannot answer a send
        // aimed at the pro plan. Absent leaves the row's traits alone;
        // an empty object clears them, which is what signing out sends.
        if let traits { body["traits"] = traits }
        // What this request puts on the wire, recorded before it goes.
        // `identityChanged` reads it back to decide whether the person
        // has changed since — including while this very request was in
        // flight, which is the window a host hits by calling
        // `Sentori.user` right after starting push registration.
        lock.lock()
        lastSentIdentity = Self.identityString(userKey: userKey, traits: traits)
        lock.unlock()

        guard let payload = try? JSONSerialization.data(withJSONObject: body) else {
            return .failure(reason: .serverRejected, message: "could not encode registration")
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(config.token)", forHTTPHeaderField: "Authorization")
        request.setValue("swift/\(SentoriVersion.current)", forHTTPHeaderField: "Sentori-Sdk")
        request.httpBody = payload
        request.timeoutInterval = 15

        return await withCheckedContinuation { (cont: CheckedContinuation<Result, Never>) in
            URLSession.shared.dataTask(with: request) { data, response, error in
                let code = (response as? HTTPURLResponse)?.statusCode ?? 0
                guard (200..<300).contains(code) else {
                    let why = error?.localizedDescription ?? "HTTP \(code)"
                    cont.resume(
                        returning: .failure(reason: .serverRejected, message: why))
                    return
                }
                // The handle is the `device_tokens` row id, a bare
                // uuid. The RN SDK parsed it as an `ipt_*` string no
                // server has ever returned.
                // The address, by its one name.
                guard let data,
                    let j = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                    let handle = j["spToken"] as? String,
                    !handle.isEmpty
                else {
                    cont.resume(
                        returning: .failure(
                            reason: .serverRejected,
                            message: "server returned no device token id"))
                    return
                }
                cont.resume(returning: .success(handle: handle))
            }.resume()
        }
    }

    /// 1 Hz while registered. The native side buffers arrivals and
    /// taps; this hands them to the host.
    private func startDrain() {
        lock.lock()
        defer { lock.unlock() }
        guard drainTimer == nil else { return }
        let t = DispatchSource.makeTimerSource(queue: .global(qos: .utility))
        t.schedule(deadline: .now() + 1, repeating: 1)
        t.setEventHandler { [weak self] in
            guard let self else { return }
            self.flush(SentoriPushNotifications.shared.drainState())
        }
        drainTimer = t
        t.resume()
    }

    private func flush(_ state: [String: Any]) {
        lock.lock()
        let message = onMessage
        let tap = onTap
        lock.unlock()
        guard message != nil || tap != nil else { return }

        let notifications = state["notifications"] as? [[String: Any]] ?? []
        let taps = state["taps"] as? [[String: Any]] ?? []
        guard !notifications.isEmpty || !taps.isEmpty else { return }

        // The host's UI lives on the main thread and a notification
        // callback almost always touches it.
        DispatchQueue.main.async {
            notifications.forEach { message?($0) }
            taps.forEach { tap?($0) }
        }
    }

    private var isDebugBuild: Bool {
        #if DEBUG
            return true
        #else
            return false
        #endif
    }

    func __resetForTests() {
        lock.lock()
        cachedHandle = nil
        onMessage = nil
        onTap = nil
        drainTimer?.cancel()
        drainTimer = nil
        lock.unlock()
        UserDefaults.standard.removeObject(forKey: Self.handleKey)
        UserDefaults.standard.removeObject(forKey: Self.tokenKey)
        // Nothing to follow the person to any more; a later sign-in
        // must not resurrect a device the host just revoked.
        SentoriScope.setIdentityListener(nil)
        lock.lock()
        lastSentIdentity = nil
        lock.unlock()
    }
}

extension Sentori {
    /// `Sentori.push.register()`, matching `sentori.push.register()`
    /// in the React Native SDK.
    public static var push: SentoriPush { SentoriPush.shared }
}
