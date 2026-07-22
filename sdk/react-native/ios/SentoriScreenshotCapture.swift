import Foundation
import UIKit

/// Phase 42 sub-E.01/02/06 — capture the current screen + view tree
/// at native crash time.
///
/// Lives separately from `SentoriCrashHandler` so it can also be
/// invoked imperatively from the JS bridge (`captureNativeScreenshot`)
/// when a non-fatal native error fires and we still want the
/// "Captured at error" gallery to fill in.
///
/// Output shape — matches `protocol.md` attachment schema:
///
///     {
///       "screenshot": { "base64": "...", "mediaType": "image/jpeg" },
///       "viewTree":   { "rootId": "n1", "nodes": { ... } }
///     }
///
/// The crash handler base64-encodes both blobs and stuffs them into
/// the event JSON under `_pendingAttachments` so the JS side can
/// upload them on next launch via the standard
/// `POST /v1/events/<id>/attachments/<kind>` path.
///
/// Why not WebP: iOS < 14 has no system WebP encoder. JPEG q=70
/// matches the JS-side decision (sub-D.03); the size budget is the
/// same 500 KB hard limit on the server.
///
/// Why not a 5s background cache (yet): the only iOS native crash
/// path we capture today is `NSSetUncaughtExceptionHandler`, which
/// fires before the app fully tears down and where UIKit is still
/// valid. Signal-based crashes (SIGSEGV / SIGABRT) would need the
/// cache approach because signal handlers can't touch UIKit safely —
/// the cache layer will land alongside any future signal-crash work.
@objc public final class SentoriScreenshotCapture: NSObject {

    /// 480 px on the long edge keeps a typical screenshot under 100 KB
    /// JPEG-encoded; well under the 500 KB attachment hard limit and
    /// big enough to read text on a phone-sized canvas.
    private static let maxLongEdgePx: CGFloat = 480
    private static let jpegQuality: CGFloat = 0.7
    /// Depth-limited tree walk: matches the JS / dashboard
    /// `viewTree` schema in sub-G and bounds payload size.
    private static let maxTreeDepth: Int = 10
    /// Hard cap on the number of nodes we serialize even within
    /// depth=10 — protects against unbounded recyclers / list views.
    private static let maxNodes: Int = 1500

    /// Capture screenshot + view tree of the key window. Bounces to
    /// the main thread synchronously if invoked from elsewhere
    /// (UIKit drawing is main-thread-only). Returns `nil` when
    /// there's no window available (backgrounded, before scene
    /// attached, etc.).
    @objc public static func captureKeyWindow() -> [String: Any]? {
        if Thread.isMainThread {
            return captureSync()
        }
        var result: [String: Any]?
        DispatchQueue.main.sync {
            result = captureSync()
        }
        return result
    }

    /// v0.7.3 — JS-triggered screenshot path. Returns just
    /// `{ base64, mediaType }`; view tree not needed (errors that
    /// reach here are non-fatal and surface the tree via the
    /// breadcrumb / stack pipeline already). When `maskedIds` is
    /// non-empty we walk the view hierarchy by
    /// `accessibilityIdentifier` (RN bridges `nativeID` to this on
    /// iOS) and paint a black rectangle over each subview's frame in
    /// the captured bitmap. Called from `Sentori.captureScreenshotWithMask`
    /// in the Expo Module bridge.
    @objc public static func captureScreenshotWithMask(maskedIds: [String]) -> [String: String]? {
        if Thread.isMainThread {
            return captureWithMaskSync(maskedIds: maskedIds)
        }
        var result: [String: String]?
        DispatchQueue.main.sync {
            result = captureWithMaskSync(maskedIds: maskedIds)
        }
        return result
    }

    private static func captureWithMaskSync(maskedIds: [String]) -> [String: String]? {
        guard let window = keyWindowDiag().window else {
            lastDiagPath = "window.null"
            return nil
        }
        guard let jpeg = renderJpegBase64(window: window, maskedIds: Set(maskedIds)) else {
            lastDiagPath = "render.failed"
            return nil
        }
        lastDiagPath = "ok"
        return [
            "base64": jpeg,
            "mediaType": "image/jpeg",
        ]
    }

    // MARK: - Internals

    // v1.0.0-rc.2 — diagnostic readout mirror of the replay-capture
    // probe. The JS side calls `probeScreenshot()` to ship raw state
    // back when screenshot returns null.
    private static var lastDiagPath: String = "none(not-yet-called)"

    @objc public static func probe() -> [String: Any] {
        let (win, path) = keyWindowDiag()
        return [
            "lastPath": lastDiagPath,
            "resolvedPath": path,
            "windowFound": win != nil,
            "rootViewControllerFound": win?.rootViewController != nil,
            "boundsW": win.map { Double($0.bounds.width) } ?? 0.0,
            "boundsH": win.map { Double($0.bounds.height) } ?? 0.0,
        ]
    }

    private static func captureSync() -> [String: Any]? {
        guard let window = keyWindowDiag().window else {
            lastDiagPath = "window.null"
            return nil
        }
        var out: [String: Any] = [:]
        if let jpeg = renderJpegBase64(window: window) {
            out["screenshot"] = [
                "base64": jpeg,
                "mediaType": "image/jpeg",
            ]
        }
        out["viewTree"] = walkTree(root: window)
        if out.isEmpty {
            lastDiagPath = "empty"
            return nil
        }
        lastDiagPath = "ok"
        return out
    }

