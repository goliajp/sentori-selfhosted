# The native iOS SDK as a pod, for an app that is not a React Native
# app.
#
# `sdk/react-native/SentoriReactNative.podspec` is the other one, and
# it depends on ExpoModulesCore — reasonably, since it is the bridge.
# The consequence was that CocoaPods, which is still how a great many
# iOS apps take a dependency, could reach Sentori only by dragging in
# Expo. Swift Package Manager had no such problem, which is why this
# went unnoticed: the package everyone here uses was fine.
#
# The sources are the package's own, referenced in place. There is no
# mirror to drift, because a pod may read files under its own
# directory and `Sources/Sentori` is under this one.

require 'json'

Pod::Spec.new do |s|
  s.name           = 'Sentori'
  s.version        = File.read(File.join(__dir__, '..', 'VERSION')).strip
  s.summary        = 'Crash and error reporting for iOS'
  s.description    = 'The native iOS SDK: five verbs, crash capture, push, and session replay.'
  s.license        = { type: 'Apache-2.0 OR MIT' }
  s.author         = { 'GOLIA K.K.' => 'takagi@golia.jp' }
  s.homepage       = 'https://sentori.golia.jp'
  s.source         = { git: 'https://github.com/goliajp/sentori-swift.git', tag: "v#{s.version}" }

  # Matches Package.swift. `SentoriPushNotifications` uses
  # `UNNotificationPresentationOptions.banner`, which is iOS 14.
  s.platforms      = { ios: '14.0', tvos: '14.0' }
  s.swift_version  = '5.9'

  s.source_files   = 'Sources/Sentori/**/*.swift'

  # App Review rejects a required-reason API with no declared reason,
  # and the rejection lands on the host app's submission.
  s.resource_bundles = { 'Sentori' => ['Sources/Sentori/PrivacyInfo.xcprivacy'] }
end
