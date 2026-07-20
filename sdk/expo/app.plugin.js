/**
 * Sentori Expo Config Plugin.
 *
 * `@goliapkg/sentori-react-native` already exposes
 * `expo-module.config.json` + iOS podspec + Android build.gradle, so
 * Expo Modules autolinking handles the native side without any
 * additional config-plugins work for error / span / replay capture.
 *
 * v2.11 — extends the plugin to also wire **push notifications** for
 * apps that opt in. When the host adds `@goliapkg/sentori-expo` to
 * its `app.json` plugins array, prebuild auto-injects:
 *
 *   iOS:
 *     - Info.plist: UIBackgroundModes ⊇ [remote-notification]
 *     - Entitlements: aps-environment = 'production' (Xcode flips to
 *       'development' for debug signing automatically)
 *
 *   Android:
 *     - AndroidManifest.xml: <uses-permission POST_NOTIFICATIONS>
 *     - Root build.gradle: classpath com.google.gms:google-services
 *     - App build.gradle: apply google-services + firebase-bom +
 *       firebase-messaging
 *     - Copies google-services.json from `props.googleServicesFile`
 *       (defaults to `./google-services.json` at the host root) to
 *       `android/app/google-services.json` on prebuild.
 *
 * Opt out per platform with `{ ios: false }` / `{ android: false }`.
 * Opt out entirely by not including the plugin in `app.json`.
 *
 * The plugin is intentionally CommonJS — Expo's plugin loader uses
 * `require()`.
 */
const fs = require('fs')
const path = require('path')
const {
  withInfoPlist,
  withEntitlementsPlist,
  withAndroidManifest,
  withProjectBuildGradle,
  withAppBuildGradle,
  withDangerousMod,
  withXcodeProject,
  AndroidConfig,
  withPlugins,
} = require('@expo/config-plugins')

// NSE target wiring constants. Path is relative to the .xcodeproj — the
// `xcode` package's addBuildPhase resolves filePaths against the project
// root, not the target subfolder, so the NSE/ prefix is required here
// even though the source actually lives at ios/SentoriNSE/<basename>.
const NSE_TARGET = 'SentoriNSE'
const NSE_SOURCE_REL = 'SentoriNSE/SentoriNotificationServiceExtension.swift'
const NSE_PLIST_REL = 'SentoriNSE-Info.plist'

const SENTORI_VERSION_KEY = 'SentoriSdkVersion'
const FIREBASE_BOM_VERSION = '33.5.1'
const GOOGLE_SERVICES_VERSION = '4.4.2'

// ── Existing marker (Sentori SDK version surface) ──────────────────

/**
 * @param {import('@expo/config-plugins').ExpoConfig} config
 * @param {{ sdkVersion?: string }} props
 */
const withSentoriVersion = (config, props = {}) => {
  return withInfoPlist(config, (cfg) => {
    cfg.modResults[SENTORI_VERSION_KEY] = props.sdkVersion || '0.1.0'
    return cfg
  })
}

// ── v2.11 iOS push ─────────────────────────────────────────────────

/**
 * @param {import('@expo/config-plugins').ExpoConfig} config
 */
const withSentoriPushIos = (config) => {
  config = withInfoPlist(config, (cfg) => {
    const modes = Array.isArray(cfg.modResults.UIBackgroundModes)
      ? cfg.modResults.UIBackgroundModes
      : []
    if (!modes.includes('remote-notification')) {
      modes.push('remote-notification')
    }
    cfg.modResults.UIBackgroundModes = modes
    return cfg
  })
  config = withEntitlementsPlist(config, (cfg) => {
    if (!cfg.modResults['aps-environment']) {
      // Xcode automatically swaps to 'development' when the build is
      // signed with a development provisioning profile, so this
      // default is correct for both flavors.
      cfg.modResults['aps-environment'] = 'production'
    }
    return cfg
  })
  return config
}

// ── v2.11 Android push ─────────────────────────────────────────────

/**
 * @param {import('@expo/config-plugins').ExpoConfig} config
 */
