// What to fetch, what it looks like, and what the wrong file looks like.
//
// Every provider hands out a credential that has a near-identical
// twin, and picking the twin is the single most common way to spend
// an evening on this:
//
//   - Apple's APNs key and Apple's App Store Connect API key are both
//     `AuthKey_<10 chars>.p8`, both PKCS#8 P-256, both ~250 bytes.
//     Nothing in the file distinguishes them. Only Apple can tell,
//     which is why the console asks Apple.
//   - Firebase's service account and `google-services.json` are both
//     JSON downloaded from the Firebase console. This one *is*
//     distinguishable, so it is caught here, before the round trip.
//
// Neither is written out as a warning in the guide. A warning is read
// before the mistake and cannot be acted on at that moment; both are
// said by the thing that catches them, at the moment it catches them —
// `recognise` names google-services.json on paste with the fix, and
// the probe's refusal names the App Store Connect key. Saying it twice
// puts the weaker copy where someone reads it first.
//
// The facts below are deliberately not translated. `AuthKey_*.p8` is
// the same string in every language, and a translated file name is a
// file name somebody will fail to find.

/** A credential the console can hold. */
export type Provider = 'apns' | 'fcm' | 'webpush';

/** Something recognisably wrong with what was pasted, named. */
export type SecretIssue =
  /** `google-services.json` — the app-side config, not a credential. */
  | 'google-services-json'
  /** A certificate, not a private key. */
  | 'is-certificate'
  /** The public half. */
  | 'is-public-key'
  /** JSON, but not a Google service account. */
  | 'json-not-service-account'
  /** Not JSON and not PEM. */
  | 'unrecognised';

/** What a pasted or dropped file turned out to be. */
export type Recognised = {
  /** Which provider this credential belongs to, when it is knowable. */
  provider?: Provider;
  /** Apple puts the Key ID in the file name and nowhere else. */
  keyId?: string;
  /** Google puts the project in the file. */
  projectId?: string;
  /** Named, when it is the wrong file. */
  issue?: SecretIssue;
};

/**
 * Read a pasted secret, and the name of the file it came from.
 *
 * Pure and total: every input produces a verdict, and an input it
 * cannot place is `unrecognised` rather than a guess. Guessing here
 * is worse than not knowing — a wrong provider silently sends the
 * credential down the wrong validator and the error names the wrong
 * field.
 */
export function recognise(text: string, filename?: string): Recognised {
  const out: Recognised = {};

  // Apple's Key ID lives in the file name. It is a ten-character
  // alphanumeric, and the field is otherwise typed by hand off a web
  // page, which is where transposed characters come from.
  const keyId = /^AuthKey_([A-Z0-9]{10})\.p8$/i.exec(filename?.trim() ?? '')?.[1];
  if (keyId !== undefined) out.keyId = keyId.toUpperCase();

  const body = text.trim();
  if (body.length === 0) return out;

  if (body.startsWith('{')) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(body);
    } catch {
      return { ...out, issue: 'json-not-service-account' };
    }
    const obj = (parsed ?? {}) as Record<string, unknown>;
    if (obj.type === 'service_account') {
      return {
        ...out,
        provider: 'fcm',
        projectId: typeof obj.project_id === 'string' ? obj.project_id : undefined,
      };
    }
    // The twin. Named specifically, because "invalid JSON" sends
    // someone back to the same download button.
    if ('project_info' in obj || 'client' in obj) {
      return { ...out, issue: 'google-services-json' };
    }
    return { ...out, issue: 'json-not-service-account' };
  }

  if (body.startsWith('-----BEGIN')) {
    if (body.includes('BEGIN CERTIFICATE')) return { ...out, issue: 'is-certificate' };
    if (body.includes('BEGIN PUBLIC KEY')) return { ...out, issue: 'is-public-key' };
    // A private key. Which of Apple's two it is cannot be told from
    // here — that is what the probe is for.
    return { ...out, provider: 'apns' };
  }

  return { ...out, issue: 'unrecognised' };
}

/** One line of the "what you will get" spec. */
export type SpecRow = {
  /** i18n key for the label in the gutter. */
  label: string;
  /** The fact itself. Not translated — it is a file name or a URL. */
  value: string;
};

/** How to obtain one credential, as a spec rather than a paragraph. */
export type ProviderSpec = {
  provider: Provider;
  /** i18n key: what this provider is called in the console. */
  title: string;
  /** Where the operator goes. Shown as a link. */
  href?: string;
  /** The clicks, as i18n keys. Numbered in the UI. */
  steps: string[];
  /** File name, size, first line — the things that let someone
   *  recognise the download before pasting it. */
  spec: SpecRow[];
};

export const PROVIDER_SPECS: ProviderSpec[] = [
  {
    provider: 'apns',
    title: 'push.specApnsTitle',
    href: 'https://developer.apple.com/account/resources/authkeys/list',
    steps: ['push.specApnsStep1', 'push.specApnsStep2', 'push.specApnsStep3'],
    spec: [
      { label: 'push.specFile', value: 'AuthKey_XXXXXXXXXX.p8' },
      { label: 'push.specFirstLine', value: '-----BEGIN PRIVATE KEY-----' },
      { label: 'push.specSize', value: '~250 B' },
    ],
  },
  {
    provider: 'fcm',
    title: 'push.specFcmTitle',
    href: 'https://console.firebase.google.com/',
    steps: ['push.specFcmStep1', 'push.specFcmStep2', 'push.specFcmStep3'],
    spec: [
      { label: 'push.specFile', value: '<project>-<hash>.json' },
      { label: 'push.specMarker', value: '"type": "service_account"' },
      { label: 'push.specSize', value: '~2.3 KB' },
    ],
  },
  {
    provider: 'webpush',
    title: 'push.specWebpushTitle',
    steps: ['push.specWebpushStep1', 'push.specWebpushStep2'],
    spec: [
      { label: 'push.specMarker', value: 'P-256 (prime256v1)' },
      { label: 'push.specFirstLine', value: '-----BEGIN PRIVATE KEY-----' },
    ],
  },
];

/** The verdict's colour role, reusing the five kind hues the app
 *  already re-inks for both themes. */
export function verdictTone(status: null | string | undefined): string {
  switch (status) {
    case 'ok':
      return 'text-kind-probe';
    case 'limited':
      return 'text-kind-warn';
    case 'rejected':
      return 'text-kind-error';
    default:
      return 'text-fg-subtle';
  }
}
