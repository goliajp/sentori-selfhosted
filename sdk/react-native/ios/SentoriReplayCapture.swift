import Foundation
import UIKit

/// v0.9.6 #2 — wireframe session replay (iOS side).
///
/// Walks the UIView hierarchy at 1 Hz and serializes each visible
/// node as a compact JSON dict:
///   { kind, x, y, w, h, text?, color? }
///
/// Mask: nodes whose `accessibilityIdentifier` matches the JS-side
/// mask registry (passed in as `maskedIds`) have their text replaced
/// with "***" and the masked flag set so descendants render as
/// black-filled rects in the dashboard player.
///
/// Output: one JSON object per snapshot, returned as a string. The
/// JS side appends each snapshot to a 60-slot ring buffer; on
/// `captureException` the ring is uploaded as a `replay` attachment
/// (NDJSON: one snapshot per line).
@objc public final class SentoriReplayCapture: NSObject {

    @objc public static func captureWireframe(maskedIds: [String]) -> String? {
        if Thread.isMainThread {
            return captureSync(maskedIds: Set(maskedIds))
        }
        var result: String?
        DispatchQueue.main.sync {
            result = captureSync(maskedIds: Set(maskedIds))
        }
        return result
    }

    /// Diagnostic readouts exposed to JS via `probeWireframe()`.
    ///
    /// v0.9.12: lastPath / lastNodes / scene-window counts
    /// v1.0.0-rc.3: + lastDepthMax / lastSizeBytes / totalTicks /
    ///              totalEmptyResultTicks — answers "the ring isn't
    ///              empty but the dashboard renders nothing, which
    ///              layer dropped the data?" without a re-roll.
    @objc public static var lastDiagPath: String = "none(not-yet-called)"
    @objc public static var lastDiagNodes: Int = 0
    @objc public static var lastDiagSceneCount: Int = 0
    @objc public static var lastDiagWindowCount: Int = 0
    @objc public static var lastDiagDepthMax: Int = 0
    @objc public static var lastDiagSizeBytes: Int = 0
    @objc public static var totalTicks: Int = 0
    @objc public static var totalEmptyResultTicks: Int = 0
    private static var loggedFirstResult = false

    private static func captureSync(maskedIds: Set<String>) -> String? {
        totalTicks += 1
        let (winOpt, path) = resolveKeyWindow()
        lastDiagPath = path
        lastDiagSceneCount = currentSceneCount()
        lastDiagWindowCount = currentWindowCount()
        guard let window = winOpt else {
            totalEmptyResultTicks += 1
            if !loggedFirstResult {
                NSLog(
                    "[sentori] wireframe: returning nil — keyWindow path=%@ scenes=%d windows=%d",
                    path,
                    lastDiagSceneCount,
                    lastDiagWindowCount
                )
                loggedFirstResult = true
            }
            return nil
        }
        var nodes: [[String: Any]] = []
        var depthMax = 0
        walk(
            view: window,
            depth: 0,
            depthMax: &depthMax,
            parentMasked: false,
            maskedIds: maskedIds,
            window: window,
            nodes: &nodes
        )
        lastDiagNodes = nodes.count
        lastDiagDepthMax = depthMax
        if nodes.isEmpty {
            totalEmptyResultTicks += 1
        }
        if !loggedFirstResult {
            NSLog(
                "[sentori] wireframe: first capture ok — keyWindow path=%@ bounds=%.0fx%.0f nodes=%d depthMax=%d",
                path,
                window.bounds.width,
                window.bounds.height,
                nodes.count,
                depthMax
            )
            loggedFirstResult = true
        }
        let payload: [String: Any] = [
            "ts": Int(Date().timeIntervalSince1970 * 1000),
            "width": Double(window.bounds.width),
            "height": Double(window.bounds.height),
            "nodes": nodes,
        ]
        if let data = try? JSONSerialization.data(withJSONObject: payload, options: []) {
            let s = String(data: data, encoding: .utf8)
            lastDiagSizeBytes = s?.utf8.count ?? 0
            return s
        }
        return nil
    }

