// Push registration: never throws, and names why it failed.
//
// There were no push tests in this package at all. That is the
// reason three unbalanced INSERTs on the server and two field
// mismatches in this file survived a year: every one of them made
// registration fail, and nothing anywhere asserted that registration
// succeeds.

import { afterEach, beforeEach, describe, expect, it } from 'bun:test';

import { __setNativeForTests } from '../native';
import {
  __resetForTests as resetPush,
  __setPlatformForTests as setPlatform,
  register,
  unregister,
} from '../push';
import { __resetForTests as resetConfig, setConfig } from '../config';
import { __resetForTests as resetScope, setUser } from '../scope';

const baseConfig = {
  token: 'st_test',
  ingestUrl: 'http://localhost:18080',
  release: 'app@1.0.0',
  environment: 'test',
  enabled: true,
  detect: { rageTap: false, longFreeze: false, slowColdStart: false, slowApi: false },
  replaySeconds: 30,
};

/** A native module that grants permission and hands back a token on
 *  the first drain — the shape the real one has when everything
 *  works. Override pieces per test. */
function grantingNative(over: Record<string, unknown> = {}) {
  return {
    pushRequestPermission: () => Promise.resolve('granted'),
    pushGetStatus: () => Promise.resolve('granted'),
    pushRegister: () => undefined,
    pushUnregister: () => undefined,
    pushDrainState: () => Promise.resolve({ token: 'abc123', notifications: [], taps: [] }),
    ...over,
  };
}

const realFetch = globalThis.fetch;

/** Answer /v1/push/devices with `body` under `status`. */
function stubFetch(status: number, body: unknown): void {
  globalThis.fetch = (() =>
    Promise.resolve({
      ok: status >= 200 && status < 300,
      status,
      json: () => Promise.resolve(body),
    })) as unknown as typeof fetch;
}

describe('push.register', () => {
  beforeEach(() => {
    resetPush();
    resetConfig();
    resetScope();
    setPlatform('ios');
    setConfig(baseConfig);
  });
  afterEach(() => {
    resetPush();
    resetConfig();
    resetScope();
    setPlatform(null);
    __setNativeForTests(undefined);
    globalThis.fetch = realFetch;
  });

  it('returns the device handle when the whole flow works', async () => {
    __setNativeForTests(grantingNative());
    stubFetch(200, { spToken: '018f0000-0000-7000-8000-000000000001', isNew: true });

    const r = await register();

    expect(r.ok).toBe(true);
    if (r.ok) expect(r.ipt).toBe('018f0000-0000-7000-8000-000000000001');
  });

  it('sends `kind`, not `provider` — the field name that 422d for a year', async () => {
    __setNativeForTests(grantingNative());
    let sent: Record<string, unknown> = {};
    globalThis.fetch = ((_url: string, init: { body: string }) => {
      sent = JSON.parse(init.body) as Record<string, unknown>;
      return Promise.resolve({
        ok: true,
        status: 200,
        json: () => Promise.resolve({ spToken: 'id-1' }),
      });
    }) as unknown as typeof fetch;

    await register();

    expect(sent.kind).toBe('apns');
    expect(sent.provider).toBeUndefined();
  });

  it('sends kind=fcm and no env on Android — FCM has no sandbox split', async () => {
    setPlatform('android');
    __setNativeForTests(grantingNative());
    let sent: Record<string, unknown> = {};
    globalThis.fetch = ((_url: string, init: { body: string }) => {
      sent = JSON.parse(init.body) as Record<string, unknown>;
      return Promise.resolve({
        ok: true,
        status: 200,
        json: () => Promise.resolve({ spToken: 'id-1' }),
      });
    }) as unknown as typeof fetch;

    await register();

    expect(sent.kind).toBe('fcm');
    expect('env' in sent).toBe(false);
  });

  it('reports not-initialised rather than throwing when init() has not run', async () => {
    resetConfig();
    __setNativeForTests(grantingNative());

    const r = await register();

    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toBe('not-initialised');
  });

  it('separates "no native module" from "user declined"', async () => {
    __setNativeForTests(null);
    const absent = await register();
    expect(absent.ok).toBe(false);
    if (!absent.ok) expect(absent.reason).toBe('no-transport');

    __setNativeForTests(grantingNative({ pushRequestPermission: () => Promise.resolve('denied') }));
    const declined = await register();
    expect(declined.ok).toBe(false);
    if (!declined.ok) expect(declined.reason).toBe('permission-denied');
  });

  it('reports token-timeout when the OS never hands one back', async () => {
    __setNativeForTests(
      grantingNative({
        pushDrainState: () => Promise.resolve({ notifications: [], taps: [] }),
      }),
    );

    const r = await register({ tokenTimeoutMs: 250 });

    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.reason).toBe('token-timeout');
  });

  it('reports server-rejected on a non-2xx, and on a 2xx with no id', async () => {
    __setNativeForTests(grantingNative());

    stubFetch(500, {});
    const five = await register();
    expect(five.ok).toBe(false);
    if (!five.ok) expect(five.reason).toBe('server-rejected');

    // A 200 that carries nothing usable is the same problem wearing a
    // better status code — this is what the SDK used to accept and
    // then throw on, deeper in.
    stubFetch(200, { ok: true });
    const empty = await register();
    expect(empty.ok).toBe(false);
    if (!empty.ok) expect(empty.reason).toBe('server-rejected');
  });

  it('never rejects — the whole point (client zero-cost iron rule)', async () => {
    // Every failure mode in one loop, including a native module that
    // throws out of each of its methods.
    const hostile = [
      null,
      grantingNative({
        pushRequestPermission: () => {
          throw new Error('native exploded');
        },
      }),
      grantingNative({
        pushDrainState: () => Promise.resolve({ error: 'APS denied', notifications: [], taps: [] }),
      }),
    ];
    globalThis.fetch = (() => Promise.reject(new Error('network down'))) as unknown as typeof fetch;

    for (const n of hostile) {
      __setNativeForTests(n as never);
      const r = await register({ tokenTimeoutMs: 150 });
      expect(r.ok).toBe(false);
    }
  });

  it('calls onError but still resolves, so a host can use either style', async () => {
    __setNativeForTests(null);
    let seen: null | string = null;

    const r = await register({ onError: (e) => (seen = e.message) });

    expect(r.ok).toBe(false);
    expect(seen).not.toBeNull();
  });
});

