import Foundation

/// The only place the SDK talks to the network.
///
/// Ported from `sdk/react-native/src/transport.ts`, which is the
/// behaviour the server's e2e already asserts: batch on a 5 s timer or
/// a 10-deep queue, whichever comes first; three attempts with a
/// doubling delay; a 429 waits out `retryAfterMs`; anything left after
/// that spills to disk and drains on the next launch.
///
/// The iron rule is harder here than in JavaScript. A JS verb can
/// return immediately because everything after it is a microtask;
/// Swift has no such floor, so "fire and forget" has to be a real
/// queue on a real background thread. `enqueue` therefore does one
/// bounded append under a lock and returns — no encoding, no I/O, no
/// allocation beyond the event itself. Everything expensive happens on
/// `worker`.
///
/// Nothing in this file throws to the caller, and nothing blocks it.
@objc(SentoriTransport)
public final class SentoriTransport: NSObject {

    // Matching transport.ts, so a batch from an iOS app and a batch
    // from a React Native app look the same to the server and to the
    // person reading the dashboard.
    private static let flushInterval: TimeInterval = 5
    private static let batchSize = 10
    private static let maxRetry = 3
    private static let maxPersisted = 1000

    /// Bounded, because an unbounded in-memory queue is a leak with a
    /// nicer name. At the batch size above this is ~50 flushes of
    /// backlog; past it the oldest events go, since a crash from ten
    /// minutes ago matters less than the one happening now.
    private static let maxQueued = 500

    /// Per-attempt request timeout.
    static var requestTimeout: TimeInterval = 15

    /// Test seam: skip the network and return this outcome.
    ///
    /// The spill test tried twice to provoke a real failure by sending
    /// to a closed port. A laptop refuses it instantly; one CI runner
    /// dropped the packet and waited out the timeout three times over,
    /// and another answered in a way that read as a 4xx — which this
    /// transport treats as handled, so nothing spilled and the test
    /// failed on logic that was correct.
    ///
    /// What that test is about is the spill and the drain, not TCP.
    /// The network path has its own gate: `ios-live-ingest` sends to a
    /// real server and reads the events back.
    static var forcedOutcomeForTests: Int?

    private static let lock = NSLock()
    private static var queue: [[String: Any]] = []
    private static var assertStats: [String: [String: Any]] = [:]
    private static var timer: DispatchSourceTimer?
    private static var started = false
    private static var dropped = 0
    private static var delivered = 0

    private static let worker = DispatchQueue(label: "jp.golia.sentori.transport", qos: .utility)

    /// Work that may only run once the server has taken particular
    /// events.
    ///
    /// Attachments are the reason: the server keys one on an event id
    /// it must already know, so uploading before that event lands is a
    /// guaranteed 404.
    ///
    /// Keyed on ids rather than "the next delivery", which was the
    /// first version and had a race at each end. `flush` hands the
    /// send to a worker and returns, so registering afterwards can
    /// miss a batch that has already come back — on a fast network the
    /// uploads then never happen at all, silently, which is the bug
    /// this exists to prevent. Registering beforehand instead lets an
    /// unrelated batch already in flight fire the block early, into a
    /// 404. Matching on ids has neither end.
    private static var afterDelivery: [(ids: Set<String>, block: () -> Void)] = []

    /// Run `block` once the server has accepted every event in `ids`.
    /// If the server refuses them outright the block is discarded —
    /// they are not there to attach to. A batch that merely spilled
    /// keeps its waiters, since the drain retries it under the same
    /// ids.
    static func afterDelivery(of ids: Set<String>, _ block: @escaping () -> Void) {
        guard !ids.isEmpty else { return }
        lock.lock()
        afterDelivery.append((ids: ids, block: block))
        lock.unlock()
    }

    private static func settle(_ events: [[String: Any]], accepted: Bool) {
        let ids = Set(events.compactMap { $0["id"] as? String })
        guard !ids.isEmpty else { return }
        lock.lock()
        var ready: [() -> Void] = []
        var kept: [(ids: Set<String>, block: () -> Void)] = []
        for var entry in afterDelivery {
            if entry.ids.isDisjoint(with: ids) {
                kept.append(entry)
                continue
            }
            guard accepted else { continue }  // refused: drop the waiter
            // Subtract what landed rather than asking whether this one
            // batch carried everything. A waiter on two events whose
            // events go out in two batches is otherwise never due — it
            // is not a subset of either.
            entry.ids.subtract(ids)
            if entry.ids.isEmpty {
                ready.append(entry.block)
            } else {
                kept.append(entry)
            }
        }
        afterDelivery = kept
        lock.unlock()
        ready.forEach { $0() }
    }

