/**
 * v2.3 W6.3 — Sentry-compat translation layer coverage.
 *
 * We exercise the pure-function pieces directly via the test hooks
 * exported from the compat module (parseDsn, mapCategoryToType,
 * mapLevel). The stateful pieces — init / captureException /
 * setUser dispatching to the native init + scope — are covered by
 * the upstream Sentori-native tests (sdk.test, before-send.test);
 * the compat layer only re-shapes the call arguments before
 * delegating, so testing the re-shape is enough.
 */
import { describe, expect, test } from 'bun:test';

import {
  __mapCategoryToTypeForTests,
  __mapLevelForTests,
  __parseDsnForTests,
  Severity,
} from '../compat/sentry';

describe('parseDsn', () => {
  test('extracts token + ingestUrl from a Sentori DSN', () => {
    const r = __parseDsnForTests(
      'https://st_pk_testabcdef@ingest.sentori.golia.jp/1',
    );
    expect(r.token).toBe('st_pk_testabcdef');
    expect(r.ingestUrl).toBe('https://ingest.sentori.golia.jp');
  });

  test('honours custom port + host', () => {
    const r = __parseDsnForTests('https://st_pk_x@self-hosted.example.com:8443/42');
    expect(r.token).toBe('st_pk_x');
    expect(r.ingestUrl).toBe('https://self-hosted.example.com:8443');
  });

  test('rejects non-Sentori token prefix with a clear pointer', () => {
    expect(() =>
      __parseDsnForTests('https://abcd1234@sentry.io/12345'),
    ).toThrow(/st_pk_/);
  });

  test('rejects DSN without user-info (no token)', () => {
    expect(() => __parseDsnForTests('https://sentry.io/12345')).toThrow();
  });

  test('rejects malformed URL', () => {
    expect(() => __parseDsnForTests('not-a-url')).toThrow(/not a valid URL/);
  });
});

describe('mapCategoryToType', () => {
  test('user-interaction categories → user', () => {
    for (const c of ['auth', 'click', 'gesture', 'input', 'touch', 'ui']) {
      expect(__mapCategoryToTypeForTests(c)).toBe('user');
    }
  });

  test('network categories → net', () => {
    for (const c of ['fetch', 'http', 'xhr']) {
      expect(__mapCategoryToTypeForTests(c)).toBe('net');
    }
  });

  test('navigation categories → nav', () => {
    for (const c of ['nav', 'navigation', 'route']) {
      expect(__mapCategoryToTypeForTests(c)).toBe('nav');
    }
  });

  test('log-shaped categories → log', () => {
    for (const c of ['console', 'log', 'sentry']) {
      expect(__mapCategoryToTypeForTests(c)).toBe('log');
    }
  });

  test('unknown category → custom', () => {
    expect(__mapCategoryToTypeForTests('whatever')).toBe('custom');
  });

  test('undefined → undefined (pass-through to native default)', () => {
    expect(__mapCategoryToTypeForTests(undefined)).toBeUndefined();
  });
});

describe('mapLevel', () => {
  test("Sentry's 'critical' maps to Sentori's 'fatal'", () => {
    expect(__mapLevelForTests('critical')).toBe('fatal');
  });

  test("Sentry's 'log' maps to Sentori's 'info' (no separate Log level)", () => {
    expect(__mapLevelForTests('log')).toBe('info');
  });

  test('5-level syslog names pass through unchanged', () => {
    expect(__mapLevelForTests('fatal')).toBe('fatal');
    expect(__mapLevelForTests('error')).toBe('error');
    expect(__mapLevelForTests('warning')).toBe('warning');
    expect(__mapLevelForTests('info')).toBe('info');
    expect(__mapLevelForTests('debug')).toBe('debug');
  });

  test('undefined → undefined', () => {
    expect(__mapLevelForTests(undefined)).toBeUndefined();
  });
});

describe('Severity export', () => {
  test('Critical collapses onto fatal', () => {
    expect(Severity.Critical).toBe('fatal');
  });
  test('Log collapses onto info', () => {
    expect(Severity.Log).toBe('info');
  });
  test('standard levels are themselves', () => {
    expect(Severity.Fatal).toBe('fatal');
    expect(Severity.Error).toBe('error');
    expect(Severity.Warning).toBe('warning');
    expect(Severity.Info).toBe('info');
    expect(Severity.Debug).toBe('debug');
  });
});
