import SwiftUI

/// Grid of demo actions. Each card triggers an SDK capability and
/// appends to the event log. iOS 26 symbols animate on tap.
struct ActionGrid: View {
    @Environment(SentoriService.self) private var sentori

    private let columns: [GridItem] = [
        GridItem(.flexible(), spacing: 12),
        GridItem(.flexible(), spacing: 12),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            SectionTitle(label: "Demo actions", hint: "tap a card to trigger the SDK")

            LazyVGrid(columns: columns, spacing: 12) {
                ActionCard(
                    symbol: "exclamationmark.triangle.fill",
                    title: "Throw error",
                    sub: "caught by global handler",
                    tint: SentoriPalette.danger,
                ) { sentori.throwTypeError() }

                ActionCard(
                    symbol: "pencil.and.scribble",
                    title: "Manual capture",
                    sub: "captureError + tags",
                    tint: SentoriPalette.accent,
                ) { sentori.captureManual() }

                ActionCard(
                    symbol: "wifi.exclamationmark",
                    title: "Failed fetch",
                    sub: "breadcrumb → capture",
                    tint: SentoriPalette.info,
                ) { sentori.failedFetch() }

                ActionCard(
                    symbol: "stopwatch",
                    title: "Hang main 3s",
                    sub: "ANR watchdog fires",
                    tint: SentoriPalette.warning,
                ) { sentori.hangMainThread() }

                ActionCard(
                    symbol: "camera.fill",
                    title: "Screenshot",
                    sub: "key-window capture",
                    tint: SentoriPalette.ink,
                ) { sentori.captureScreenshot() }

                ActionCard(
                    symbol: "square.grid.3x3.fold",
                    title: "Wireframe probe",
                    sub: "scene + window state",
                    tint: SentoriPalette.accent,
                ) { sentori.probeWireframe() }

                ActionCard(
                    symbol: "square.stack.3d.up.fill",
                    title: "Drain ring",
                    sub: "flush replay frames",
                    tint: SentoriPalette.success,
                ) { sentori.drainRing() }

                ActionCard(
                    symbol: "bolt.trianglebadge.exclamationmark.fill",
                    title: "Native crash",
                    sub: "closes the app",
                    tint: SentoriPalette.danger,
                    destructive: true,
                ) { sentori.triggerNativeCrash() }
            }
        }
    }
}

/// Single demo action. Liquid-glass-styled card with a tinted symbol.
/// Tap animates the symbol once (bounce.up) and runs the action.
private struct ActionCard: View {
    let symbol: String
    let title: String
    let sub: String
    let tint: Color
    var destructive: Bool = false
    let action: () -> Void

    @State private var symbolTrigger = 0

    var body: some View {
        Button {
            symbolTrigger += 1
            action()
        } label: {
            VStack(alignment: .leading, spacing: 10) {
                ZStack {
                    Circle()
                        .fill(tint.opacity(0.12))
                        .frame(width: 38, height: 38)
                    Image(systemName: symbol)
                        .font(.system(size: 17, weight: .medium))
                        .foregroundStyle(tint)
                        .symbolEffect(.bounce.up, value: symbolTrigger)
                }
                Spacer(minLength: 6)
                Text(title)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(SentoriPalette.ink)
                    .lineLimit(1)
                Text(sub)
                    .font(.system(size: 12))
                    .foregroundStyle(SentoriPalette.inkMuted)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(14)
            .background {
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .fill(.ultraThinMaterial.opacity(0.6))
                    .overlay(
                        RoundedRectangle(cornerRadius: 18, style: .continuous)
                            .stroke(
                                destructive ? SentoriPalette.danger.opacity(0.45) : SentoriPalette.paperEdge,
                                lineWidth: destructive ? 1 : 0.5,
                            ),
                    )
            }
        }
        .buttonStyle(ScaleOnPress())
    }
}

struct SectionTitle: View {
    let label: String
    let hint: String?

    init(label: String, hint: String? = nil) {
        self.label = label
        self.hint = hint
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label)
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(SentoriPalette.ink)
            if let hint {
                Text(hint)
                    .font(SentoriType.mono(10, weight: .regular))
                    .tracking(1.8)
                    .foregroundStyle(SentoriPalette.inkMuted)
                    .textCase(.uppercase)
                Spacer()
            }
        }
        .padding(.top, 8)
    }
}

private struct ScaleOnPress: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.97 : 1.0)
            .animation(.spring(response: 0.22, dampingFraction: 0.7), value: configuration.isPressed)
    }
}
