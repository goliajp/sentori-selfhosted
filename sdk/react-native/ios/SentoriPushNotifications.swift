// v2.9 — iOS push notification bridge.
//
// Owns:
//   * UNUserNotificationCenter delegate (for foreground / tap callbacks)
//   * AppDelegate method swizzle (for the APNs token registration)
//   * In-memory buffers (token, notifications, taps) drained by JS
//
// Design notes:
//   * The JS side polls `pushDrainState()` rather than receiving
//     RCTEventEmitter events. Matches the existing crash handler /
//     pending event pattern in this SDK — no new bridge primitive
//     introduced.
//   * Buffers are bounded (32-slot FIFO). A wedged JS-side drain
//     loop will not OOM the native side.
//   * Method swizzle is installed once on first register call.
//     Idempotent — re-running is a no-op. Hosts that prefer not to
//     swizzle set `Sentori.disableAppDelegateSwizzle = YES` in
//     Info.plist; the SDK then expects the host to manually call
//     `SentoriPushNotifications.shared.handleRegisteredToken(_)`
//     from `application:didRegisterForRemoteNotificationsWithDeviceToken:`.

import Foundation
import UIKit
import UserNotifications
import ObjectiveC.runtime

@objc public class SentoriPushNotifications: NSObject, UNUserNotificationCenterDelegate {
    @objc public static let shared = SentoriPushNotifications()

    // MARK: Buffers (capped FIFO, 32 slots each)
    private let bufferLock = NSLock()
    private var tokenHex: String?
    private var registrationError: String?
    private var notifications: [[String: Any]] = []
    private var taps: [[String: Any]] = []
    private static let bufferCap = 32

    // MARK: Swizzle
    private var swizzleInstalled = false

    // MARK: Permission

    /// Returns the current permission status as a JS-friendly string.
    /// Does NOT prompt.
    @objc public func currentPermission(completion: @escaping (String) -> Void) {
        UNUserNotificationCenter.current().getNotificationSettings { settings in
            completion(authString(settings.authorizationStatus))
        }
    }

    /// Requests authorization. Triggers the OS prompt the first time;
    /// subsequent calls return the cached decision.
    @objc public func requestPermission(completion: @escaping (String) -> Void) {
        let center = UNUserNotificationCenter.current()
        center.requestAuthorization(options: [.alert, .badge, .sound]) { granted, error in
            if let error = error {
                completion("error:\(error.localizedDescription)")
                return
            }
            center.getNotificationSettings { settings in
                completion(authString(settings.authorizationStatus))
            }
        }
    }

    // MARK: Register / Unregister

    /// Installs the SDK delegate + AppDelegate swizzle, then asks
    /// UIApplication to register for remote notifications. The OS
    /// will eventually call back into the swizzled AppDelegate
    /// method, which routes the token into our buffer.
    @objc public func registerForRemoteNotifications() {
        ensureSwizzleInstalled()
        UNUserNotificationCenter.current().delegate = self
        DispatchQueue.main.async {
            UIApplication.shared.registerForRemoteNotifications()
        }
    }

    /// Tells UIApplication to drop the device token + clears our
    /// cached token. Does not call the server — JS owns that side.
    @objc public func unregisterForRemoteNotifications() {
        DispatchQueue.main.async {
            UIApplication.shared.unregisterForRemoteNotifications()
        }
        bufferLock.lock()
        tokenHex = nil
        bufferLock.unlock()
    }

    // MARK: AppDelegate token handler (called from swizzle OR host)

    /// Host-callable when swizzle is disabled. The host's
    /// `application:didRegisterForRemoteNotificationsWithDeviceToken:`
    /// implementation should forward to this method.
    @objc public func handleRegisteredToken(_ deviceToken: Data) {
        let hex = deviceToken.map { String(format: "%02x", $0) }.joined()
        bufferLock.lock()
        tokenHex = hex
        registrationError = nil
        bufferLock.unlock()
    }

    /// Host-callable counterpart for the failure path.
    @objc public func handleRegistrationFailure(_ error: Error) {
        bufferLock.lock()
        registrationError = error.localizedDescription
        bufferLock.unlock()
    }

    // MARK: UNUserNotificationCenterDelegate

