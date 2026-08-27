import Foundation

/// Ambient scope: the current user and the context patch that ride
/// every outgoing event. Two verbs own this state; everything else
/// reads it.
@objc(SentoriScope)
public final class SentoriScope: NSObject {

    private static let lock = NSLock()
    private static var _userKey: String?
    private static var _traits: [String: Any]?
    private static var _context: [String: Any] = [:]

    /// What to tell when the person changes.
    ///
    /// The push device row carries the identity, and nothing updated
    /// it after registration: an app that registers at launch and
    /// signs in ten seconds later — which is every app with a login
    /// screen — held a row with no user on it for the life of the
    /// install. A send aimed at that person reached nobody and said it
    /// had worked.
    ///
    /// A callback rather than a call into `SentoriPush`, so this file
    /// keeps knowing nothing about push.
    private static var _onIdentityChange: (() -> Void)?

    /// Register interest in identity changes. Only push does.
    public static func setIdentityListener(_ listener: (() -> Void)?) {
        lock.lock()
        _onIdentityChange = listener
        lock.unlock()
    }

    /// Identify the person using the app. Only the hash goes on the
    /// wire; the id and email stay on the device.
    ///
    /// Unlike the JavaScript version this is genuinely synchronous —
    /// WebCrypto's digest is a promise, so `scope.ts` sets the key a
    /// tick later and events sent in that gap carry none. CryptoKit
    /// has no such gap, so the first event after this call is already
    /// addressable.
    ///
    /// Pass `nil` for both to forget the user on sign-out.
    @objc public static func setUser(id: String?, email: String?) {
        setUser(id: id, email: email, traits: nil)
    }

    /// The same, plus attributes a push campaign can select on: plan,
    /// cohort, org.
    ///
    /// Traits travel raw, unlike `id` and `email`, so put nothing here
    /// that identifies the person. A call describes the person
    /// completely, so one made without traits means they have none
    /// rather than "leave the last ones" — a signed-out device that
    /// kept them would still be selectable as whoever just left.
    @objc public static func setUser(id: String?, email: String?, traits: [String: Any]?) {
        let key = SentoriIdentity.userKey(id: id, email: email)
        lock.lock()
        _userKey = key
        _traits = traits ?? [:]
        let listener = _onIdentityChange
        lock.unlock()
        // Outside the lock: a listener that registers a device must
        // not be holding this while it makes a request.
        listener?()
    }

    /// Merge keys into the ambient context. Later calls win per key;
    /// nothing is ever removed except by `clear`.
    @objc public static func patchContext(_ patch: [String: Any]) {
        lock.lock()
        for (k, v) in patch { _context[k] = v }
        lock.unlock()
    }

    @objc public static var userKey: String? {
        lock.lock()
        defer { lock.unlock() }
        return _userKey
    }

    /// The person's attributes, for the push device row.
    ///
    /// `nil` until the host has called `setUser` at all, which is
    /// different from an empty dictionary: `nil` leaves the row's
    /// traits alone, empty clears them.
    @objc public static var traits: [String: Any]? {
        lock.lock()
        defer { lock.unlock() }
        return _traits
    }

    /// `nil` rather than an empty dictionary, so an event with no
    /// context omits the field instead of carrying `{}`.
    @objc public static var context: [String: Any]? {
        lock.lock()
        defer { lock.unlock() }
        return _context.isEmpty ? nil : _context
    }

    @objc public static func clear() {
        lock.lock()
        _userKey = nil
        _traits = nil
        _context = [:]
        _onIdentityChange = nil
        lock.unlock()
    }
}