// A device registers at launch; the person signs in ten seconds
// later. Until now nothing updated the row, so it carried no user for
// the life of the install — and a send aimed at that user reached
// nobody and reported success.
describe('push registration follows the person', () => {
  /** Record every /v1/push/devices body, so a test can say what the
   *  server was told rather than that it was told something. */
  function recordingFetch(): Array<Record<string, unknown>> {
    const seen: Array<Record<string, unknown>> = [];
    globalThis.fetch = ((_url: string, init?: { body?: string }) => {
      if (init?.body != null) {
        seen.push(JSON.parse(init.body) as Record<string, unknown>);
      }
      return Promise.resolve({
        ok: true,
        status: 202,
        json: () => Promise.resolve({ spToken: 'dev-1' }),
      });
    }) as unknown as typeof fetch;
    return seen;
  }

  /** `user()` hashes on a promise, so the key lands a tick later and
   *  the re-registration is sent after that. */
  const settle = () => new Promise((r) => setTimeout(r, 10));

  beforeEach(() => {
    resetPush();
    resetConfig();
    resetScope();
    setPlatform('ios');
    setConfig(baseConfig);
    __setNativeForTests(grantingNative());
  });
  afterEach(() => {
    resetPush();
    resetConfig();
    resetScope();
    setPlatform(null);
    __setNativeForTests(undefined);
    globalThis.fetch = realFetch;
  });

  it('sends the identity when the person signs in after registering', async () => {
    const seen = recordingFetch();
    await register();
    expect(seen).toHaveLength(1);
    expect(seen[0]?.userKey).toBeUndefined();

    setUser({ id: 'usr_123', traits: { plan: 'pro' } });
    await settle();

    // Two assertions, because either alone passes on a mistake: a
    // count alone passes if the update carries nothing, and a key
    // alone passes if the update never happened and this is still the
    // first request.
    expect(seen).toHaveLength(2);
    expect(typeof seen[1]?.userKey).toBe('string');
    expect(seen[1]?.traits).toEqual({ plan: 'pro' });
  });

  it('does not send anything when the same person is set again', async () => {
    const seen = recordingFetch();
    await register();
    setUser({ id: 'usr_123', traits: { plan: 'pro' } });
    await settle();
    const after = seen.length;

    // `user()` is a verb an app may call on every screen. One request
    // per call is not free to a host, and the iron rule is that we are.
    setUser({ id: 'usr_123', traits: { plan: 'pro' } });
    await settle();
    expect(seen).toHaveLength(after);
  });

  it('clears the traits when the person signs out', async () => {
    const seen = recordingFetch();
    await register();
    setUser({ id: 'usr_123', traits: { plan: 'pro' } });
    await settle();
    setUser(null);
    await settle();

    const last = seen[seen.length - 1];
    expect(last?.userKey).toBeUndefined();
    // `{}` rather than absent: absent means "keep what is there", so
    // a signed-out device would stay selectable as a pro user.
    expect(last?.traits).toEqual({});
  });

  it('does not register a device that never registered', async () => {
    const seen = recordingFetch();
    setUser({ id: 'usr_123' });
    await settle();
    expect(seen).toHaveLength(0);
  });
});

