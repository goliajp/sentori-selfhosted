// A receipt is proof of this test, not merely traffic in the project.
import assert from 'node:assert/strict';
import { api } from '../src/lib/api.ts';
import { findSetupEvent, hasReadableSource } from '../src/lib/setup.ts';
import { setupSnippets } from '../src/lib/setup-snippets.ts';

const originalFetch = globalThis.fetch;
const requests = [];
let match = false;
globalThis.fetch = async (input) => {
  const url = new URL(input, 'http://test');
  requests.push(url);
  if (url.pathname === '/admin/api/issues') {
    assert.equal(url.searchParams.get('project_id'), 'p');
    assert.equal(url.searchParams.get('context_value'), 'this-test');
    return Response.json({ issues: [{ id: 'i', projectId: 'p' }, { id: 'foreign', projectId: 'other' }] });
  }
  if (url.pathname === '/admin/api/issues/i/events') return Response.json({ events: [{ id: 'old' }, { id: 'new' }] });
  if (url.pathname.startsWith('/admin/api/events/')) return Response.json({
    projectId: 'p', payload: { context: { sentoriSetup: match && url.pathname.endsWith('/new') ? 'this-test' : 'old-test' } },
  });
  throw new Error(`Unexpected request ${url}`);
};
try {
  assert.equal(await findSetupEvent('p', 'this-test'), null);
  match = true;
  assert.equal((await findSetupEvent('p', 'this-test')).issueId, 'i');
  assert.ok(!requests.some((r) => r.pathname.includes('foreign')));
  for (const stack of [undefined, {}, [null], [{ file: 'app.js', line: 1 }], [{ file: '?', line: 1, symbolicated: true }]]) {
    assert.equal(hasReadableSource({ payload: { error: { stack } } }), false);
  }
  assert.equal(hasReadableSource({ payload: { error: { stack: [{ file: 'src/App.tsx', line: 10, symbolicated: true }] } } }), true);
  assert.equal(hasReadableSource({ payload: { error: { stack: [{ file: 'App.swift', line: 10, contextLine: 'work()' }] } } }), true);
  const special = 'quote"\\\n$token';
  for (const platform of ['react-native', 'web', 'weapp']) {
    const snippets = setupSnippets(platform, special, 'https://ingest.test', 'app@1', 'this-test');
    const executable = snippets.configure.replace(/^import .*\n/, '');
    let config;
    new Function('sentori', 'App', executable)({ init: (value) => { config = value; } }, (app) => app.onLaunch());
    assert.equal(config.token, special);
  }
  assert.ok(setupSnippets('android', '$token', '', '', '').configure.includes('\\$token'));
  globalThis.fetch = async () => Response.json({ error: 'unauthenticated' }, { status: 401 });
  assert.equal(await api.currentUser(), null);
  globalThis.fetch = async () => Response.json({ error: 'offline' }, { status: 503 });
  await assert.rejects(api.currentUser(), (error) => error.status === 503);
  console.log('✓ setup rejects unrelated traffic; source proof, copied JS, escaping and visitor auth checked');
} finally { globalThis.fetch = originalFetch; }
