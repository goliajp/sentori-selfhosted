import { useEffect, useId, useRef, type ReactNode } from 'react';
import { Check, CircleAlert, LockKeyhole, Minus } from 'lucide-react';
import { Spinner } from './Spinner';
import { Button } from './controls';
export type FlowStepStatus = 'pending' | 'current' | 'complete' | 'busy' | 'error' | 'blocked' | 'skipped';
export type FlowAction = { label: string; onClick: () => void; disabled?: boolean; busy?: boolean };
/** The parent owns one size and one inset for the entire action row. */
export function FlowActions({ back, next, size = 'large', summary, placement = 'page' }: { back?: FlowAction; next: FlowAction; size?: 'regular' | 'large'; summary?: ReactNode; placement?: 'page' | 'dialog' }) {
  return <div className="flow-actions" data-size={size} data-placement={placement}>
    {back && <Button size={size} disabled={back.disabled} busy={back.busy} onClick={back.onClick}>{back.label}</Button>}
    {summary && <div className="flow-actions-summary">{summary}</div>}
    <Button size={size} variant="primary" disabled={next.disabled} busy={next.busy} onClick={next.onClick}>{next.label}</Button>
  </div>;
}
export type FlowStep = { id: string; label: string; description?: string; status?: FlowStepStatus; optional?: boolean; disabled?: boolean; panelId?: string };
export function StepMark({ status, number }: { status: FlowStepStatus; number: number }) {
  const Icon = status === 'complete' ? Check : status === 'error' ? CircleAlert : status === 'busy' ? Spinner : status === 'blocked' ? LockKeyhole : status === 'skipped' ? Minus : null;
  return <span className={`flow-step-mark${status === 'busy' ? ' is-busy' : ''}`} aria-hidden="true">{Icon ? <Icon size={16} /> : <span className="control-label-text">{number}</span>}</span>;
}
/** Completed/skipped states are explicit; previous positions are never assumed complete. */
export function FlowSteps({ label, steps, current, canGo, onGo, orientation = 'horizontal', statusLabels, optionalLabel }: {
  label: string; steps: readonly FlowStep[]; current: string; canGo: (step: string) => boolean; onGo: (step: string) => void; orientation?: 'horizontal' | 'vertical'; statusLabels: Record<FlowStepStatus, string>; optionalLabel: string;
}) {
  const id = useId();
  return <nav aria-label={label} className={`flow-steps flow-steps-${orientation}`}><ol>{steps.map((step, i) => {
    const status = step.status ?? (current === step.id ? 'current' : 'pending');
    const disabled = !!step.disabled || (current !== step.id && !canGo(step.id));
    return <li key={step.id} data-status={status}>
      <button type="button" aria-current={current === step.id ? 'step' : undefined} aria-controls={step.panelId} aria-describedby={`${id}-${i}-description`} disabled={disabled} onClick={() => onGo(step.id)}>
        <StepMark status={status} number={i + 1} />
        <span className="flow-step-copy"><span className="flow-step-title"><span className="control-label-text">{step.label}</span>{step.optional && <span className="flow-step-optional control-label-text">{optionalLabel}</span>}</span>
          <span className="flow-step-description" id={`${id}-${i}-description`}>{step.description}</span>
          <span className="flow-step-status">{statusLabels[status]}</span>
        </span>
      </button>
    </li>;
  })}</ol></nav>;
}
/** One fixed body track and one shared footer: step changes cannot move the action rail. */
export function FlowFrame({ title, description, feedback, children, asideTitle, asideDescription, asideBanner, aside, actions, stepKey, state, panelId, navigation }: {
  title: string; description?: string; feedback?: ReactNode; children: ReactNode; asideTitle?: string; asideDescription?: string; asideBanner?: ReactNode; aside?: ReactNode; actions: ReactNode; stepKey: string; state?: string; panelId?: string; navigation?: ReactNode;
}) {
  const id = useId(), titleId = `${id}-title`, asideId = `${id}-aside`;
  const content = useRef<HTMLDivElement>(null), heading = useRef<HTMLHeadingElement>(null), lastStep = useRef(stepKey);
  useEffect(() => {
    if (lastStep.current !== stepKey) {
      content.current?.scrollTo({ top: 0 });
      heading.current?.focus({ preventScroll: true });
      lastStep.current = stepKey;
    }
  }, [stepKey]);
  return <section className={`flow-frame${navigation ? ' flow-frame-with-navigation' : ''}${!aside && !navigation ? ' flow-frame-solo' : ''}${aside && navigation ? ' flow-frame-three-pane' : ''}`} data-flow-state={state} aria-labelledby={titleId}>
    <div className="flow-frame-body">
      {navigation && <aside className="flow-frame-navigation">{navigation}</aside>}
      <section className="flow-pane flow-main-pane" id={panelId} aria-labelledby={titleId}>
        <header className="flow-pane-head"><h2 ref={heading} tabIndex={-1} id={titleId}>{title}</h2><div className="flow-pane-description" aria-live="polite">{feedback || <p>{description}</p>}</div></header>
        <div className="flow-pane-content" ref={content} role="region" aria-labelledby={titleId} tabIndex={0}>{children}</div>
      </section>
      {aside && <aside className="flow-pane flow-status-pane" aria-labelledby={asideId}><header className="flow-pane-head"><h2 id={asideId}>{asideTitle}</h2><div className="flow-pane-description"><p>{asideDescription}</p></div></header><div className={`flow-pane-content${asideBanner ? " flow-status-content" : ""}`} role="region" aria-labelledby={asideId} tabIndex={0}>{asideBanner && <div className="flow-status-banner">{asideBanner}</div>}{asideBanner ? <div className="flow-status-details">{aside}</div> : aside}</div></aside>}
    </div>
    <footer className="flow-frame-footer">{actions}</footer>
  </section>;
}
