import SwiftUI

/// Top-level showcase surface.
///
/// One long scroll. Hero, KPI strip, demo grid, replay ring,
/// recent events. No tabs, no navigation rail — the demo is a
/// single editorial page that scrolls top-to-bottom, like a Linear
/// changelog or a Vercel landing.
struct ContentView: View {
    @Environment(SentoriService.self) private var sentori

    var body: some View {
        ScrollView {
            VStack(spacing: 32) {
                HeroSection()
                KPIRow()
                ActionGrid()
                ReplayRingPanel()
                EventLog()
                FooterCredits()
            }
            .padding(.horizontal, 20)
            .padding(.top, 12)
            .padding(.bottom, 48)
        }
        .scrollIndicators(.hidden)
        .background(
            ZStack {
                SentoriPalette.paper.ignoresSafeArea()
                BackdropAura()
                    .ignoresSafeArea()
                if #available(iOS 18.0, *) {
                    AuroraMesh()
                        .ignoresSafeArea()
                        .opacity(0.55)
                        .allowsHitTesting(false)
                }
            }
        )
        .preferredColorScheme(.dark)
    }
}

#Preview {
    ContentView()
        .environment(SentoriService())
}

/// A faint, off-centre radial of accent + cool indigo — gives the
/// dark backdrop depth without competing with content. Static; no
/// performance cost.
private struct BackdropAura: View {
    var body: some View {
        ZStack {
            RadialGradient(
                colors: [SentoriPalette.accent.opacity(0.18), .clear],
                center: .topTrailing,
                startRadius: 60,
                endRadius: 360,
            )
            RadialGradient(
                colors: [Color(red: 0.235, green: 0.184, blue: 0.412).opacity(0.4), .clear],
                center: UnitPoint(x: 0.1, y: 0.85),
                startRadius: 80,
                endRadius: 420,
            )
        }
        .blendMode(.plusLighter)
        .opacity(0.55)
    }
}

/// iOS 18+ mesh gradient — a low-amplitude, slowly drifting aurora
/// behind the hero. The motion is gentle enough to read as
/// atmospheric (not animated UI) but enough to make the page feel
/// alive on first scroll.
@available(iOS 18.0, *)
private struct AuroraMesh: View {
    @State private var t: Float = 0

    private var points: [SIMD2<Float>] {
        let drift = sin(t) * 0.06
        let drift2 = cos(t * 0.7) * 0.04
        return [
            SIMD2(0, 0), SIMD2(0.5, -0.02), SIMD2(1, 0),
            SIMD2(-0.02 + drift, 0.45), SIMD2(0.5 + drift2, 0.5),
            SIMD2(1.02 - drift, 0.55),
            SIMD2(0, 1), SIMD2(0.5, 1.02), SIMD2(1, 1),
        ]
    }

    var body: some View {
        MeshGradient(
            width: 3,
            height: 3,
            points: points,
            colors: [
                SentoriPalette.paper, Color(red: 0.18, green: 0.12, blue: 0.16), SentoriPalette.accent.opacity(0.25),
                Color(red: 0.10, green: 0.10, blue: 0.18), Color(red: 0.16, green: 0.14, blue: 0.22), Color(red: 0.20, green: 0.10, blue: 0.12),
                SentoriPalette.paper, Color(red: 0.10, green: 0.13, blue: 0.20), SentoriPalette.paper,
            ],
        )
        .onAppear {
            withAnimation(.linear(duration: 18).repeatForever(autoreverses: true)) {
                t = .pi
            }
        }
    }
}

/// Footer — keeps the page from feeling like it just ended.
private struct FooterCredits: View {
    var body: some View {
        VStack(spacing: 6) {
            Text("SENTORI · IOS SHOWCASE")
                .font(SentoriType.mono(10, weight: .medium))
                .tracking(2.2)
                .foregroundStyle(SentoriPalette.inkMuted)
            Text("Errors, traces, and intent — at the speed of triage.")
                .font(SentoriType.body(13))
                .foregroundStyle(SentoriPalette.inkSoft)
                .multilineTextAlignment(.center)
        }
        .padding(.top, 12)
    }
}
