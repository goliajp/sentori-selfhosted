require 'json'

package = JSON.parse(File.read(File.join(__dir__, 'package.json')))

Pod::Spec.new do |s|
  s.name           = 'SentoriReactNative'
  s.version        = package['version']
  s.summary        = package['description']
  s.description    = package['description']
  s.license        = 'TBD'
  s.author         = { 'Sentori' => 'support@sentori.golia.jp' }
  s.homepage       = 'https://sentori.golia.jp'
  # 13.4 was a claim nothing compiled. `SentoriPushNotifications`
  # uses `UNNotificationPresentationOptions.banner` and `.list`, both
  # iOS 14 — building the sources against the declared floor for the
  # first time (as a Swift package, 2026-08-11) failed on exactly
  # that. The floor now matches what the code needs.
  s.platforms      = { ios: '14.0', tvos: '14.0' }
  s.swift_version  = '5.4'
  s.source         = { git: '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  # `ios/SentoriModule.swift` is the bridge; `ios/core/` is a mirror of
  # `sdk/native/ios/Sources/Sentori`, written by
  # `scripts/sync-native-core.mjs` and gated byte-for-byte. Edit the
  # package, never the mirror — a pod cannot reach outside its own
  # directory, which is the only reason a copy exists at all.
  s.source_files = 'ios/**/*.{h,m,mm,swift,hpp,cpp}'
end
