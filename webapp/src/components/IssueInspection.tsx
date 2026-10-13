import { useContext } from 'react';
import { createPortal } from 'react-dom';
import { Link } from 'react-router-dom';
import { Button } from '../../shared/design/controls';
import { DocumentInspectionContext } from '../lib/document-context';
import type { EventDetail } from '../lib/api';
import { useI18n } from '../i18n';
import type { StackFrame } from './StackTrace';

/** Portals retain the owning document's route and event, including across tabs. */
export function IssueInspection({event,frame,frameIndex,stack,onReset,onRaw,loading,error}: {
  event: EventDetail|null; frame?: StackFrame; frameIndex?: number; stack: StackFrame[];
  onReset:()=>void; onRaw:()=>void; loading:boolean; error:string|null;
}) {
  const host=useContext(DocumentInspectionContext),{t,locale}=useI18n();
  if(!host)return null;
  const functionName=typeof frame?.function==='string'?frame.function:undefined;
  const file=typeof frame?.file==='string'?frame.file:undefined;
  const imageUuid=typeof frame?.imageUuid==='string'?frame.imageUuid:undefined;
  const hasSource=typeof frame?.contextLine==='string';
  const hasLocation=typeof frame?.line==='number'&&Number.isInteger(frame.line)&&frame.line>0;
  const sourceCount=stack.filter(item=>typeof item.contextLine==='string').length;
  const debugHref=event?`/releases?project=${encodeURIComponent(event.projectId)}&q=${encodeURIComponent(event.release)}`:'';
  const hex=(value:unknown)=>typeof value==='number'&&Number.isSafeInteger(value)?`0x${value.toString(16)}`:null;
  return createPortal(<div className="workbench-inspector-body" data-inspected-event={event?.id}>
    <span className="eyebrow">{t(frame?'inspect.frame':'inspect.event')}</span>
    {!event?<p role="status">{error||t(loading?'shell.loading':'detail.noEvents')}</p>:<>
      {frame?<>
        <h2>{t('inspect.frameNumber',{n:String((frameIndex??0)+1)})}</h2>
        <dl>
          <div><dt>{t('inspect.location')}</dt><dd className="inspection-code">{file||t('bench.unknown')}{hasLocation?`:${frame.line}`:''}</dd></div>
        </dl>
        <p className="inspection-status">{t(hasSource?'inspect.sourceAvailable':hasLocation?'stack.sourceMissing':imageUuid?'stack.nativeLocationMissing':'stack.locationMissing')}</p>
      </>:<>
        <h2>{new Date(event.occurredAt).toLocaleString(locale)}</h2>
        <p className="inspection-status">{t('stack.sourceCount',{n:String(sourceCount),total:String(stack.length)})}</p>
        <p>{t('inspect.selectFrame')}</p>
      </>}
      <h3>{t('inspect.eventContext')}</h3>
      <dl>
        <div><dt>{t('bench.release')}</dt><dd className="inspection-code">{event.release||t('bench.unknown')}</dd></div>
        <div><dt>{t('bench.environment')}</dt><dd>{event.environment||t('bench.unknown')}</dd></div>
        <div><dt>{t('issue.deviceClock')}</dt><dd>{new Date(event.occurredAt).toLocaleString(locale)}</dd></div>
      </dl>
      <div className="workbench-inspector-actions">
        {event.release&&<Link className="text-link" to={debugHref}>{t('stack.debugFiles')}</Link>}
        <Button small onClick={onRaw}>{t('detail.rawEvent')}</Button>
        {frame&&<Button small variant="ghost" onClick={onReset}>{t('inspect.backToEvent')}</Button>}
      </div>
      {functionName&&<details><summary>{t('inspect.function')}</summary><p className="inspection-code">{functionName}</p></details>}
      <details><summary>{t('inspect.identifiers')}</summary><dl>
        <div><dt>{t('detail.event')}</dt><dd className="inspection-code">{event.id}</dd></div>
        <div><dt>{t('detail.lastReceived')}</dt><dd>{new Date(event.receivedAt).toLocaleString(locale)}</dd></div>
        {imageUuid&&<div><dt>{t('stack.binaryUuid')}</dt><dd className="inspection-code">{imageUuid}</dd></div>}
        {hex(frame?.addr)&&<div><dt>{t('stack.address')}</dt><dd className="inspection-code">{hex(frame?.addr)}</dd></div>}
        {hex(frame?.imageBase)&&<div><dt>{t('inspect.imageBase')}</dt><dd className="inspection-code">{hex(frame?.imageBase)}</dd></div>}
      </dl></details>
    </>}
  </div>,host);
}
