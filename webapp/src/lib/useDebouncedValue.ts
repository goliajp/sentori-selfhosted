import { useEffect, useState } from 'react';

/** Initial URL state loads immediately; subsequent typing waits for a quiet gap. */
export function useDebouncedValue<T>(value: T, milliseconds: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(value), milliseconds);
    return () => window.clearTimeout(timer);
  }, [value, milliseconds]);
  return settled;
}