    public func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        let payload = payloadFor(notification: notification)
        appendNotification(payload)
        completionHandler([.banner, .list, .sound, .badge])
    }

    public func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        let payload = payloadFor(notification: response.notification)
        appendTap(payload)
        completionHandler()
    }

    // MARK: Drain (called by Expo Function)

    /// Snapshot the current buffer contents + clear. Resets the
    /// buffers atomically under the lock so concurrent registers
    /// from native don't drop events.
    @objc public func drainState() -> [String: Any] {
        bufferLock.lock()
        let tok = tokenHex
        let err = registrationError
        let n = notifications
        let t = taps
        notifications.removeAll()
        taps.removeAll()
        bufferLock.unlock()
        var dict: [String: Any] = [
            "notifications": n,
            "taps": t,
        ]
        if let tok = tok { dict["token"] = tok }
        if let err = err { dict["error"] = err }
        return dict
    }

    // MARK: Internal helpers

    private func appendNotification(_ payload: [String: Any]) {
        bufferLock.lock()
        notifications.append(payload)
        if notifications.count > Self.bufferCap {
            notifications.removeFirst(notifications.count - Self.bufferCap)
        }
        bufferLock.unlock()
    }

    private func appendTap(_ payload: [String: Any]) {
        bufferLock.lock()
        taps.append(payload)
        if taps.count > Self.bufferCap {
            taps.removeFirst(taps.count - Self.bufferCap)
        }
        bufferLock.unlock()
    }

    private func payloadFor(notification: UNNotification) -> [String: Any] {
        let content = notification.request.content
        var out: [String: Any] = [
            "id": notification.request.identifier,
            "title": content.title,
            "body": content.body,
            "userInfo": content.userInfo,
            "receivedAt": notification.date.timeIntervalSince1970,
        ]
        if !content.subtitle.isEmpty { out["subtitle"] = content.subtitle }
        if let category = content.categoryIdentifier as String?, !category.isEmpty {
            out["category"] = category
        }
        return out
    }

    // MARK: Swizzle

    private func ensureSwizzleInstalled() {
        if swizzleInstalled { return }

        // Allow opt-out per Info.plist.
        if Bundle.main.object(forInfoDictionaryKey: "Sentori.disableAppDelegateSwizzle") as? Bool == true {
            swizzleInstalled = true
            return
        }

        guard let appDelegate = UIApplication.shared.delegate else {
            // Without a delegate there's nothing to swizzle; nothing
            // will ever route a token back here. We mark installed so
            // we don't retry on every register call.
            swizzleInstalled = true
            return
        }

        let cls: AnyClass = type(of: appDelegate)
        swizzleDidRegister(on: cls)
        swizzleDidFailToRegister(on: cls)
        swizzleInstalled = true
    }

    private func swizzleDidRegister(on cls: AnyClass) {
        let sel = #selector(UIApplicationDelegate.application(_:didRegisterForRemoteNotificationsWithDeviceToken:))
        let stubSel = #selector(SentoriPushNotifications._sentori_application(_:didRegisterForRemoteNotificationsWithDeviceToken:))
        installForwardingPair(on: cls, original: sel, replacement: stubSel)
    }

    private func swizzleDidFailToRegister(on cls: AnyClass) {
        let sel = #selector(UIApplicationDelegate.application(_:didFailToRegisterForRemoteNotificationsWithError:))
        let stubSel = #selector(SentoriPushNotifications._sentori_application(_:didFailToRegisterForRemoteNotificationsWithError:))
        installForwardingPair(on: cls, original: sel, replacement: stubSel)
    }

    /// Inject a method implementation that calls our buffer first,
    /// then invokes the host's original implementation (if any).
    private func installForwardingPair(on cls: AnyClass, original: Selector, replacement: Selector) {
        // Add the replacement method on the host class — borrowed from
        // our static implementations on `SentoriPushNotifications`.
        guard let stubMethod = class_getInstanceMethod(SentoriPushNotifications.self, replacement) else {
            return
        }
        let stubImpl = method_getImplementation(stubMethod)
        let stubType = method_getTypeEncoding(stubMethod)

        if let existing = class_getInstanceMethod(cls, original) {
            // Host already implements this method. Swap our stub in
            // place, store the original IMP under a side selector.
            let sideSel = sel_registerName("_sentori_original_\(original.description)")
            // Add the original IMP under sideSel so our stub can
            // call back to it.
            class_addMethod(
                cls,
                sideSel,
                method_getImplementation(existing),
                method_getTypeEncoding(existing)
            )
            // Replace the existing IMP with our stub.
            class_replaceMethod(cls, original, stubImpl, stubType)
        } else {
            // Host doesn't implement this method. Add ours.
            class_addMethod(cls, original, stubImpl, stubType)
        }
    }

    // MARK: Stub implementations (added to host AppDelegate via swizzle)

    @objc func _sentori_application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        SentoriPushNotifications.shared.handleRegisteredToken(deviceToken)
        // Forward to host's original if it had one.
        let sideSel = sel_registerName("_sentori_original_application:didRegisterForRemoteNotificationsWithDeviceToken:")
        if let cls = object_getClass(self),
           let original = class_getInstanceMethod(cls, sideSel) {
            typealias FnType = @convention(c) (AnyObject, Selector, UIApplication, Data) -> Void
            let fn = unsafeBitCast(method_getImplementation(original), to: FnType.self)
            fn(self, sideSel, application, deviceToken)
        }
    }

    @objc func _sentori_application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        SentoriPushNotifications.shared.handleRegistrationFailure(error)
        let sideSel = sel_registerName("_sentori_original_application:didFailToRegisterForRemoteNotificationsWithError:")
        if let cls = object_getClass(self),
           let original = class_getInstanceMethod(cls, sideSel) {
            typealias FnType = @convention(c) (AnyObject, Selector, UIApplication, Error) -> Void
            let fn = unsafeBitCast(method_getImplementation(original), to: FnType.self)
            fn(self, sideSel, application, error)
        }
    }
}

/// Map UNAuthorizationStatus to a JS-friendly string mirroring web
/// `Notification.permission` values.
private func authString(_ status: UNAuthorizationStatus) -> String {
    switch status {
    case .notDetermined: return "notDetermined"
    case .denied: return "denied"
    case .authorized: return "granted"
    case .provisional: return "provisional"
    case .ephemeral: return "ephemeral"
    @unknown default: return "unknown"
    }
}