    /// Four-tier window resolution. The previous single-pass loop
    /// returned nil whenever the first connected scene was a
    /// `.background` or `.unattached` SwiftUI/preview scene that had
    /// no windows yet — common on iOS 26 cold-start where the JS
    /// thread spins up the replay tick before scene activation
    /// settles (the v0.9.6 default 1 Hz fires within ~200 ms).
    private static func resolveKeyWindow() -> (UIWindow?, String) {
        if #available(iOS 13.0, *) {
            let scenes = Array(UIApplication.shared.connectedScenes)
            // Pass 1: foregroundActive scene with a key window.
            for scene in scenes where scene.activationState == .foregroundActive {
                if let ws = scene as? UIWindowScene,
                   let key = ws.windows.first(where: { $0.isKeyWindow }) {
                    return (key, "scene.fg.key")
                }
            }
            // Pass 2: foregroundActive scene's first window (no key set yet).
            for scene in scenes where scene.activationState == .foregroundActive {
                if let ws = scene as? UIWindowScene, let win = ws.windows.first {
                    return (win, "scene.fg.first")
                }
            }
            // Pass 3: foregroundInactive (mid-transition) scene with any window.
            for scene in scenes where scene.activationState == .foregroundInactive {
                if let ws = scene as? UIWindowScene, let win = ws.windows.first {
                    return (win, "scene.fgi.first")
                }
            }
            // Pass 4: any scene at all with windows.
            for scene in scenes {
                if let ws = scene as? UIWindowScene, let win = ws.windows.first {
                    return (win, "scene.any.first")
                }
            }
            // Fallthrough → legacy windows list.
        }
        if let leg = UIApplication.shared.windows.first {
            return (leg, "legacy.first")
        }
        return (nil, "none")
    }

    private static func currentSceneCount() -> Int {
        if #available(iOS 13.0, *) {
            return UIApplication.shared.connectedScenes.count
        }
        return 0
    }

    private static func currentWindowCount() -> Int {
        if #available(iOS 13.0, *) {
            return UIApplication.shared.connectedScenes.reduce(0) { acc, scene in
                acc + ((scene as? UIWindowScene)?.windows.count ?? 0)
            }
        }
        return UIApplication.shared.windows.count
    }

    /// JS-side probe. Returns a dict the example/dashboard can render
    /// to ask "why is the ring empty?" without parsing Metro logs.
    /// `lastNodes == 0 && lastPath != "none"` means the window walk
    /// happened but the tree was empty (unusual — backgrounded?).
    /// `lastPath == "none(...)"` means no UIWindow was reachable at
    /// the moment of the last tick.
    @objc public static func probe() -> [String: Any] {
        return [
            "lastPath": lastDiagPath,
            "lastNodes": lastDiagNodes,
            "sceneCount": lastDiagSceneCount,
            "windowCount": lastDiagWindowCount,
            "lastDepthMax": lastDiagDepthMax,
            "lastSizeBytes": lastDiagSizeBytes,
            "totalTicks": totalTicks,
            "totalEmptyResultTicks": totalEmptyResultTicks,
        ]
    }

    /// Cap on nodes per snapshot — extremely deep / wide trees can
    /// have thousands of subviews (UICollectionView recyclers).
    private static let MAX_NODES = 800
    private static let MAX_DEPTH = 60

    private static func walk(
        view: UIView,
        depth: Int,
        depthMax: inout Int,
        parentMasked: Bool,
        maskedIds: Set<String>,
        window: UIWindow,
        nodes: inout [[String: Any]]
    ) {
        if nodes.count >= MAX_NODES { return }
        if depth >= MAX_DEPTH { return }
        if view.isHidden || view.alpha < 0.01 { return }

        if depth > depthMax { depthMax = depth }

        let isThisMasked = view.accessibilityIdentifier
            .map { maskedIds.contains($0) } ?? false
        let masked = parentMasked || isThisMasked

        let frame = view.convert(view.bounds, to: window)
        // Skip nodes outside the window bounds (off-screen recyclers).
        if !frame.intersects(window.bounds) {
            return
        }

        var node: [String: Any] = [
            "x": Double(frame.origin.x),
            "y": Double(frame.origin.y),
            "w": Double(frame.size.width),
            "h": Double(frame.size.height),
        ]

        if masked {
            node["kind"] = "mask"
        } else if let label = view as? UILabel, let text = label.text, !text.isEmpty {
            node["kind"] = "text"
            node["text"] = text.count > 200 ? String(text.prefix(200)) : text
            if let color = label.textColor.flatMap(colorToHex) {
                node["color"] = color
            }
        } else if let textView = view as? UITextView, let text = textView.text, !text.isEmpty {
            node["kind"] = "text"
            node["text"] = text.count > 200 ? String(text.prefix(200)) : text
        } else if view is UIImageView {
            node["kind"] = "image"
        } else if let bg = view.backgroundColor, let hex = colorToHex(bg), hex != "#00000000" {
            node["kind"] = "rect"
            node["color"] = hex
        }
        // else: invisible container — skip emitting but recurse.

        if node["kind"] != nil {
            nodes.append(node)
        }

        if !masked {
            // Don't expose internals of masked subtrees.
            for sub in view.subviews {
                walk(
                    view: sub,
                    depth: depth + 1,
                    depthMax: &depthMax,
                    parentMasked: masked,
                    maskedIds: maskedIds,
                    window: window,
                    nodes: &nodes
                )
            }
        }
    }

    private static func colorToHex(_ color: UIColor?) -> String? {
        guard let c = color else { return nil }
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        c.getRed(&r, green: &g, blue: &b, alpha: &a)
        let ri = max(0, min(255, Int(r * 255)))
        let gi = max(0, min(255, Int(g * 255)))
        let bi = max(0, min(255, Int(b * 255)))
        let ai = max(0, min(255, Int(a * 255)))
        return String(format: "#%02X%02X%02X%02X", ri, gi, bi, ai)
    }
}
