import Foundation

/// The crash that killed the app, sent on the next launch.
///
/// `SentoriCrashHandler` writes one JSON file per crash from inside an
/// `NSUncaughtExceptionHandler`, where almost nothing is safe to do —
/// so it writes a flat, older shape and gets out. Something has to
/// read those files, convert them to the wire format and enqueue
/// them, and until now nothing in this package did: the crash handler
/// filled a directory nobody emptied.
///
/// React Native has had this since the beginning, in
/// `sdk/react-native/src/native-pending.ts`. This is the same
/// conversion, so a crash from a Swift app and a crash from an RN app
/// arrive identical — the dashboard cannot tell which SDK sent one and
/// should not have to.
///
/// Attachments captured before death (a screenshot, a replay ring) are
/// carried in the file and are **not** uploaded yet: that needs the
/// attachment endpoint, which this package does not speak. They are
/// left in place rather than silently dropped, and the event ships
/// without them — a crash report with no screenshot beats no crash
/// report.
enum SentoriPendingCrash {

    /// Drain, convert, enqueue. Called once by `Sentori.start`.
    static func ship() {
        let files = SentoriCrashHandler.consumePending()
        guard !files.isEmpty else { return }

        for text in files {
            guard let data = text.data(using: .utf8),
                var raw = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
            else {
                // One corrupt file must not cost the others.
                continue
            }
            // The screenshot and replay ring the handler captured as
            // the app died. They travel in the file and never on the
            // wire — the server takes them separately, keyed on an
            // event it must already have.
            let blobs = raw.removeValue(forKey: "_pendingAttachments") as? [[String: Any]] ?? []
            let wire = toWire(raw)

            // Registered before this event is queued, and keyed on its
            // id. Two races close here, and one of them was real: the
            // first version registered after `flush`, which hands the
            // send to a worker and returns — so on a fast network the
            // batch was accepted before the block existed and the
            // attachments never uploaded at all. Registering before
            // the loop's flush is not enough either, since `enqueue`
            // sends of its own accord once ten events are queued, and
            // a run with ten crash files would flush mid-loop.
            if !blobs.isEmpty, let id = wire["id"] as? String {
                SentoriTransport.afterDelivery(of: [id]) {
                    for blob in blobs {
                        guard let kind = blob["kind"] as? String,
                            let base64 = blob["base64"] as? String
                        else { continue }
                        SentoriAttachment.upload(
                            eventId: id,
                            kind: kind,
                            base64: base64,
                            mediaType: blob["mediaType"] as? String ?? "application/octet-stream",
                            source: blob["source"] as? String ?? "ios"
                        )
                    }
                }
            }
            SentoriTransport.enqueue(wire)
        }
        SentoriTransport.flush()
    }

    /// The flat on-disk shape into the nested wire one. Mirrors
    /// `toWire` in `native-pending.ts` field for field.
    static func toWire(_ raw: [String: Any]) -> [String: Any] {
        let rawError = raw["error"] as? [String: Any] ?? [:]
        let frames = (rawError["stack"] as? [[String: Any]] ?? []).map { f -> [String: Any] in
            var out: [String: Any] = [:]
            if let file = f["file"] as? String { out["file"] = file }
            if let fn = f["function"] as? String { out["function"] = fn }
            if let line = f["line"] as? Int { out["line"] = line }
            if let inApp = f["inApp"] as? Bool { out["inApp"] = inApp }
            return out
        }

        var payload: [String: Any] = [
            "error": [
                // A crash with no type is still a crash. The default
                // names what it is rather than leaving the issue title
                // empty.
                "type": rawError["type"] as? String ?? "NativeCrash",
                "message": rawError["message"] as? String ?? "",
                "stack": frames,
            ],
            // Tells the dashboard this arrived from the grave rather
            // than from a caught error, which is the difference between
            // "the app died" and "the app noticed".
            "nativeCrash": true,
        ]
        if let device = raw["device"] as? [String: Any] { payload["device"] = device }
        if let app = raw["app"] as? [String: Any] { payload["app"] = app }

        return [
            "id": raw["id"] as? String ?? Sentori.newEventId(),
            "kind": "error",
            // The file says `timestamp`; the wire says `occurredAt`.
            // Keeping the crash's own time matters — it is the moment
            // the app died, not the moment the next launch noticed.
            "occurredAt": raw["timestamp"] as? String ?? Sentori.iso8601(Date()),
            "platform": raw["platform"] as? String == "android" ? "android" : "ios",
            "release": raw["release"] as? String ?? "",
            "environment": raw["environment"] as? String ?? "",
            "payload": payload,
        ]
    }
}
