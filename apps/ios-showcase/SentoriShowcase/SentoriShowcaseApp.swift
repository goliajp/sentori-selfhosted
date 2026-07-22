import SwiftUI

/// Sentori — iOS 26 showcase app.
///
/// This is the public face of the project. Every detail matters: type,
/// motion, materials, copy. We use SwiftUI 6 + iOS 26 idioms (Liquid
/// Glass, animated symbols, the new symbol effect framework, fluid
/// transitions). The Sentori SDK's pure-Swift core (crash handler,
/// wireframe sampler, replay ring, vitals) is linked directly — no
/// React Native, no Expo, no JS bridge in this app.
@main
struct SentoriShowcaseApp: App {
    @State private var sentori = SentoriService()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(sentori)
                .preferredColorScheme(.dark)
                .task {
                    sentori.boot()
                }
        }
    }
}