    /// keyWindow with the same 4-tier resolution as the replay capture,
    /// plus the diagnostic path tag for the probe. The original
    /// single-pass `keyWindow()` is kept for source-compat callers but
    /// new paths funnel through this so screenshot + replay agree on
    /// which window they are pointing at.
    private static func keyWindowDiag() -> (window: UIWindow?, path: String) {
        if #available(iOS 13.0, *) {
            let scenes = Array(UIApplication.shared.connectedScenes)
            for scene in scenes where scene.activationState == .foregroundActive {
                if let ws = scene as? UIWindowScene,
                   let key = ws.windows.first(where: { $0.isKeyWindow }) {
                    return (key, "scene.fg.key")
                }
            }
            for scene in scenes where scene.activationState == .foregroundActive {
                if let ws = scene as? UIWindowScene, let win = ws.windows.first {
                    return (win, "scene.fg.first")
                }
            }
            for scene in scenes where scene.activationState == .foregroundInactive {
                if let ws = scene as? UIWindowScene, let win = ws.windows.first {
                    return (win, "scene.fgi.first")
                }
            }
            for scene in scenes {
                if let ws = scene as? UIWindowScene, let win = ws.windows.first {
                    return (win, "scene.any.first")
                }
            }
        }
        if let leg = UIApplication.shared.windows.first {
            return (leg, "legacy.first")
        }
        return (nil, "none")
    }

    private static func keyWindow() -> UIWindow? {
        if #available(iOS 13.0, *) {
            for scene in UIApplication.shared.connectedScenes {
                guard let ws = scene as? UIWindowScene else { continue }
                if let key = ws.windows.first(where: { $0.isKeyWindow }) {
                    return key
                }
                if let first = ws.windows.first {
                    return first
                }
            }
        }
        // Fallback (pre-iOS 13 multi-scene shape)
        return UIApplication.shared.windows.first
    }

    private static func renderJpegBase64(
        window: UIWindow,
        maskedIds: Set<String> = []
    ) -> String? {
        let bounds = window.bounds
        let longEdge = max(bounds.width, bounds.height)
        let scale: CGFloat = longEdge > maxLongEdgePx ? maxLongEdgePx / longEdge : 1.0
        let outSize = CGSize(width: bounds.width * scale, height: bounds.height * scale)
        guard outSize.width > 1, outSize.height > 1 else { return nil }

        let format = UIGraphicsImageRendererFormat()
        format.scale = 1.0
        format.opaque = true
        let renderer = UIGraphicsImageRenderer(size: outSize, format: format)
        let image = renderer.image { ctx in
            window.drawHierarchy(
                in: CGRect(origin: .zero, size: outSize),
                afterScreenUpdates: false
            )
            // v0.7.3 — paint a black rectangle over every masked
            // subview's frame, in the same render pass so we don't
            // pay for a second bitmap allocation. `convert(_,to:)`
            // handles transforms and nested coordinate spaces; the
            // scale factor maps window-points to output-pixels.
            if !maskedIds.isEmpty {
                let regions = findMaskedSubviews(rootView: window, ids: maskedIds)
                if !regions.isEmpty {
                    UIColor.black.setFill()
                    for v in regions {
                        let rect = v.convert(v.bounds, to: window)
                        let scaled = CGRect(
                            x: rect.origin.x * scale,
                            y: rect.origin.y * scale,
                            width: rect.size.width * scale,
                            height: rect.size.height * scale
                        )
                        ctx.fill(scaled)
                    }
                }
            }
        }
        guard let data = image.jpegData(compressionQuality: jpegQuality) else {
            return nil
        }
        return data.base64EncodedString()
    }

    /// Depth-first walk that stops descending once a masked subtree
    /// is hit — the entire region is being blacked out, no need to
    /// look at children for a second match.
    private static func findMaskedSubviews(
        rootView: UIView,
        ids: Set<String>
    ) -> [UIView] {
        var found: [UIView] = []
        func walk(_ v: UIView) {
            if let id = v.accessibilityIdentifier, ids.contains(id) {
                found.append(v)
                return
            }
            for sub in v.subviews { walk(sub) }
        }
        walk(rootView)
        return found
    }

    private static func walkTree(root: UIView) -> [String: Any] {
        var nodes: [String: Any] = [:]
        var counter = 0
        var nodeCount = 0

        func nextId() -> String {
            counter += 1
            return "n\(counter)"
        }

        func walk(view: UIView, depth: Int) -> String {
            let id = nextId()
            nodeCount += 1
            var childIds: [String] = []
            if depth < maxTreeDepth && nodeCount < maxNodes {
                for sv in view.subviews {
                    if nodeCount >= maxNodes { break }
                    childIds.append(walk(view: sv, depth: depth + 1))
                }
            }
            let className = String(describing: type(of: view))
            let frame = view.frame
            var propsSummary: [String: String] = [
                "frame": String(
                    format: "%.0f,%.0f,%.0f,%.0f",
                    frame.origin.x, frame.origin.y,
                    frame.size.width, frame.size.height
                ),
                "alpha": String(format: "%.2f", view.alpha),
                "hidden": view.isHidden ? "true" : "false",
            ]
            if let label = view.accessibilityLabel, !label.isEmpty {
                // 200-byte cap matches sub-G dashboard / protocol budget.
                propsSummary["accessibilityLabel"] =
                    String(label.prefix(200))
            }
            nodes[id] = [
                "type": "UIView",
                "name": className,
                "props_summary": propsSummary,
                "children": childIds,
            ]
            return id
        }

        let rootId = walk(view: root, depth: 0)
        return [
            "rootId": rootId,
            "nodes": nodes,
        ]
    }
}