// A vendor token is not an address. It rotates — on a reinstall, a
// restore, cleared app data — and the server keys the row on it, so a
// rotation wrote a *new* row under a *new* spToken and every backend
// holding the old one was addressing nothing.
//
// The native SDKs were given an installId and a rotation report. This
// package was not: its registration goes through JS, and the native
// rotation hook it does have refuses to act on a device the *native*
// side never registered — which, for a React Native app, is every
// device.
describe('push survives the vendor rotating its token', () => {
  function recordingFetch(): Array<Record<string, unknown>> {
    const seen: Array<Record<string, unknown>> = [];
    globalThis.fetch = ((_url: string, init?: { body?: string }) => {
      if (init?.body != null) {
        seen.push(JSON.parse(init.body) as Record<string, unknown>);
      }
      return Promise.resolve({
        ok: true,
        status: 202,
        json: () => Promise.resolve({ spToken: 'dev-1' }),
      });
    }) as unknown as typeof fetch;
    return seen;
  }

  beforeEach(() => {
    resetPush();
    resetConfig();
    resetScope();
    setPlatform('ios');
    setConfig(baseConfig);
  });
  afterEach(() => {
    resetPush();
    resetConfig();
    resetScope();
    setPlatform(null);
    __setNativeForTests(undefined);
    globalThis.fetch = realFetch;
  });

  it('tells the server which installation this is', async () => {
    __setNativeForTests(grantingNative());
    const seen = recordingFetch();
    await register();
    const installId = seen[0]?.installId;
    expect(typeof installId).toBe('string');
    expect(String(installId).length).toBeGreaterThan(8);
  });

  it('keeps the same installation across registrations', async () => {
    __setNativeForTests(grantingNative());
    const seen = recordingFetch();
    await register();
    await register();
    expect(seen).toHaveLength(2);
    // Asserting equality alone passes when neither call sent one —
    // `undefined === undefined` — which is the state this test
    // exists to reject. So: a string, and the same string.
    expect(typeof seen[0]?.installId).toBe('string');
    expect(seen[1]?.installId).toBe(seen[0]?.installId);
  });

  it('reports a token that rotated while the app was running', async () => {
    // The vendor hands back a different token on the second drain,
    // which is what a rotation looks like from here.
    let handed = 0;
    __setNativeForTests(
      grantingNative({
        pushDrainState: () => {
          handed += 1;
          return Promise.resolve({
            notifications: [],
            taps: [],
            token: handed <= 1 ? 'abc123' : 'rotated456',
          });
        },
      }),
    );
    const seen = recordingFetch();
    await register();
    expect(seen).toHaveLength(1);

    // One turn of the drain loop, which already runs at 1 Hz and
    // already reads the token — it just threw it away.
    await new Promise((r) => setTimeout(r, 1400));

    expect(seen).toHaveLength(2);
    expect(seen[1]?.nativeToken).toBe('rotated456');
    // Same installation, or the server writes a second row and the
    // rotation has achieved exactly what it used to.
    expect(seen[1]?.installId).toBe(seen[0]?.installId);
  });

  it('does not report the same token twice', async () => {
    __setNativeForTests(grantingNative());
    const seen = recordingFetch();
    await register();
    await new Promise((r) => setTimeout(r, 2400));
    expect(seen).toHaveLength(1);
  });
});

