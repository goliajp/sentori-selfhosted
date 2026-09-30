import { afterEach, describe, expect, test } from 'bun:test';

import { parseGqlOpName } from '../handlers/network';

// v0.9.0 #11 — covers the GraphQL operation extraction logic. The
// patched fetch / XHR plumbing is exercised by manual smoke; this file
// nails the parser, which is the part with branchy logic.

afterEach(() => {
  // nothing — parseGqlOpName is pure.
});

describe('parseGqlOpName', () => {
  test('extracts operationName from a standard Apollo POST body', () => {
    const body = JSON.stringify({
      query: 'query UpdateCart($id:ID!){...}',
      operationName: 'UpdateCart',
      variables: { id: 'c-1' },
    });
    expect(parseGqlOpName(body)).toBe('UpdateCart');
  });

  test('extracts operationName from a batched array body (Apollo Link Batch)', () => {
    const body = JSON.stringify([
      { query: '...', operationName: 'FirstOp', variables: {} },
      { query: '...', operationName: 'SecondOp', variables: {} },
    ]);
    expect(parseGqlOpName(body)).toBe('FirstOp');
  });

  test('falls back to sniffing the query string when operationName is absent', () => {
    const body = JSON.stringify({
      query: 'mutation CompleteCheckout($id:ID!){...}',
    });
    expect(parseGqlOpName(body)).toBe('CompleteCheckout');
  });

  test('sniffs application/graphql body (no JSON wrapper)', () => {
    const body = 'query   ListOrders {\n  orders { id }\n}';
    expect(parseGqlOpName(body)).toBe('ListOrders');
  });

  test('returns undefined for non-graphql JSON', () => {
    const body = JSON.stringify({ hello: 'world' });
    expect(parseGqlOpName(body)).toBeUndefined();
  });

  test('returns undefined for malformed JSON', () => {
    expect(parseGqlOpName('{not json')).toBeUndefined();
  });

  test('returns undefined for an empty body', () => {
    expect(parseGqlOpName('')).toBeUndefined();
  });

  test('rejects bodies larger than 8 KB', () => {
    const big = JSON.stringify({
      query: 'query Big {...}',
      operationName: 'Big',
      variables: { padding: 'x'.repeat(10_000) },
    });
    expect(parseGqlOpName(big)).toBeUndefined();
  });

  test('rejects an operationName that is too long (>200 chars)', () => {
    const long = 'A'.repeat(201);
    const body = JSON.stringify({ query: 'q', operationName: long });
    expect(parseGqlOpName(body)).toBeUndefined();
  });

  test('handles leading comments in raw query body', () => {
    const body = '# a comment\n# another comment\nsubscription LiveTicker {...}';
    expect(parseGqlOpName(body)).toBe('LiveTicker');
  });
});
