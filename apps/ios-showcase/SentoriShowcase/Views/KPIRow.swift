import SwiftUI

/// Three-cell numeric strip under the hero. Updates live.
struct KPIRow: View {
    @Environment(SentoriService.self) private var sentori

    var body: some View {
        HStack(spacing: 12) {
            KPICell(
                label: "events",
                value: "\(sentori.events.count)",
                hint: "captured this session",
                accent: false,
            )
            KPICell(
                label: "replay frames",
                value: "\(sentori.ringFrames)",
                hint: "ring fill",
                accent: sentori.ringFrames > 0,
            )
            KPICell(
                label: "replay bytes",
                value: formatBytes(sentori.ringBytes),
                hint: "uncompressed",
                accent: false,
            )
        }
    }
}

private struct KPICell: View {
    let label: String
    let value: String
    let hint: String
    let accent: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(label.uppercased())
                .font(SentoriType.mono(9, weight: .medium))
                .tracking(2)
                .foregroundStyle(SentoriPalette.inkMuted)
            Text(value)
                .font(.system(size: 32, weight: .semibold, design: .default))
                .foregroundStyle(accent ? SentoriPalette.accent : SentoriPalette.ink)
                .contentTransition(.numericText())
                .animation(.snappy, value: value)
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            Text(hint)
                .font(.system(size: 11, weight: .regular))
                .foregroundStyle(SentoriPalette.inkMuted)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background {
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .fill(SentoriPalette.paperLift.opacity(0.5))
                .overlay(
                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                        .stroke(SentoriPalette.paperEdge, lineWidth: 0.5),
                )
        }
    }
}
