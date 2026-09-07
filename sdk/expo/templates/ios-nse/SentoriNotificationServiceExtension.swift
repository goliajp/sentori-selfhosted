// Sentori Notification Service Extension — v2.28.
//
// When an APNs payload arrives with `mutable-content: 1` AND a custom
// `sentori_attachment_url` field, this extension downloads the image
// and attaches it to the notification before iOS displays it.
//
// Lifecycle:
//   - serviceExtensionTimeWillExpire is called ~30 s after delivery.
//     We fall back to the unaltered notification at that point.
//   - All work runs off the main thread; the OS gives this extension
//     up to ~30 s with a hard CPU budget. The download is on a
//     background URLSession with 5 s timeout so a slow CDN does not
//     burn the budget.
//
// This file is written by `@goliapkg/sentori-expo` v2.28+'s
// `withSentoriNSE` config plugin into `ios/SentoriNSE/` on every
// `expo prebuild`. The one-time Xcode target wiring is documented in
// the recipe (auto-injection lands in v2.28.1).

import UserNotifications

final class SentoriNotificationServiceExtension: UNNotificationServiceExtension {
    private var contentHandler: ((UNNotificationContent) -> Void)?
    private var bestAttempt: UNMutableNotificationContent?

    override func didReceive(
        _ request: UNNotificationRequest,
        withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void
    ) {
        self.contentHandler = contentHandler
        self.bestAttempt = (request.content.mutableCopy() as? UNMutableNotificationContent)
        guard let bestAttempt = self.bestAttempt else {
            contentHandler(request.content)
            return
        }

        // Extract the Sentori-reserved attachment URL. v2.28 server
        // emits this as a top-level custom-data key when the send's
        // `richMedia.imageUrl` is set.
        guard
            let raw = bestAttempt.userInfo["sentori_attachment_url"] as? String,
            let url = URL(string: raw)
        else {
            contentHandler(bestAttempt)
            return
        }

        // Bounded-time download to a temp file. URLSession.shared
        // honours the request's timeout; we set it to 5 s so a stalled
        // CDN does not eat the ~30 s extension budget.
        var request = URLRequest(url: url)
        request.timeoutInterval = 5
        let task = URLSession.shared.downloadTask(with: request) { tempURL, _, _ in
            defer { contentHandler(bestAttempt) }
            guard let tempURL = tempURL else { return }
            // Move to a guessed-extension destination so iOS picks a
            // sensible content type.
            let ext = url.pathExtension.isEmpty ? "img" : url.pathExtension
            let dest = tempURL.deletingPathExtension().appendingPathExtension(ext)
            try? FileManager.default.moveItem(at: tempURL, to: dest)
            if let attachment = try? UNNotificationAttachment(
                identifier: "sentori-image",
                url: dest,
                options: nil
            ) {
                bestAttempt.attachments = [attachment]
            }
        }
        task.resume()
    }

    override func serviceExtensionTimeWillExpire() {
        if let contentHandler = contentHandler, let bestAttempt = bestAttempt {
            contentHandler(bestAttempt)
        }
    }
}
