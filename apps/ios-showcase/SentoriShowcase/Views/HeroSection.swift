import SwiftUI

/// Brand + tagline + live ingest pill. The opening frame.
struct HeroSection: View {
    @Environment(SentoriService.self) private var sentori

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text("SENTORI")
                    .font(.system(size: 13, weight: .semibold, design: .default))
                    .tracking(4)
                    .foregroundStyle(SentoriPalette.ink)
                Circle()
                    .fill(SentoriPalette.accent)
                    .frame(width: 7, height: 7)
                    .offset(y: -2)
            }
            .padding(.top, 4)

            Text("Errors, traces, and ")
                .foregroundStyle(SentoriPalette.ink)
                + Text("intent")
                .foregroundStyle(SentoriPalette.accent)
                + Text(" —\nat the speed of triage.")
                .foregroundStyle(SentoriPalette.ink)

            HStack(spacing: 10) {
                LiveStatusPill(status: sentori.status)
                if let cs = sentori.coldStartMs {
                    PillBadge(
                        symbol: "stopwatch",
                        label: "cold start",
                        value: "\(cs) ms",
                    )
                }
                Spacer()
            }
            .padding(.top, 6)
        }
        .font(.system(size: 36, weight: .semibold, design: .default))
        .lineSpacing(2)
        .padding(.top, 28)
        .padding(.bottom, 4)
    }
}

/// Pulses with the SDK status. Uses iOS 26's variableColor symbol
/// effect for the radiating dot.
private struct LiveStatusPill: View {
    let status: SentoriService.Status

    private var color: Color {
        switch status {
        case .ready: SentoriPalette.success
        case .booting: SentoriPalette.warning
        case .offline: SentoriPalette.danger
        }
    }

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "circle.fill")
                .font(.system(size: 7))
                .foregroundStyle(color)
                .symbolEffect(
                    .pulse.byLayer,
                    options: .repeating,
                )
            Text(status.rawValue.uppercased())
                .font(SentoriType.mono(10, weight: .medium))
                .tracking(2)
                .foregroundStyle(SentoriPalette.inkSoft)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background {
            Capsule(style: .continuous)
                .fill(.ultraThinMaterial)
                .overlay(
                    Capsule(style: .continuous)
                        .stroke(SentoriPalette.paperEdge, lineWidth: 0.5),
                )
        }
    }
}

struct PillBadge: View {
    let symbol: String
    let label: String
    let value: String

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: symbol)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(SentoriPalette.inkMuted)
            Text(label.uppercased())
                .font(SentoriType.mono(10, weight: .medium))
                .tracking(1.8)
                .foregroundStyle(SentoriPalette.inkMuted)
            Text(value)
                .font(SentoriType.mono(11, weight: .semibold))
                .foregroundStyle(SentoriPalette.ink)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background {
            Capsule(style: .continuous)
                .fill(.ultraThinMaterial)
                .overlay(
                    Capsule(style: .continuous)
                        .stroke(SentoriPalette.paperEdge, lineWidth: 0.5),
                )
        }
    }
}