// Push is allowed to fail silently. It is never allowed to make the
// host app fail — a notification that does not arrive is a product
// decision the host can live with; an exception out of an SDK the host
// merely opted into is not.
//
// The host's own handlers are the sharp edge: they are its code, they
// run inside our loop, and JavaScript throws for a living.
describe('nothing the host does can reach back into the app', () => {
  const boom = () => {
    throw new Error('the host threw');
  };

  /** Hands the batch over once and is empty after, the way the real
   *  native drain is — a stub that keeps re-delivering makes a second
   *  tick look like a bug in the SDK. */
  function drainingNative(notifications: unknown[], taps: unknown[]) {
    let drained = false;
    return grantingNative({
      pushDrainState: () => {
        const batch = drained
          ? { notifications: [], taps: [] }
          : { notifications, taps };
        drained = true;
        return Promise.resolve({ ...batch, token: 'abc123' });
      },
    });
  }

  function okFetch(): void {
    globalThis.fetch = (() =>
      Promise.resolve({
        ok: true,
        status: 202,
        json: () => Promise.resolve({ spToken: 'dev-1' }),
      })) as unknown as typeof fetch;
  }

  beforeEach(() => {
    resetPush();
    resetConfig();
    resetScope();
    setPlatform('ios');
    setConfig(baseConfig);
    okFetch();
  });
  afterEach(() => {
    resetPush();
    resetConfig();
    resetScope();
    setPlatform(null);
    __setNativeForTests(undefined);
    globalThis.fetch = realFetch;
  });

  it('keeps delivering after a handler throws', async () => {
    const seen: string[] = [];
    __setNativeForTests(
      drainingNative(
        [{ title: 'one' }, { title: 'two' }],
        [{ tapped: true }],
      ),
    );
    const r = await register({
      onMessage: (n) => {
        seen.push(String(n.title));
        // The first one throws. The second one, and the tap after it,
        // still have to arrive: one bad handler must not swallow the
        // rest of the batch.
        if (n.title === 'one') boom();
      },
      onTap: () => seen.push('tap'),
    });
    expect(r.ok).toBe(true);
    await new Promise((res) => setTimeout(res, 1400));
    expect(seen).toEqual(['one', 'two', 'tap']);
  });

  it('keeps ticking after a handler throws', async () => {
    let ticks = 0;
    __setNativeForTests(
      grantingNative({
        pushDrainState: () => {
          ticks += 1;
          return Promise.resolve({
            notifications: [{ title: 'n' }],
            taps: [],
            token: 'abc123',
          });
        },
      }),
    );
    await register({ onMessage: boom });
    const first = ticks;
    await new Promise((res) => setTimeout(res, 2400));
    // A throwing handler that stops the loop is worse than one that
    // drops a notification: every later push is gone too, silently.
    expect(ticks).toBeGreaterThan(first + 1);
  });

  it('survives a native module that rejects', async () => {
    let ticks = 0;
    __setNativeForTests(
      grantingNative({
        pushDrainState: () => {
          ticks += 1;
          return ticks === 1
            ? Promise.resolve({ notifications: [], taps: [], token: 'abc123' })
            : Promise.reject(new Error('bridge died'));
        },
      }),
    );
    await register();
    await new Promise((res) => setTimeout(res, 2400));
    expect(ticks).toBeGreaterThan(2);
  });

  it('does not report a failure because onToken threw', async () => {
    __setNativeForTests(grantingNative());
    const r = await register({ onToken: boom });
    // The registration reached the server and came back with a handle.
    // What the host did with it afterwards is not our outcome.
    expect(r).toEqual({ ok: true, ipt: 'dev-1' });
  });

  it('still resolves when onError throws', async () => {
    // `register()` never throws is the documented contract, and the
    // one path that reports failure is the one that hands the host an
    // error to look at.
    __setNativeForTests(
      grantingNative({ pushRequestPermission: () => Promise.resolve('denied') }),
    );
    const r = await register({ onError: boom });
    expect(r.ok).toBe(false);
  });

  it('unregister does not throw when the server is unreachable', async () => {
    __setNativeForTests(grantingNative());
    await register();
    globalThis.fetch = (() => Promise.reject(new Error('offline'))) as unknown as typeof fetch;
    // Signing out must not be something that can fail.
    await unregister();
  });
});
