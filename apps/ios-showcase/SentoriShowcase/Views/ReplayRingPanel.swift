import SwiftUI

/// Live read-out of the wireframe replay ring + the last probe.
struct ReplayRingPanel: View {
    @Environment(SentoriService.self) private var sentori

    private let ringCapacity = 60

    private var fillFraction: Double {
        min(1.0, Double(sentori.ringFrames) / Double(ringCapacity))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            SectionTitle(label: "Wireframe replay", hint: "60-slot ring · 2 Hz sampler")

            HStack(spacing: 18) {
                RingChart(fraction: fillFraction)
                    .frame(width: 110, height: 110)

                VStack(alignment: .leading, spacing: 8) {
                    DefRow(
                        label: "frames",
                        value: "\(sentori.ringFrames) / \(ringCapacity)",
                    )
                    DefRow(
                        label: "bytes",
                        value: formatBytes(sentori.ringBytes),
                    )
                    DefRow(
                        label: "last path",
                        value: sentori.lastProbe.lastPath,
                    )
                    DefRow(
                        label: "last nodes",
                        value: "\(sentori.lastProbe.lastNodes)",
                    )
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(16)
            .background {
                RoundedRectangle(cornerRadius: 22, style: .continuous)
                    .fill(.ultraThinMaterial.opacity(0.55))
                    .overlay(
                        RoundedRectangle(cornerRadius: 22, style: .continuous)
                            .stroke(SentoriPalette.paperEdge, lineWidth: 0.5),
                    )
            }
        }
    }
}

private struct RingChart: View {
    let fraction: Double

    var body: some View {
        ZStack {
            Circle()
                .stroke(SentoriPalette.inkDim, lineWidth: 6)
            Circle()
                .trim(from: 0, to: fraction)
                .stroke(
                    AngularGradient(
                        colors: [SentoriPalette.accentDeep, SentoriPalette.accent],
                        center: .center,
                    ),
                    style: StrokeStyle(lineWidth: 6, lineCap: .round),
                )
                .rotationEffect(.degrees(-90))
                .animation(.snappy, value: fraction)
            VStack(spacing: 2) {
                Text("\(Int(fraction * 100))%")
                    .font(.system(size: 22, weight: .semibold))
                    .foregroundStyle(SentoriPalette.ink)
                    .contentTransition(.numericText())
                Text("FILL")
                    .font(SentoriType.mono(9, weight: .medium))
                    .tracking(2)
                    .foregroundStyle(SentoriPalette.inkMuted)
            }
        }
    }
}

struct DefRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label.uppercased())
                .font(SentoriType.mono(9, weight: .medium))
                .tracking(1.8)
                .foregroundStyle(SentoriPalette.inkMuted)
                .frame(width: 72, alignment: .leading)
            Text(value)
                .font(SentoriType.mono(12))
                .foregroundStyle(SentoriPalette.ink)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
                .truncationMode(.middle)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

/// Compact byte size — keeps the value short under the narrow
/// rail without dropping precision at small sizes.
func formatBytes(_ bytes: Int) -> String {
    if bytes == 0 { return "—" }
    if bytes < 1024 { return "\(bytes) B" }
    let kb = Double(bytes) / 1024.0
    if kb < 100 { return String(format: "%.1f KB", kb) }
    if kb < 1024 { return "\(Int(kb)) KB" }
    return String(format: "%.1f MB", kb / 1024.0)
}
