import SwiftUI

/// Editorial palette + type tokens.
///
/// Matches `web/src/index.css` so the iOS showcase reads as the same
/// product as the dashboard: warm off-paper backdrop, ink-on-paper
/// type scale, tora-orange as the single chromatic accent, semantic
/// colours reserved for status (danger/warning/success/info).
///
/// Dark mode is the default for the showcase (iOS lock screen vibe).
/// Light mode token paths are kept in source so the brand stays
/// consistent if a future variant flips schemes.
enum SentoriPalette {
    /// Tora-orange. The brand colour. Used sparingly.
    static let accent = Color(red: 1.0, green: 0.471, blue: 0.282) // #FF7848 (dark)
    static let accentSoft = Color(red: 1.0, green: 0.471, blue: 0.282).opacity(0.18)
    static let accentDeep = Color(red: 0.816, green: 0.286, blue: 0.125) // #D04920 (light)

    /// Ink hierarchy.
    static let ink = Color.white.opacity(0.92)
    static let inkSoft = Color.white.opacity(0.68)
    static let inkMuted = Color.white.opacity(0.42)
    static let inkDim = Color.white.opacity(0.18)

    /// Paper hierarchy.
    static let paper = Color(red: 0.043, green: 0.043, blue: 0.063) // near-black, slightly warm
    static let paperLift = Color(red: 0.078, green: 0.078, blue: 0.102)
    static let paperEdge = Color.white.opacity(0.06)

    /// Semantic.
    static let danger = Color(red: 1.0, green: 0.412, blue: 0.380)   // soft red
    static let success = Color(red: 0.443, green: 0.851, blue: 0.624) // mint
    static let warning = Color(red: 1.0, green: 0.733, blue: 0.337)  // amber
    static let info = Color(red: 0.392, green: 0.682, blue: 1.0)     // sky
}

/// Variable-font / display type. We use system rounded for warmth and
/// SF Mono for technical readouts (event ids, payload bytes).
enum SentoriType {
    static func display(_ size: CGFloat, weight: Font.Weight = .semibold) -> Font {
        .system(size: size, weight: weight, design: .default).width(.standard)
    }

    static func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced)
    }

    static func body(_ size: CGFloat = 15, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .default)
    }
}