const withSentoriPushAndroidManifest = (config) => {
  return withAndroidManifest(config, (cfg) => {
    // addPermission expects the AndroidManifest object (cfg.modResults);
    // it dereferences `.manifest['uses-permission']` internally. Passing
    // `cfg.modResults.manifest` makes that read fail with `Cannot read
    // properties of undefined (reading 'uses-permission')` and crashes
    // `expo prebuild`.
    AndroidConfig.Permissions.addPermission(
      cfg.modResults,
      'android.permission.POST_NOTIFICATIONS'
    )
    return cfg
  })
}

/**
 * @param {import('@expo/config-plugins').ExpoConfig} config
 */
const withSentoriPushAndroidGradle = (config) => {
  // Root build.gradle: add google-services classpath.
  config = withProjectBuildGradle(config, (cfg) => {
    if (cfg.modResults.language === 'groovy') {
      const classpath = `classpath('com.google.gms:google-services:${GOOGLE_SERVICES_VERSION}')`
      if (!cfg.modResults.contents.includes('com.google.gms:google-services')) {
        cfg.modResults.contents = cfg.modResults.contents.replace(
          /(dependencies\s*\{)/,
          `$1\n        ${classpath}`
        )
      }
    }
    return cfg
  })
  // App build.gradle: apply plugin + firebase deps.
  config = withAppBuildGradle(config, (cfg) => {
    if (cfg.modResults.language !== 'groovy') return cfg
    let contents = cfg.modResults.contents
    if (!contents.includes('com.google.gms.google-services')) {
      contents += `\napply plugin: 'com.google.gms.google-services'\n`
    }
    if (!contents.includes('firebase-bom')) {
      contents = contents.replace(
        /(dependencies\s*\{)/,
        `$1\n    implementation platform('com.google.firebase:firebase-bom:${FIREBASE_BOM_VERSION}')\n    implementation 'com.google.firebase:firebase-messaging'`
      )
    }
    cfg.modResults.contents = contents
    return cfg
  })
  return config
}

/**
 * @param {import('@expo/config-plugins').ExpoConfig} config
 * @param {{ googleServicesFile?: string }} props
 */
const withSentoriGoogleServicesJson = (config, props = {}) => {
  return withDangerousMod(config, [
    'android',
    async (cfg) => {
      const srcRel = props.googleServicesFile || './google-services.json'
      const projectRoot = cfg.modRequest.projectRoot
      const src = path.isAbsolute(srcRel) ? srcRel : path.join(projectRoot, srcRel)
      if (!fs.existsSync(src)) {
        // Don't fail the build; warn so the operator notices.
        // eslint-disable-next-line no-console
        console.warn(
          `[sentori-expo] google-services.json not found at ${src}; skipping copy. Push will work once the file is added + prebuild re-runs.`
        )
        return cfg
      }
      const platformRoot = cfg.modRequest.platformProjectRoot
      const dest = path.join(platformRoot, 'app', 'google-services.json')
      fs.mkdirSync(path.dirname(dest), { recursive: true })
      fs.copyFileSync(src, dest)
      return cfg
    },
  ])
}

// ── v2.28 iOS Notification Service Extension ─────────────────────
//
// Rich-media notifications (images / future video) require an NSE
// target on the iOS app. The Sentori NSE template downloads the URL
// at `userInfo.sentori_attachment_url` and attaches it before iOS
// displays the notification. APNs server side sets this key when
// `richMedia.imageUrl` is on the send (v2.28+).
//
// As of 7.0.2 the wiring is fully automated across two plugins:
//
//   - withSentoriNSE         (this section) writes the template files
//                            into ios/SentoriNSE/ AND syncs the NSE
//                            Info.plist's CFBundleShortVersionString /
//                            CFBundleVersion to the host app's values.
//                            Apple's app-extension verifier rejects an
//                            .appex whose version keys do not match
//                            the parent app at signing time, so the
//                            sync is mandatory.
//
//   - withSentoriNSETarget   (further down) creates the actual Xcode
//                            target via withXcodeProject so signing +
//                            building succeed without any manual
//                            Xcode-UI step.
//
// Opt out with `{ ios: false }` (which drops the rest of the iOS push
// wiring too) or with `{ nse: false }` for just NSE (template + target).

/**
 * Rewrite the NSE Info.plist's marketing + build version strings to
 * track the host app. Apple rejects an .appex at signing time when its
 * CFBundleShortVersionString / CFBundleVersion don't match the parent
 * app — present in every Sentori install that enabled NSE before 7.0.2.
 *
 * Pure / exported for unit-test coverage.
 *
 * @param {string} contents — current NSE-Info.plist contents
 * @param {{ version: string, buildNumber: string }} ver
 * @returns {string}
 */
function syncNSEPlistVersion(contents, ver) {
  return contents
    .replace(
      /(<key>CFBundleShortVersionString<\/key>\s*<string>)[^<]*(<\/string>)/,
      `$1${ver.version}$2`
    )
    .replace(
      /(<key>CFBundleVersion<\/key>\s*<string>)[^<]*(<\/string>)/,
      `$1${ver.buildNumber}$2`
    )
}

/**
 * @param {import('@expo/config-plugins').ExpoConfig} config
 */
const withSentoriNSE = (config) => {
  return withDangerousMod(config, [
    'ios',
    async (cfg) => {
      const platformRoot = cfg.modRequest.platformProjectRoot
      const destDir = path.join(platformRoot, NSE_TARGET)
      const templateDir = path.join(__dirname, 'templates', 'ios-nse')
      const swiftSrc = path.join(templateDir, 'SentoriNotificationServiceExtension.swift')
      const plistSrc = path.join(templateDir, 'SentoriNSE-Info.plist')
      if (!fs.existsSync(swiftSrc) || !fs.existsSync(plistSrc)) {
        // eslint-disable-next-line no-console
        console.warn(
          '[sentori-expo] NSE templates missing; skipping. Reinstall the package to restore.'
        )
        return cfg
      }
      fs.mkdirSync(destDir, { recursive: true })
      fs.copyFileSync(
        swiftSrc,
        path.join(destDir, 'SentoriNotificationServiceExtension.swift')
      )
      const plistDest = path.join(destDir, NSE_PLIST_REL)
      fs.copyFileSync(plistSrc, plistDest)

      // Version-sync the freshly-copied plist in the same callback so
      // the read+write is atomic with the copy (no dependency on
      // Expo's cross-plugin dangerousMod ordering, which is LIFO and
      // therefore brittle to reason about). Expo defaults the host
      // app's CFBundleVersion to '1' when ios.buildNumber is unset.
      const version = cfg.version
      const buildNumber = cfg.ios?.buildNumber ?? '1'
      if (version) {
        const current = fs.readFileSync(plistDest, 'utf-8')
        const updated = syncNSEPlistVersion(current, { buildNumber, version })
        if (updated !== current) fs.writeFileSync(plistDest, updated)
      }
      return cfg
    },
  ])
}

/**
 * Pure pbxproj mutation. Adds the NSE target + build phases + build
 * settings, or returns false when the target already exists.
 *
 * Exported for unit-test coverage.
 *
 * @param {object} pbxproj — `xcode` package's Pbxproj instance
 *   (cfg.modResults inside withXcodeProject)
 * @param {{ mainBundleId: string, deploymentTarget: string, appleTeamId?: string }} opts
 * @returns {boolean} — true when the target was added, false when no-op
 */
function injectNSETarget(pbxproj, opts) {
  if (pbxproj.pbxTargetByName(NSE_TARGET)) return false

  const { appleTeamId, deploymentTarget, mainBundleId } = opts
  if (!mainBundleId) throw new Error('[sentori-expo] mainBundleId is required for NSE target')
  if (!deploymentTarget) {
    throw new Error('[sentori-expo] deploymentTarget is required for NSE target')
  }

  // addTarget creates the PBXNativeTarget + XCBuildConfigurationList with
  // INFOPLIST_FILE / PRODUCT_NAME / SKIP_INSTALL pre-set, and wires the
  // produced .appex into a "Copy Files" phase on the main app target —
  // that is Xcode's "Embed App Extensions" phase under a different name.
  const target = pbxproj.addTarget(
    NSE_TARGET,
    'app_extension',
    NSE_TARGET,
    `${mainBundleId}.${NSE_TARGET}`
  )

  pbxproj.addBuildPhase([NSE_SOURCE_REL], 'PBXSourcesBuildPhase', 'Sources', target.uuid)
  pbxproj.addBuildPhase([], 'PBXResourcesBuildPhase', 'Resources', target.uuid)
  pbxproj.addBuildPhase([], 'PBXFrameworksBuildPhase', 'Frameworks', target.uuid)

  // xcode 3.0.x stores native target names quoted (`'"SentoriNSE"'`), so
  // `pbxTargetByName('SentoriNSE')` / `updateBuildProperty(..., 'SentoriNSE')`
  // miss the target via string equality. Patch the buildSettings dictionary
  // for the NSE build configurations directly off the project hash.
  const settings = {
    CLANG_ENABLE_MODULES: 'YES',
    CODE_SIGN_STYLE: 'Automatic',
    IPHONEOS_DEPLOYMENT_TARGET: deploymentTarget,
    SWIFT_VERSION: '5.0',
    TARGETED_DEVICE_FAMILY: '"1,2"',
  }
  // Mirror the host app's signing team onto the NSE target. Without
  // DEVELOPMENT_TEAM, `expo run:ios --device` falls back to the user's
  // personal Apple-ID team and Xcode can't issue a provisioning profile
  // for `<mainBundleId>.SentoriNSE` under the project's actual team —
  // fails with "No profiles for ... were found". If the host hasn't
  // configured a team (manual-signing projects), leave it unset so the
  // user's existing flow keeps working.
  if (appleTeamId) settings.DEVELOPMENT_TEAM = appleTeamId
  const nseTarget = pbxproj.hash.project.objects.PBXNativeTarget[target.uuid]
  const configListUuid = nseTarget.buildConfigurationList
  const configList = pbxproj.hash.project.objects.XCConfigurationList[configListUuid]
  for (const { value: configUuid } of configList.buildConfigurations) {
    const config = pbxproj.hash.project.objects.XCBuildConfiguration[configUuid]
    Object.assign(config.buildSettings, settings)
  }

  return true
}

/**
 * @param {import('@expo/config-plugins').ExpoConfig} config
 */
const withSentoriNSETarget = (config) =>
  withXcodeProject(config, (cfg) => {
    const mainBundleId = cfg.ios?.bundleIdentifier
    if (!mainBundleId) {
      // Skip silently rather than throw — a host without an iOS
      // bundleIdentifier configured is mid-setup, not broken.
      // eslint-disable-next-line no-console
      console.warn(
        '[sentori-expo] ios.bundleIdentifier not set; skipping NSE target injection. Add it to app.json and re-prebuild.'
      )
      return cfg
    }
    // Match the host app's deployment target so the NSE links against
    // the same minimum iOS. Multi-source fallback because cfg.ios?.deploymentTarget
    // is not reliably populated — Expo readers prefer the
    // `expo-build-properties` config plugin's value or the Podfile,
    // not the top-level ios field. 15.1 matches Expo SDK 55's default.
    const deploymentTarget =
      cfg.ios?.deploymentTarget ?? cfg.ios?.infoPlist?.MinimumOSVersion ?? '15.1'
    const appleTeamId = cfg.ios?.appleTeamId
    injectNSETarget(cfg.modResults, { appleTeamId, deploymentTarget, mainBundleId })
    return cfg
  })

// ── Composer ───────────────────────────────────────────────────────

/**
 * @param {import('@expo/config-plugins').ExpoConfig} config
 * @param {{ sdkVersion?: string, ios?: boolean, android?: boolean, nse?: boolean, googleServicesFile?: string }} [props]
 */
const withSentori = (config, props = {}) => {
  const plugins = [[withSentoriVersion, props]]
  if (props.ios !== false) {
    plugins.push([withSentoriPushIos, props])
    if (props.nse !== false) {
      plugins.push([withSentoriNSE, props], [withSentoriNSETarget, props])
    }
  }
  if (props.android !== false) {
    plugins.push(
      [withSentoriPushAndroidManifest, props],
      [withSentoriPushAndroidGradle, props],
      [withSentoriGoogleServicesJson, props]
    )
  }
  return withPlugins(config, plugins)
}

module.exports = withSentori
// Exported for unit-test coverage of the pure helpers.
module.exports.injectNSETarget = injectNSETarget
module.exports.syncNSEPlistVersion = syncNSEPlistVersion
module.exports.NSE_TARGET = NSE_TARGET
