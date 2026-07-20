// Linear-style two-key nav shortcuts: press `g` then a letter
// within 1.5s to navigate. Skips when focus is in an input.

import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';

/// Page-level single-key handler. Caller passes a map of
/// { key: handler }; we skip when focus is in an editable.
/// The listener registers once; the ref always sees the latest map
/// (callers pass a fresh object literal every render).
export function useKeyHandlers(map: Record<string, () => void>) {
  const stable = useRef(map);
  stable.current = map;
  useEffect(() => {
    function inEditable(): boolean {
      const a = document.activeElement;
      if (!a) return false;
      const tag = a.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
      if ((a as HTMLElement).isContentEditable) return true;
      return false;
    }
    function onKey(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (inEditable()) return;
      const fn = stable.current[e.key];
      if (fn) {
        e.preventDefault();
        fn();
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
}

const SHORTCUTS: Record<string, string> = {
  i: '/',
  p: '/projects',
  m: '/members',
  a: '/alerts',
  v: '/saved-views',
  u: '/audit',
  s: '/settings',
  h: '/health',
  o: '/saas',
  n: '/notifications',
  '/': '/search',
};

export function useNavShortcuts() {
  const navigate = useNavigate();
  useEffect(() => {
    let armed = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    function disarm() {
      armed = false;
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
    }

    function inEditable(): boolean {
      const a = document.activeElement;
      if (!a) return false;
      const tag = a.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') {
        return true;
      }
      if ((a as HTMLElement).isContentEditable) return true;
      return false;
    }

    function onKey(e: KeyboardEvent) {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (inEditable()) return;

      if (e.key === 'g' && !armed) {
        armed = true;
        timer = setTimeout(disarm, 1500);
        return;
      }
      if (armed) {
        const target = SHORTCUTS[e.key];
        disarm();
        if (target) {
          e.preventDefault();
          navigate(target);
        }
      }
    }

    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
      disarm();
    };
  }, [navigate]);
}
