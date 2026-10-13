import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { PanelBottom, PanelLeft, PanelRight, RotateCcw, X } from 'lucide-react';
import { Button } from './controls';

type Pane = 'explorer' | 'inspector' | 'activity';
type Layout = Record<Pane, number> & { left: boolean; right: boolean; bottom: boolean };
const defaults: Layout = { explorer: 224, inspector: 264, activity: 160, left: true, right: true, bottom: true };
const bounds = { explorer: [180, 360], inspector: [220, 400], activity: [100, 360] } as const;
export type WorkbenchLabels = {
  title: string; explorer: string; inspector: string; activity: string;
  toggleExplorer: string; toggleInspector: string; toggleActivity: string;
  reset: string; close: string; resize: (name: string) => string;
};
function initialLayout(key: string): Layout {
  try {
    const value = JSON.parse(localStorage.getItem(key) ?? '{}');
    return Object.fromEntries(Object.entries(defaults).map(([name, fallback]) => [name,
      typeof fallback === 'boolean' ? typeof value[name] === 'boolean' ? value[name] : fallback
        : Number.isFinite(value[name]) ? Math.max(bounds[name as Pane][0], Math.min(bounds[name as Pane][1], value[name])) : fallback,
    ])) as Layout;
  } catch { return defaults; }
}

