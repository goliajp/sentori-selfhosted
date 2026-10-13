import { useLayoutEffect, useRef, type ReactNode } from 'react';

let context: CanvasRenderingContext2D | null = null;
const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
const measurements = new Set<() => (() => void) | undefined>();
let frame: number | undefined;
let fontGeneration = 0;
function scheduleAlignment() {
  if (frame !== undefined || !measurements.size) return;
  frame = requestAnimationFrame(() => {
    frame = undefined;
    // Read all rows before writing: scrolling must not force a layout per label.
    const writes = [...measurements].map(measure => measure());
    for (const write of writes) write?.();
  });
}
function fontsChanged() { fontGeneration++; scheduleAlignment(); }

/** Center the visible glyphs, including an ellipsis, instead of the font's em box. */
function ResourceText({ children, className }: { children: string; className: string }) {
  const box = useRef<HTMLSpanElement>(null);
  const line = useRef<HTMLSpanElement>(null);
  const baseline = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const outer = box.current, text = line.current, anchor = baseline.current;
    if (!outer || !text || !anchor) return;
    let active = true;
    let cachedKey = '', ascent = 0, descent = 0;
    function measure() {
      if (!active || !outer || !text || !anchor || !outer.getClientRects().length) return;
      context ??= document.createElement('canvas').getContext('2d');
      if (!context) return;
      const style = getComputedStyle(text);
      context.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      context.letterSpacing = style.letterSpacing === 'normal' ? '0px' : style.letterSpacing;
      const key = JSON.stringify([children, text.clientWidth, context.font, style.letterSpacing, fontGeneration, window.devicePixelRatio]);
      if (key !== cachedKey) {
        let visible = text.innerText;
        if (text.scrollWidth > text.clientWidth) {
          const letters = [...graphemes.segment(visible)].map(part => part.segment);
          let low = 0, high = letters.length;
          while (low < high) {
            const mid = Math.ceil((low + high) / 2);
            if (context.measureText(letters.slice(0, mid).join('') + '…').width <= text.clientWidth) low = mid;
            else high = mid - 1;
          }
          visible = letters.slice(0, low).join('') + '…';
        }
        const metrics = context.measureText(visible);
        ascent = metrics.actualBoundingBoxAscent; descent = metrics.actualBoundingBoxDescent; cachedKey = key;
        // Quantize the visible edges before choosing a baseline; preserve the
        // lower edge of descenders instead of rounding the total height.
        const scale = window.devicePixelRatio || 1;
        ascent = Math.round(ascent * scale - .25) / scale;
        descent = Math.round(descent * scale + .25) / scale;
      }
      if (!Number.isFinite(ascent) || !Number.isFinite(descent)) return;
      const bounds = outer.getBoundingClientRect();
      const target = bounds.y + bounds.height / 2 + (ascent - descent) / 2;
      // Keep baselines on whole CSS pixels so font hinting cannot add a fraction.
      const alignedBaseline = Math.round(target);
      const previous = parseFloat(text.style.getPropertyValue('--resource-text-offset')) || 0;
      const next = previous + alignedBaseline - anchor.getBoundingClientRect().y;
      if (Math.abs(next - previous) < 1 / 64) return;
      return () => text.style.setProperty('--resource-text-offset', `${next}px`);
    }
    measure()?.();
    measurements.add(measure);
    if (measurements.size === 1) {
      window.addEventListener('scroll', scheduleAlignment, true);
      window.addEventListener('resize', scheduleAlignment);
      document.fonts.addEventListener('loadingdone', fontsChanged);
      document.fonts.ready.then(fontsChanged);
    }
    const resize = new ResizeObserver(scheduleAlignment);
    resize.observe(outer);
    return () => {
      active = false; resize.disconnect(); measurements.delete(measure);
      if (!measurements.size) {
        window.removeEventListener('scroll', scheduleAlignment, true);
        window.removeEventListener('resize', scheduleAlignment);
        document.fonts.removeEventListener('loadingdone', fontsChanged);
        if (frame !== undefined) cancelAnimationFrame(frame);
        frame = undefined;
      }
    };
  });
  return <span ref={box} className={`resource-text ${className}`}><span ref={line} className="resource-text-line"><span ref={baseline} className="resource-text-baseline" aria-hidden="true"/>{children}</span></span>;
}

/** One row layout for production and design-preview resource names and counts. */
export function ResourceLabel({ marker, children, meta }: { marker: ReactNode; children: string; meta?: string }) {
  return <span className="resource-label">
    <span className="resource-label-marker" aria-hidden="true">{marker}</span>
    <ResourceText className="resource-label-name">{children}</ResourceText>
    {meta !== undefined && <ResourceText className="resource-label-meta">{meta}</ResourceText>}
  </span>;
}