    /// O(1) on the calling thread: append, maybe schedule. Everything
    /// else is the worker's problem.
    @objc public static func enqueue(_ event: [String: Any]) {
        lock.lock()
        queue.append(event)
        if queue.count > maxQueued {
            queue.removeFirst(queue.count - maxQueued)
            dropped += 1
        }
        let due = queue.count >= batchSize
        lock.unlock()

        if due {
            worker.async { flush() }
        } else {
            scheduleFlush(after: flushInterval)
        }
    }

    /// Assert outcomes aggregate rather than becoming events, and ride
    /// whatever batch goes out next — the liveness ledger without a
    /// heartbeat flood. A run with only passing asserts still reports,
    /// on a lazy timer six times the batch interval.
    @objc public static func countAssert(name: String, ok: Bool, release: String) {
        let key = "\(name)\u{1f}\(release)"
        lock.lock()
        var stat =
            assertStats[key] ?? ["name": name, "release": release, "passDelta": 0, "failDelta": 0]
        let field = ok ? "passDelta" : "failDelta"
        stat[field] = ((stat[field] as? Int) ?? 0) + 1
        assertStats[key] = stat
        let idle = queue.isEmpty
        lock.unlock()

        if idle { scheduleFlush(after: flushInterval * 6) }
    }

    @objc public static func start() {
        lock.lock()
        started = true
        lock.unlock()
        worker.async { drainPersisted() }
    }

    /// Send everything queued. Safe to call from anywhere; runs on the
    /// worker.
    @objc public static func flush() {
        // Before touching the queue, not after. The first version of
        // this drained into a local, cleared the queue, and only then
        // looked for a config — so anything enqueued before `start`
        // was silently destroyed by the first flush instead of waiting
        // for init. `transport.ts` checks in this order for the same
        // reason.
        guard let config = SentoriConfig.current else { return }

        lock.lock()
        guard started, !queue.isEmpty || !assertStats.isEmpty else {
            lock.unlock()
            return
        }
        let events = queue
        let stats = Array(assertStats.values)
        let lost = dropped
        queue.removeAll(keepingCapacity: true)
        assertStats.removeAll(keepingCapacity: true)
        dropped = 0
        timer?.cancel()
        timer = nil
        lock.unlock()

        var envelope: [String: Any] = ["events": events]
        if !stats.isEmpty { envelope["assertStats"] = stats }
        if let health = config.backendHealthUrl { envelope["backendHealthUrl"] = health }
        // Say so rather than let the gap look like quiet. A backlog
        // that overflowed is a fact about the device, and hiding it
        // makes the next person read a hole in the timeline as calm.
        if lost > 0 { envelope["droppedEvents"] = lost }

        worker.async {
            switch sendWithRetry(envelope, config: config) {
            case .delivered:
                lock.lock()
                delivered += events.count
                lock.unlock()
                settle(events, accepted: true)
            case .dropped:
                // Handled, but not accepted. Nothing to retry and
                // nothing to count.
                settle(events, accepted: false)
            default:
                // Spilled, not lost: `drainPersisted` puts these back
                // through this path on the next start with the same
                // ids, so anything waiting on them keeps waiting
                // rather than being discarded here.
                persist(events)
            }
        }
    }

    // ── the network ───────────────────────────────────────────────

