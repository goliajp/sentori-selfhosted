// Metro config for the RN example app.
//
// Why this isn't the Expo default:
//   1. `@goliapkg/sentori-react-native` is linked from
//      `file:../../sdk/react-native`. Metro doesn't follow symlinks
//      and only watches the project root by default, so we
//      explicitly add the SDK directory as a watch root +
//      enable unstable_enableSymlinks.
//   2. The dev box runs other RN apps on port 8081, so we pin Metro
//      to 9090. The iOS .app reads `RCT_METRO_PORT` from
//      `ios/.xcode.env.local` at build time and bakes the URL in.
//      Both numbers must match.

const { getDefaultConfig } = require('expo/metro-config');
const path = require('path');

const projectRoot = __dirname;
// `apps/rn-example/` → `sdk/react-native/`
const sdkRoot = path.resolve(projectRoot, '..', '..', 'sdk', 'react-native');
const repoRoot = path.resolve(projectRoot, '..', '..');

const config = getDefaultConfig(projectRoot);

config.watchFolders = [sdkRoot, repoRoot];
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(sdkRoot, 'node_modules'),
  path.resolve(repoRoot, 'node_modules'),
];
config.resolver.unstable_enableSymlinks = true;
config.resolver.unstable_enablePackageExports = true;

config.server = {
  ...(config.server ?? {}),
  port: 9090,
};

module.exports = config;
