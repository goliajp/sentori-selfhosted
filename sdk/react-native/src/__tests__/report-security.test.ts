// v1.1 chunk S2 — reportSecurity + reportPinMismatch round-trip.

import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  mock,
} from 'bun:test';

import { __resetForTests as resetConfig, setConfig } from '../config';
import { __resetInstallIdForTests, __setInstallIdForTests } from '../install-id';
import { setUser } from '../capture';
import { reportPinMismatch, reportSecurity } from '../report-security';

const originalFetch = globalThis.fetch;

describe('reportSecurity', () => {
  beforeEach(() => {
    resetConfig();
    __resetInstallIdForTests();
    setConfig({
      token: 'st_pk_test',
      release: 'app@1.0.0+1',
      environment: 'test',
      ingestUrl: 'http://localhost:8080',
      enabled: true,
    });
    setUser({ id: 'u_demo' });
    __setInstallIdForTests('install-abc');
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    setUser(null);
  });

  it('posts the report envelope and returns the server id', async () => {
    const calls: { body: string; url: string }[] = [];
    globalThis.fetch = mock(async (url: unknown, init: unknown) => {
      calls.push({
        body: String((init as { body?: unknown })?.body ?? ''),
        url: String(url),
      });
      return new Response(JSON.stringify({ id: 'sec-1' }), { status: 202 });
    }) as unknown as typeof fetch;

    const id = await reportSecurity('root.detected', { detector: 'rootbeer' });
    expect(id).toBe('sec-1');
    expect(calls.length).toBe(1);
    expect(calls[0].url).toBe('http://localhost:8080/v1/security:report');
    const parsed = JSON.parse(calls[0].body) as {
      data: Record<string, unknown>;
      installId: string;
      kind: string;
      release: string;
      userId: string;
    };
    expect(parsed.kind).toBe('root.detected');
    expect(parsed.data.detector).toBe('rootbeer');
    expect(parsed.installId).toBe('install-abc');
    expect(parsed.userId).toBe('u_demo');
    expect(parsed.release).toBe('app@1.0.0+1');
  });

  it('reportPinMismatch flattens to pin.mismatch with serverName', async () => {
    const calls: { body: string }[] = [];
    globalThis.fetch = mock(async (_url: unknown, init: unknown) => {
      calls.push({ body: String((init as { body?: unknown })?.body ?? '') });
      return new Response(JSON.stringify({ id: 'sec-2' }), { status: 202 });
    }) as unknown as typeof fetch;

    const id = await reportPinMismatch({
      expected: 'sha256/AAAA',
      observed: 'sha256/BBBB',
      serverName: 'api.example.com',
    });
    expect(id).toBe('sec-2');
    const parsed = JSON.parse(calls[0].body) as {
      data: { expected: string; observed: string };
      kind: string;
      serverName: string;
    };
    expect(parsed.kind).toBe('pin.mismatch');
    expect(parsed.serverName).toBe('api.example.com');
    expect(parsed.data.expected).toBe('sha256/AAAA');
    expect(parsed.data.observed).toBe('sha256/BBBB');
  });

  it('returns null on transport failure rather than throwing', async () => {
    globalThis.fetch = mock(async () => {
      throw new Error('offline');
    }) as unknown as typeof fetch;

    const id = await reportSecurity('arbitrary.kind');
    expect(id).toBe(null);
  });

  it('drops bad inputs silently', async () => {
    const id1 = await reportSecurity('');
    const id2 = await reportPinMismatch({ expected: 'a', observed: 'b', serverName: '' });
    expect(id1).toBe(null);
    expect(id2).toBe(null);
  });
});
