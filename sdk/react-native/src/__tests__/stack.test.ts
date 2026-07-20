import { describe, it, expect } from 'bun:test';

import { parseStack } from '../stack';

describe('parseStack', () => {
  it('returns [] for undefined', () => {
    expect(parseStack(undefined)).toEqual([]);
  });

  it('returns [] for empty string', () => {
    expect(parseStack('')).toEqual([]);
  });

  it('parses V8/Hermes-style frames', () => {
    const stack = `TypeError: Cannot read property 'foo' of undefined
    at handleSubmit (src/screens/Checkout.tsx:42:10)
    at onPress (src/components/Button.tsx:15:5)`;
    const frames = parseStack(stack);
    expect(frames).toHaveLength(2);
    // Phase 21: core's parseStack also populates absolutePath, so we
    // match the subset of fields rather than full equality.
    expect(frames[0]).toMatchObject({
      column: 10,
      file: 'src/screens/Checkout.tsx',
      function: 'handleSubmit',
      inApp: true,
      line: 42,
    });
    expect(frames[1]?.function).toBe('onPress');
  });

  it('marks node_modules frames as not inApp', () => {
    const stack = `Error
    at vendorFn (node_modules/react-native/Libraries/Foo.js:1:1)`;
    const frames = parseStack(stack);
    expect(frames[0]?.inApp).toBe(false);
  });

  it('marks http(s) urls as not inApp', () => {
    const stack = `Error
    at someFn (https://cdn.example.com/bundle.js:1:1)`;
    const frames = parseStack(stack);
    expect(frames[0]?.inApp).toBe(false);
  });
});
