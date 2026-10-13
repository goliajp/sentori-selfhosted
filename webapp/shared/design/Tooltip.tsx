import { cloneElement, useEffect, useLayoutEffect, useId, useRef, useState, type HTMLAttributes, type ReactElement } from 'react';
import { createPortal } from 'react-dom';

/** A portal keeps rail hints visible outside the rail's scrolling/clipping box. */
export function Tooltip({ label, children }: { label: string; children: ReactElement<HTMLAttributes<HTMLElement>> }) {
  const id = useId();
  const [position, setPosition] = useState<{ left: number; top: number; side: 'left' | 'right' } | null>(null);
  const openTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const focused = useRef(false);
  const bubble = useRef<HTMLSpanElement>(null);
  function cancelTimers() { clearTimeout(openTimer.current); clearTimeout(closeTimer.current); }
  function close() { cancelTimers(); setPosition(null); }
  function show(target: HTMLElement, delay: number) {
    cancelTimers();
    const display = () => {
      if (!target.isConnected) return;
      const trigger = target.firstElementChild;
      if (!(trigger instanceof HTMLElement)) return;
      const bounds = trigger.getBoundingClientRect();
      const side = bounds.right > window.innerWidth / 2 ? 'left' : 'right';
      setPosition({ left: side === 'left' ? bounds.left : bounds.right, top: bounds.top + bounds.height / 2, side });
    };
    if (delay) openTimer.current = setTimeout(display, delay);
    else display();
  }
  function leave() {
    clearTimeout(openTimer.current);
    clearTimeout(closeTimer.current);
    if (!focused.current) closeTimer.current = setTimeout(close, 150);
  }
  useLayoutEffect(() => {
    if (!position || !bubble.current) return;
    const bounds = bubble.current.getBoundingClientRect();
    const inset = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--space-2')) || 8;
    const dx = Math.max(inset - bounds.left, Math.min(0, innerWidth - inset - bounds.right));
    const dy = Math.max(inset - bounds.top, Math.min(0, innerHeight - inset - bounds.bottom));
    bubble.current.style.left = `${position.left + dx}px`;
    bubble.current.style.top = `${position.top + dy}px`;
  }, [position]);
  useEffect(() => () => { clearTimeout(openTimer.current); clearTimeout(closeTimer.current); }, []);
  useEffect(() => {
    if (!position) return;
    const dismiss = () => { clearTimeout(openTimer.current); clearTimeout(closeTimer.current); setPosition(null); };
    const key = (event: KeyboardEvent) => { if (event.key === 'Escape') dismiss(); };
    document.addEventListener('keydown', key);
    window.addEventListener('resize', dismiss);
    window.addEventListener('scroll', dismiss, true);
    return () => { document.removeEventListener('keydown', key); window.removeEventListener('resize', dismiss); window.removeEventListener('scroll', dismiss, true); };
  }, [position]);
  return <span className="tooltip-anchor"
    onPointerEnter={event => { if (event.pointerType !== 'touch') show(event.currentTarget, 200); }}
    onPointerLeave={leave}
    onFocus={event => { focused.current = true; show(event.currentTarget, 0); }}
    onBlur={() => { focused.current = false; close(); }}
    onClick={close}>
    {cloneElement(children, {
      title: undefined,
      'aria-describedby': [children.props['aria-describedby'], position ? id : ''].filter(Boolean).join(' ') || undefined,
    })}
    {position && createPortal(<span ref={bubble} id={id} role="tooltip" className="control-tooltip" data-side={position.side} style={{ left: position.left, top: position.top }}
      onPointerEnter={() => clearTimeout(closeTimer.current)} onPointerLeave={leave}>{label}</span>, document.body)}
  </span>;
}