    /// Returns the terminal outcome rather than a bool.
    ///
    /// It used to return "stop trying", which conflated *accepted*
    /// with *rejected for good reason*: a 4xx is not worth retrying,
    /// but it is also not delivery. With one bool, a wrong token
    /// looked exactly like a successful send — the live suite passed
    /// against a server that had stored nothing, and only a readback
    /// through the admin API noticed.
    private static func sendWithRetry(_ envelope: [String: Any], config: SentoriConfig) -> Outcome
    {
        var delay: TimeInterval = 1
        for attempt in 1...maxRetry {
            let outcome = sendOnce(envelope, config: config)
            switch outcome {
            case .delivered, .dropped:
                // A 4xx that is not 429 is our request being wrong.
                // Retrying produces the same 4xx forever, so it ends
                // here rather than spilling and trying again on every
                // launch.
                return outcome
            case .retryAfter(let wait):
                if attempt == maxRetry { return outcome }
                Thread.sleep(forTimeInterval: wait)
            case .failed:
                if attempt == maxRetry { return outcome }
                Thread.sleep(forTimeInterval: delay)
                delay *= 2
            }
        }
        return .failed
    }

    private enum Outcome {
        case delivered
        case dropped
        case retryAfter(TimeInterval)
        case failed
    }

    private static func sendOnce(_ envelope: [String: Any], config: SentoriConfig) -> Outcome {
        if let forced = forcedOutcomeForTests {
            return forced == 0 ? .delivered : (forced == 1 ? .dropped : .failed)
        }
        // `JSONSerialization` raises `NSInvalidArgumentException` for a
        // NaN or an infinity — an Objective-C exception, which `try?`
        // does not catch and Swift cannot. One `Double.nan` in one
        // event's data would terminate the host app: the SDK crashing
        // the app instead of reporting the app's crash, which is the
        // failure-isolation rule broken as completely as it can be.
        //
        // `isValidJSONObject` answers the same question without
        // raising, and `scrubbed` replaces what it objects to so a
        // single bad value costs one field rather than the batch.
        let safe = JSONSerialization.isValidJSONObject(envelope) ? envelope : scrubbed(envelope)
        guard let url = URL(string: "\(config.ingestUrl)/v1/events:batch"),
            JSONSerialization.isValidJSONObject(safe),
            let body = try? JSONSerialization.data(withJSONObject: safe)
        else { return .dropped }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(config.token)", forHTTPHeaderField: "Authorization")
        request.setValue("swift/\(SentoriVersion.current)", forHTTPHeaderField: "Sentori-Sdk")
        request.httpBody = body
        request.timeoutInterval = requestTimeout

        // The worker thread is ours and is not the caller's, so
        // blocking it is fine and keeps the retry loop readable.
        let semaphore = DispatchSemaphore(value: 0)
        var outcome = Outcome.failed
        let task = URLSession.shared.dataTask(with: request) { data, response, _ in
            defer { semaphore.signal() }
            guard let http = response as? HTTPURLResponse else { return }
            switch http.statusCode {
            case 200..<300:
                outcome = .delivered
            case 429:
                var wait: TimeInterval = 5
                if let data,
                    let j = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                    let ms = j["retryAfterMs"] as? Double
                {
                    wait = ms / 1000
                }
                outcome = .retryAfter(wait)
            case 500...:
                outcome = .failed
            default:
                outcome = .dropped
            }
        }
        task.resume()
        _ = semaphore.wait(timeout: .now() + 30)
        return outcome
    }

    /// Replace anything `JSONSerialization` refuses with something it
    /// accepts, keeping the field so the reader can see what happened
    /// rather than finding a hole.
    ///
    /// Non-finite numbers become their names; a type with no JSON
    /// equivalent — a `Date`, a `URL`, a host's own struct — becomes
    /// its description, which is more use than dropping the event.
    private static func scrubbed(_ value: Any) -> Any {
        switch value {
        case let dict as [String: Any]:
            return dict.mapValues { scrubbed($0) }
        case let array as [Any]:
            return array.map { scrubbed($0) }
        case let number as Double:
            if number.isNaN { return "NaN" }
            if number.isInfinite { return number > 0 ? "Infinity" : "-Infinity" }
            return number
        case let number as Float:
            return scrubbed(Double(number))
        case is String, is Int, is Bool, is NSNull:
            return value
        case let number as NSNumber:
            return scrubbed(number.doubleValue)
        default:
            return String(describing: value)
        }
    }

    // ── the offline queue ─────────────────────────────────────────

    private static var spillURL: URL? {
        guard
            let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
                .first
        else { return nil }
        let sentori = dir.appendingPathComponent("sentori", isDirectory: true)
        try? FileManager.default.createDirectory(at: sentori, withIntermediateDirectories: true)
        return sentori.appendingPathComponent("pending-events.json")
    }

