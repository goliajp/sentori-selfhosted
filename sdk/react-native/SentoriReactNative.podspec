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
  s.platforms      = { ios: '13.4', tvos: '13.4' }
  s.swift_version  = '5.4'
  s.source         = { git: '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  s.source_files = 'ios/**/*.{h,m,mm,swift,hpp,cpp}'
  # XCTest only links into test targets; including ios/Tests/** in the
  # main pod target makes app builds fail with `no such module 'XCTest'`.
  s.exclude_files = 'ios/Tests/**'
end
