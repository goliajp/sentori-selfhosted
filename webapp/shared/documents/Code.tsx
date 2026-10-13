import { Action } from "../design/controls";
import { useEffect, useMemo, useRef, useState } from "react";
import { Check, TriangleAlert, Copy, ChevronsDownUp, ChevronsUpDown, ChevronRight, ChevronDown } from "lucide-react";
import { Button } from "../design/controls";
import { highlightCodeLines } from "./code-highlight";
import { annotateCodeHTML, type CodeReference } from "./code-references";
import { jsonFolds, visibleJsonLines } from "./json-fold";
type Translator = (key:string, params?:Record<string,string|number>) => string;
function CodeCopyButton({ code, t }: { code: string; t: Translator }) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  const sequence = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(
    () => () => {
      sequence.current += 1;
      clearTimeout(timer.current);
    },
    [],
  );
  return (
    <Button
      small
      variant="ghost"
      className="code-copy-button"
      data-copy-state={state}
      title={state === "failed" ? t("common.copyFailed") : undefined}
      onClick={async () => {
        const request = ++sequence.current;
        clearTimeout(timer.current);
        setState("idle");
        let next: "copied" | "failed";
        try {
          await navigator.clipboard.writeText(code);
          next = "copied";
        } catch {
          next = "failed";
        }
        // A new copy or a different source invalidates late clipboard results.
        if (sequence.current !== request) return;
        setState(next);
        timer.current = setTimeout(() => setState("idle"), 2000);
      }}
    >
      <span className="code-copy-label" aria-live="polite" aria-atomic="true">
        {(["idle", "copied", "failed"] as const).map(value => {
          const Icon = value === "copied" ? Check : value === "failed" ? TriangleAlert : Copy;
          return <span key={value} aria-hidden={value !== state}>
            <Icon size={14} aria-hidden="true" />
            <span>{t(value === "idle" ? "common.copy" : value === "copied" ? "common.copied" : "code.copyFailed")}</span>
          </span>;
        })}
      </span>
    </Button>
  );
}
export function Code({ t, code, language = "tsx", fileName, lineNumbers = true, startLine = 1, highlightedLines = {}, references = [], onReference, onOpenFile }: {
  t: Translator; code: string; language?: string; fileName?: string; lineNumbers?: boolean; startLine?: number;
  highlightedLines?: Record<number, 'error'|'warning'|'success'>;
  references?: CodeReference[]; onReference?: (reference:CodeReference,navigate:boolean)=>void; onOpenFile?:()=>void;
}) {
  const lines = useMemo(() => highlightCodeLines(code, language), [code,language]);
  const folds = useMemo(() => language.toLowerCase() === 'json' ? jsonFolds(code) : [], [code,language]);
  const [foldState,setFoldState] = useState<{code:string;collapsed:Set<number>}>(()=>({code,collapsed:new Set()}));
  const collapsed = foldState.code === code ? foldState.collapsed : new Set<number>();
  const visible = visibleJsonLines(lines.length,folds,collapsed);
  const starts = new Map(folds.map(fold=>[fold.start,fold]));
  const firstLine = Number.isFinite(startLine) ? Math.max(1,Math.trunc(startLine)) : 1;
  const toggle = (line:number) => setFoldState(previous => { const next = new Set(previous.code === code ? previous.collapsed : []); if(next.has(line))next.delete(line);else next.add(line);return {code,collapsed:next}; });
  return <div className="code-block" data-language={language} data-foldable={folds.length>0 || undefined}>
    <div className="code-head"><div className="code-caption">{onOpenFile?<Action type="button" className="code-file-link" title={t('inspect.openSource')} onClick={onOpenFile}>{fileName ?? language}</Action>:<strong title={fileName ?? language}>{fileName ?? language}</strong>}{fileName && <span>{language}</span>}</div><div className="code-tools">{folds.length>0 && <><Button small variant="ghost" className="button-icon" aria-label={t('code.collapseAll')} title={t('code.collapseAll')} onClick={() => setFoldState({code,collapsed:new Set(folds.map(f=>f.start))})}><ChevronsDownUp size={14}/></Button><Button small variant="ghost" className="button-icon" aria-label={t('code.expandAll')} title={t('code.expandAll')} onClick={() => setFoldState({code,collapsed:new Set()})}><ChevronsUpDown size={14}/></Button></>}<CodeCopyButton key={code} code={code} t={t}/></div></div>
    <div className="code-body">
      {lineNumbers && <div className="code-gutter" aria-hidden={folds.length ? undefined : true}>{visible.map(index => <span className="code-gutter-row" key={index} data-tone={highlightedLines[firstLine+index]}><span aria-hidden="true">{firstLine+index}</span>{starts.has(index) && <Action type="button" className="code-fold-button" aria-expanded={!collapsed.has(index)} aria-label={t(collapsed.has(index)?'code.expandLine':'code.collapseLine',{line:firstLine+index})} onClick={() => toggle(index)}>{collapsed.has(index)?<ChevronRight size={12}/>:<ChevronDown size={12}/>}</Action>}</span>)}</div>}
      <pre tabIndex={0} aria-label={t('code.read',{name:fileName ?? language})}><code>{visible.map(index => {const fold=starts.get(index),closed=fold&&collapsed.has(index),tone=highlightedLines[firstLine+index];return <span className="code-source-line" key={index} data-line={firstLine+index} data-tone={tone}>{tone && <span className="sr-only">{t(`code.line.${tone}`)}: </span>}<span onClick={event=>{const element=(event.target as HTMLElement).closest<HTMLElement>('[data-code-reference]');if(element){event.preventDefault();onReference?.(references[Number(element.dataset.codeReference)],event.altKey);}}} onKeyDown={event=>{if(event.altKey&&event.key==='Enter'){const element=(event.target as HTMLElement).closest<HTMLElement>('[data-code-reference]');if(element){event.preventDefault();onReference?.(references[Number(element.dataset.codeReference)],true);}}}} dangerouslySetInnerHTML={{__html:annotateCodeHTML(lines[index] || ' ',firstLine+index,references,t('inspect.altHint'))}}/>{closed && <Action className="code-fold-summary" onClick={() => toggle(index)} aria-expanded="false" aria-label={t('code.expandLine',{line:firstLine+index})}> … {fold.count} {t(fold.opening==='['?'code.items':'code.properties')} {fold.closing}{code.split('\n')[fold.end]?.trimEnd().endsWith(',')?',':''}</Action>}</span>})}</code></pre>
    </div>
  </div>;
}
