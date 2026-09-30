import SwiftUI

/// Recent events stream — what the SDK saw, in order. Each row
/// pairs a SF Symbol (animated when added) with monospace metadata.
struct EventLog: View {
    @Environment(SentoriService.self) private var sentori

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            SectionTitle(label: "Recent events", hint: "newest first")

            VStack(spacing: 0) {
                if sentori.events.isEmpty {
                    HStack {
                        Spacer()
                        Text("waiting for an action…")
                            .font(.system(size: 13))
                            .foregroundStyle(SentoriPalette.inkMuted)
                            .padding(.vertical, 18)
                        Spacer()
                    }
                } else {
                    ForEach(Array(sentori.events.prefix(10).enumerated()), id: \.element.id) { idx, entry in
                        EventRow(entry: entry)
                            .transition(.asymmetric(
                                insertion: .move(edge: .top).combined(with: .opacity),
                                removal: .opacity,
                            ))
                        if idx < min(sentori.events.count, 10) - 1 {
                            Divider()
                                .background(SentoriPalette.paperEdge)
                        }
                    }
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background {
                RoundedRectangle(cornerRadius: 22, style: .continuous)
                    .fill(.ultraThinMaterial.opacity(0.55))
                    .overlay(
                        RoundedRectangle(cornerRadius: 22, style: .continuous)
                            .stroke(SentoriPalette.paperEdge, lineWidth: 0.5),
                    )
            }
            .animation(.snappy, value: sentori.events.count)
        }
    }
}

private struct EventRow: View {
    let entry: SentoriService.EventEntry

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            ZStack {
                Circle()
                    .fill(kindColor.opacity(0.14))
                    .frame(width: 28, height: 28)
                Image(systemName: kindSymbol)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(kindColor)
            }
            VStack(alignment: .leading, spacing: 3) {
                Text(entry.label)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(SentoriPalette.ink)
                if let detail = entry.detail {
                    Text(detail)
                        .font(SentoriType.mono(11))
                        .foregroundStyle(SentoriPalette.inkMuted)
                        .lineLimit(2)
                }
            }
            Spacer(minLength: 8)
            Text(timeFormatter.string(from: entry.timestamp))
                .font(SentoriType.mono(11))
                .foregroundStyle(SentoriPalette.inkMuted)
        }
        .padding(.vertical, 10)
    }

    private var kindSymbol: String {
        switch entry.kind {
        case .errorThrown: "exclamationmark.triangle.fill"
        case .errorCaptured: "checkmark.shield.fill"
        case .nativeCrash: "bolt.trianglebadge.exclamationmark.fill"
        case .mainHang: "stopwatch"
        case .probe: "square.grid.3x3.fold"
        case .drain: "square.stack.3d.up.fill"
        case .other: "circle.dotted"
        }
    }

    private var kindColor: Color {
        switch entry.kind {
        case .errorThrown, .nativeCrash: SentoriPalette.danger
        case .mainHang: SentoriPalette.warning
        case .errorCaptured, .drain: SentoriPalette.success
        case .probe: SentoriPalette.info
        case .other: SentoriPalette.inkMuted
        }
    }
}

private let timeFormatter: DateFormatter = {
    let f = DateFormatter()
    f.dateFormat = "HH:mm:ss"
    return f
}()