    private static func persist(_ events: [[String: Any]]) {
        guard let url = spillURL else { return }
        var all = readPersisted()
        all.append(contentsOf: events)
        // Newest wins: the same reasoning as the in-memory cap, and
        // the file has to stop growing on a device that is offline for
        // a week.
        if all.count > maxPersisted { all.removeFirst(all.count - maxPersisted) }
        // Same raise as the send path: a NaN here would terminate the
        // app while writing the file whose whole job is to survive a
        // failure. `try?` does not catch an Objective-C exception.
        let safe = scrubbed(all)
        guard JSONSerialization.isValidJSONObject(safe),
            let data = try? JSONSerialization.data(withJSONObject: safe)
        else { return }
        try? data.write(to: url, options: .atomic)
    }

    private static func readPersisted() -> [[String: Any]] {
        guard let url = spillURL, let data = try? Data(contentsOf: url),
            let all = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else { return [] }
        return all
    }

    /// Called once on start. Reads the spill, clears it, and puts the
    /// events back through the normal path — clearing first, so a
    /// batch that fails again spills once rather than doubling.
    private static func drainPersisted() {
        let pending = readPersisted()
        guard !pending.isEmpty, let url = spillURL else { return }
        try? FileManager.default.removeItem(at: url)
        for event in pending { enqueue(event) }
        flush()
    }

    // ── timer ─────────────────────────────────────────────────────

    private static func scheduleFlush(after seconds: TimeInterval) {
        lock.lock()
        defer { lock.unlock() }
        guard timer == nil else { return }
        let t = DispatchSource.makeTimerSource(queue: worker)
        t.schedule(deadline: .now() + seconds)
        t.setEventHandler {
            lock.lock()
            timer = nil
            lock.unlock()
            flush()
        }
        timer = t
        t.resume()
    }

    // ── test seams ────────────────────────────────────────────────

    static func __resetForTests() {
        // Wait for the worker before clearing anything. A flush
        // dispatched by the previous test can still be in
        // `sendWithRetry`, and its `persist` then writes the spill
        // file *after* this deleted it — so the next test's `start`
        // drains that file, flushes, and carries off assert counters
        // that belonged to it. That is what turned `passDelta` into 1
        // on CI while it stayed 2 here.
        //
        // `worker.sync {}` deadlocks: `drainPersisted` and the send
        // both run *on* this queue and can reach here, and a
        // same-queue sync waits for itself. A semaphore posted from an
        // async block waits from either side, and gives up rather than
        // hanging a suite if the worker is wedged.
        let drained = DispatchSemaphore(value: 0)
        worker.async { drained.signal() }
        _ = drained.wait(timeout: .now() + 10)

        lock.lock()
        queue.removeAll()
        assertStats.removeAll()
        dropped = 0
        delivered = 0
        afterDelivery.removeAll()
        started = false
        requestTimeout = 15
        forcedOutcomeForTests = nil
        timer?.cancel()
        timer = nil
        lock.unlock()
        if let url = spillURL { try? FileManager.default.removeItem(at: url) }
    }

    static func __peekQueue() -> [[String: Any]] {
        lock.lock()
        defer { lock.unlock() }
        return queue
    }

    static func __peekAssertStats() -> [[String: Any]] {
        lock.lock()
        defer { lock.unlock() }
        return Array(assertStats.values)
    }

    static func __peekPersisted() -> [[String: Any]] { readPersisted() }

    /// Events the server actually accepted. A test that checks only
    /// for the *absence* of a spill passes while three retries are
    /// still in flight — which is how the live suite went green on CI
    /// against a server that had stored nothing.
    /// How many blocks are still waiting on events. Without this a
    /// test cannot tell "discarded because the server refused it" from
    /// "has not been reached yet", and the version that tried instead
    /// flipped the forced outcome mid-send — measuring the race rather
    /// than the rule.
    static func __peekWaiting() -> Int {
        lock.lock()
        defer { lock.unlock() }
        return afterDelivery.count
    }

    static func __peekDelivered() -> Int {
        lock.lock()
        defer { lock.unlock() }
        return delivered
    }
}
