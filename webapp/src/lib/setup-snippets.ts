import type { SetupPlatform } from './setup';

export function setupSnippets(platform: SetupPlatform, token: string, origin: string, release: string, checkId: string) {
  const q = (value: string) => platform === 'android' ? JSON.stringify(value).replaceAll('$', '\\$') : JSON.stringify(value);
  const values = { token: q(token), origin: q(origin), release: q(release), check: q(checkId) };
  if (platform === 'ios') {
    return {
      install: 'https://github.com/goliajp/sentori-swift',
      configure: `import Sentori\n\nSentori.start(SentoriConfig(\n    token: ${values.token},\n    ingestUrl: ${values.origin},\n    release: ${values.release},\n    environment: "development"\n))`,
      test: `import Foundation\n\nSentori.context(["sentoriSetup": ${values.check}])\nSentori.error(message: "Sentori connection check")\nSentori.context(["sentoriSetup": NSNull()])`,
    };
  }
  if (platform === 'android') {
    // The published dependency version belongs to the native guide;
    // do not duplicate a release pin in a second source of truth.
    return {
      install: 'jp.golia.sentori:sentori',
      configure: `import android.app.Application\nimport com.sentori.Sentori\nimport com.sentori.SentoriConfig\n\nclass App : Application() {\n    override fun onCreate() {\n        super.onCreate()\n        Sentori.start(\n            SentoriConfig(\n                token = ${values.token},\n                ingestUrl = ${values.origin},\n                release = ${values.release},\n                environment = "development",\n            ),\n            context = this,\n        )\n    }\n}`,
      test: `Sentori.context(mapOf("sentoriSetup" to ${values.check}))\nSentori.error("Sentori connection check")\nSentori.context(mapOf("sentoriSetup" to null))`,
    };
  }
  const pkg = `@goliapkg/sentori-${platform}`;
  const init = `sentori.init({\n  token: ${values.token},\n  ingestUrl: ${values.origin},\n  release: ${values.release},\n  environment: 'development',\n})`;
  return {
    install: `npm install ${pkg}`,
    configure: `import { sentori } from '${pkg}'\n\n${platform === 'weapp' ? `App({\n  onLaunch() {\n${init.split('\n').map((line) => `    ${line}`).join('\n')}\n  },\n})` : init}`,
    test: `sentori.context({ sentoriSetup: ${values.check} })\nsentori.error(new Error('Sentori connection check'))\nsentori.context({ sentoriSetup: null })`,
  };
}
