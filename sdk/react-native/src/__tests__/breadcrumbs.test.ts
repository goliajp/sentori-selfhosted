import { describe, it, expect, beforeEach } from 'bun:test';

import {
  addBreadcrumb,
  getBreadcrumbs,
  __resetForTests,
} from '../breadcrumbs';

describe('breadcrumbs', () => {
  beforeEach(() => {
    __resetForTests();
  });

  it('adds and retrieves a breadcrumb', () => {
    addBreadcrumb({ type: 'log', data: { level: 'info', message: 'hi' } });
    const crumbs = getBreadcrumbs();
    expect(crumbs).toHaveLength(1);
    expect(crumbs[0]?.type).toBe('log');
    expect(crumbs[0]?.data.message).toBe('hi');
  });

  it('auto-generates ISO 8601 timestamp when not provided', () => {
    addBreadcrumb({ type: 'nav', data: { from: 'a', to: 'b' } });
    const crumbs = getBreadcrumbs();
    expect(crumbs[0]?.timestamp).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  });

  it('caps at 100 entries (oldest discarded)', () => {
    for (let i = 0; i < 150; i++) {
      addBreadcrumb({ type: 'custom', data: { i } });
    }
    const crumbs = getBreadcrumbs();
    expect(crumbs).toHaveLength(100);
    expect(crumbs[0]?.data.i).toBe(50);
    expect(crumbs[99]?.data.i).toBe(149);
  });

  it('returns a fresh copy on each getBreadcrumbs call', () => {
    addBreadcrumb({ type: 'log', data: {} });
    const a = getBreadcrumbs();
    const b = getBreadcrumbs();
    expect(a).not.toBe(b);
  });
});
