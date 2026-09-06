import Foundation

/// Blobs that belong to an event: the screenshot taken as the app
/// died, the wireframe replay ring behind it.
///
/// They upload *after* the event, never before. The server keys an
/// attachment on an event id it must already know, so an upload that
/// races the batch 404s — and it always wins that race, because the
/// batch waits for a flush and the upload does not.
///
/// The multipart body is built by hand rather than with a form API.
/// React Native learned that the expensive way: its `FormData` file
/// part wants a `uri`, the `data:` form throws a bare network error on
/// iOS, and every JS attachment died silently for a release. Doing it
/// literally is both shorter and checkable.
enum SentoriAttachment {

    /// The kinds the server's CHECK constraint accepts, from
    /// `handlers/sdk/events_attachments.rs`. Anything else is a 400,
    /// so it is dropped here with the round trip saved.
    ///
    /// This list is worth keeping honest: the first version of it had
    /// three entries, and the crash handler's view tree — written on
    /// both platforms, kind `viewTree` — was not one of them. It
    /// would have been dropped on the way out by the very code meant
    /// to deliver it, silently, with the event arriving intact and
    /// the evidence missing.
    static let known: Set<String> = [
        "logTail", "replay", "screens", "screenshot",
        "sessionTrail", "stateSnapshot", "viewTree",
    ]

    /// `android` | `ios` | `js`, likewise CHECK-constrained.
    static let knownSources: Set<String> = ["android", "ios", "js"]

    /// Test seam: capture instead of sending.
    ///
    /// What is worth asserting here is *what* would be uploaded and
    /// *when* — the kind, the event it is keyed on, the body, and
    /// that none of it moves before the batch lands. None of that is
    /// about TCP. The wire itself is covered by `ios-live-ingest`,
    /// which posts to a real server and reads the blob back.
    static var recorderForTests: ((String, String, String) -> Void)?

    /// Upload one blob against an event the server already has.
    ///
    /// Fire and forget: nothing here reaches the caller, and a
    /// failure costs the attachment rather than the crash report it
    /// belongs to.
    static func upload(
        eventId: String,
        kind: String,
        base64: String,
        mediaType: String,
        source: String = "ios",
        completion: ((Bool) -> Void)? = nil
    ) {
        guard let config = SentoriConfig.current, known.contains(kind),
            knownSources.contains(source),
            let encodedId = eventId.addingPercentEncoding(
                withAllowedCharacters: .urlPathAllowed),
            let url = URL(
                string: "\(config.ingestUrl)/v1/events/\(encodedId)/attachments/\(kind)")
        else {
            completion?(false)
            return
        }

        let boundary = "----sentori-\(eventId)"
        if let record = recorderForTests {
            record(
                eventId, kind,
                multipartBody(
                    boundary: boundary, kind: kind, mediaType: mediaType, base64: base64,
                    source: source))
            completion?(true)
            return
        }
        var body = ""
        body += "--\(boundary)\r\n"
        body += "Content-Disposition: form-data; name=\"file\"; filename=\"\(kind).bin\"\r\n"
        body += "Content-Type: \(mediaType)\r\n"
        // The server decodes when told to. Without this header it
        // stores the base64 text as the image, which renders as
        // nothing and looks like a capture problem.
        body += "Content-Transfer-Encoding: base64\r\n"
        body += "\r\n\(base64)\r\n"
        body += "--\(boundary)\r\n"
        body += "Content-Disposition: form-data; name=\"source\"\r\n"
        body += "\r\n\(source)\r\n"
        body += "--\(boundary)--\r\n"

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue(
            "multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(config.token)", forHTTPHeaderField: "Authorization")
        request.setValue("swift/\(SentoriVersion.current)", forHTTPHeaderField: "Sentori-Sdk")
        request.httpBody = body.data(using: .utf8)
        request.timeoutInterval = 30

        URLSession.shared.dataTask(with: request) { _, response, _ in
            let code = (response as? HTTPURLResponse)?.statusCode ?? 0
            completion?((200..<300).contains(code))
        }.resume()
    }

    /// The multipart body, exposed so a test can read what would go
    /// on the wire. Building it correctly is the whole job here, and
    /// the failure mode is a 2xx that stored the wrong bytes.
    static func multipartBody(
        boundary: String, kind: String, mediaType: String, base64: String, source: String
    ) -> String {
        var body = ""
        body += "--\(boundary)\r\n"
        body += "Content-Disposition: form-data; name=\"file\"; filename=\"\(kind).bin\"\r\n"
        body += "Content-Type: \(mediaType)\r\n"
        body += "Content-Transfer-Encoding: base64\r\n"
        body += "\r\n\(base64)\r\n"
        body += "--\(boundary)\r\n"
        body += "Content-Disposition: form-data; name=\"source\"\r\n"
        body += "\r\n\(source)\r\n"
        body += "--\(boundary)--\r\n"
        return body
    }
}