/** Shared workspace furniture. Product adapters supply real resources and documents. */
export function WorkbenchLayout({ storageKey, routeKey, labels, toolbar, explorer, inspector, tabs, activity, status, children, className = '' }: {
  storageKey: string; routeKey: string; labels: WorkbenchLabels; toolbar: ReactNode;
  explorer: ReactNode; inspector: ReactNode; tabs: ReactNode; activity: ReactNode | ((close: () => void) => ReactNode);
  status?: ReactNode; children: ReactNode; className?: string;
}) {
  const [layout, setLayout] = useState(() => initialLayout(storageKey));
  const [width, setWidth] = useState(0);
  const [overlay, setOverlay] = useState<'explorer' | 'inspector' | null>(null);
  const [previousRoute, setPreviousRoute] = useState(routeKey);
  const root = useRef<HTMLElement>(null), id = useId();
  const drag = useRef<{ pane: Pane; start: number; size: number } | null>(null);
  if (previousRoute !== routeKey) { setPreviousRoute(routeKey); setOverlay(null); }
  if (overlay && width > (overlay === 'explorer' ? 720 : 1000)) setOverlay(null);
  const showLeft = width > 720 ? layout.left : overlay === 'explorer';
  const showRight = width > 1000 ? layout.right : overlay === 'inspector';
  useLayoutEffect(() => {
    const element = root.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => { try { localStorage.setItem(storageKey, JSON.stringify(layout)); } catch { /* Session layout still works. */ } }, [storageKey, layout]);
  useEffect(() => { if (overlay) root.current?.querySelector<HTMLElement>(`.workbench-${overlay} .workbench-pane-title`)?.focus({ preventScroll: true }); }, [overlay]);
  function size(pane: Pane, value: number) {
    setLayout(previous => ({ ...previous, [pane]: Math.max(bounds[pane][0], Math.min(bounds[pane][1], value)) }));
  }
  function separator(pane: Pane) {
    const horizontal = pane === 'activity';
    return <div className={`workbench-splitter ${horizontal ? 'horizontal' : ''}`} role="separator" tabIndex={0}
      aria-label={labels.resize(labels[pane])} aria-controls={`${id}-${pane}`}
      aria-orientation={horizontal ? 'horizontal' : 'vertical'} aria-valuemin={bounds[pane][0]} aria-valuemax={bounds[pane][1]} aria-valuenow={layout[pane]}
      onDoubleClick={() => size(pane, defaults[pane])}
      onKeyDown={event => {
        if (event.key === 'Home' || event.key === 'End') { event.preventDefault(); size(pane, bounds[pane][event.key === 'Home' ? 0 : 1]); }
        if ((horizontal ? ['ArrowUp', 'ArrowDown'] : ['ArrowLeft', 'ArrowRight']).includes(event.key)) {
          event.preventDefault(); const increase = pane === 'explorer' ? event.key === 'ArrowRight' : horizontal ? event.key === 'ArrowUp' : event.key === 'ArrowLeft';
          size(pane, layout[pane] + (increase ? 16 : -16));
        }
      }}
      onPointerDown={event => { if (event.button !== 0) return; event.preventDefault(); drag.current = { pane, start: horizontal ? event.clientY : event.clientX, size: layout[pane] }; event.currentTarget.setPointerCapture(event.pointerId); }}
      onPointerMove={event => { const state = drag.current; if (state?.pane === pane) size(pane, state.size + ((horizontal ? event.clientY : event.clientX) - state.start) * (pane === 'explorer' ? 1 : -1)); }}
      onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }} onLostPointerCapture={() => { drag.current = null; }}/>
  }
  function dismiss() {
    const index = overlay === 'explorer' ? 0 : 2;
    setOverlay(null);
    root.current?.querySelectorAll<HTMLButtonElement>('.workbench-panel-controls button')[index]?.focus({ preventScroll: true });
  }
  return <section ref={root} className={`workbench ${className}`} aria-label={labels.title} data-overlay={overlay ?? undefined}
    style={{ '--explorer-size': `${layout.explorer}px`, '--inspector-size': `${layout.inspector}px`, '--activity-size': `${layout.activity}px` } as CSSProperties}
    onKeyDown={event => { if (overlay && event.key === 'Escape' && !event.defaultPrevented) { event.preventDefault(); dismiss(); } }}>
    <div className="workbench-toolbar"><div className="workbench-toolbar-context">{toolbar}</div><div className="workbench-panel-controls">
      {([[PanelLeft, 'left', 'toggleExplorer', 'explorer'], [PanelBottom, 'bottom', 'toggleActivity', 'activity'], [PanelRight, 'right', 'toggleInspector', 'inspector']] as const).map(([Icon, key, label, pane]) =>
        <Button key={key} small variant="ghost" className="button-icon" title={labels[label]} aria-label={labels[label]} aria-controls={`${id}-${pane}`}
          aria-pressed={key === 'left' ? showLeft : key === 'right' ? showRight : layout.bottom}
          onClick={() => {
            if (key === 'left' && width <= 720 || key === 'right' && width <= 1000) {
              const next = key === 'left' ? 'explorer' : 'inspector'; setOverlay(previous => previous === next ? null : next);
            } else setLayout(previous => ({ ...previous, [key]: !previous[key] }));
          }}><Icon size={16}/></Button>)}
      <Button small variant="ghost" className="button-icon" title={labels.reset} aria-label={labels.reset} onClick={() => { setLayout({ ...defaults }); setOverlay(null); }}><RotateCcw size={14}/></Button>
    </div></div>
    <div className="workbench-body">
      {overlay && <button type="button" className="workbench-backdrop" aria-label={labels.close} onClick={dismiss}/>}
      {showLeft && <><aside id={`${id}-explorer`} className="workbench-explorer" aria-label={labels.explorer} inert={overlay === 'inspector'}>
        <header className="workbench-pane-title" tabIndex={-1}>{labels.explorer}</header>{explorer}
      </aside>{separator('explorer')}</>}
      <div className="workbench-center" inert={!!overlay}>
        {tabs}<div className="workbench-documents">{children}</div>
        {layout.bottom && <>{separator('activity')}<section id={`${id}-activity`} className="workbench-bottom" aria-label={labels.activity}>{typeof activity === 'function' ? activity(() => setLayout(previous => ({ ...previous, bottom: false }))) : activity}</section></>}
      </div>
      {showRight && <>{separator('inspector')}<aside id={`${id}-inspector`} className="workbench-inspector" aria-label={labels.inspector} inert={overlay === 'explorer'}>
        <header className="workbench-pane-title" tabIndex={-1}>{labels.inspector}<Button small variant="ghost" className="button-icon" aria-label={`${labels.close} ${labels.inspector}`} onClick={()=>{if(overlay)dismiss();else setLayout(previous=>({...previous,right:false}));}}><X size={14}/></Button></header>{inspector}
      </aside></>}
    </div>
    {status && <footer className="workbench-status">{status}</footer>}
  </section>;
}
