import Foundation

/// The privacy half of visual replay.
///
/// A host registers a closure returning the identifiers of views that
/// must never appear in a captured frame — a camera preview, a card
/// number, anything identifying. The capture paints over those
/// subtrees in the same render pass, so the pixels never leave the
/// device.
///
/// There was no way for a native host to say any of this. The capture
/// has taken a list of identifiers all along; only React Native had
/// somewhere to put one, so an app using the Swift SDK directly could
/// enable replay and had no means of excluding anything from it.
///
/// Identifiers are matched against a view's `accessibilityIdentifier`,
/// which is what React Native's `nativeID` and `testID` both become —
/// so a mixed app registering one string masks the same view from
/// either side.
public enum SentoriMask {
    /// Returns the identifiers to mask. Called once per captured
    /// frame, so keep it cheap: return a cached array rather than
    /// walking a view tree.
    public typealias Query = () -> [String]

    private static let lock = NSLock()
    private static var query: Query?

    /// Register, or with `nil`, clear.
    public static func register(_ query: Query?) {
        lock.lock()
        defer { lock.unlock() }
        self.query = query
    }

    /// The identifiers to mask right now.
    ///
    /// A query that throws or is absent returns an empty list, which
    /// means this frame masks nothing — matching what the TypeScript
    /// registry does, because a rule that differs by platform is a
    /// privacy rule nobody can state. Swift closures cannot throw
    /// through this signature, so the failure here is a query that
    /// returns rubbish, and non-strings cannot exist in `[String]`.
    public static func ids() -> [String] {
        lock.lock()
        let current = query
        lock.unlock()
        guard let current else { return [] }
        return current()
    }

    public static func __resetForTests() {
        register(nil)
    }
}
