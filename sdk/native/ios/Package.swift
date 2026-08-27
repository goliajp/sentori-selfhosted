// swift-tools-version: 5.9
//
// Sentori for iOS — the native core.
//
// These sources lived under `sdk/react-native/ios` and were compiled
// only as part of an Expo module, which meant two things: apps without
// React Native could not use any of it, and the 214 lines of XCTest
// beside it never ran. There was no module to import — `@testable
// import SentoriCrashHandler` named something that did not exist, and
// the `-scheme SentoriTests` in the test headers named a scheme no
// project defined.
//
// A package gives both a home: a module apps can link, and a test
// target `xcodebuild` can run.
//
// The React Native package now depends on this rather than carrying a
// copy. One implementation is the point — a copy would drift, and
// drift in push is invisible until someone is not notified.

import PackageDescription

let package = Package(
    name: "Sentori",
    // Matches the podspec's floor, so a CocoaPods host and a SwiftPM
    // host get the same support statement.
    platforms: [.iOS(.v14), .tvOS(.v14)],
    products: [
        .library(name: "Sentori", targets: ["Sentori"])
    ],
    targets: [
        .target(
            name: "Sentori",
            path: "Sources/Sentori"
        ),
        .testTarget(
            name: "SentoriTests",
            dependencies: ["Sentori"],
            path: "Tests/SentoriTests"
        ),
    ]
)
