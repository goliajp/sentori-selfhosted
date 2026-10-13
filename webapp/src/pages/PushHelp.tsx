import { Action } from '../../shared/design/controls';
import { Fragment, useState } from 'react';
import { Link } from 'react-router-dom';
import { Check, Copy, ExternalLink } from 'lucide-react';
import { Surface as Panel } from '../../shared/design/Surface';
import { clsx } from '../components/ui';
import { useT } from '../i18n';
import type { MessageKey } from '../i18n/en';
import { PROVIDER_SPECS, type Provider } from '../lib/push-credentials';
import { highlightBlock } from '../lib/highlight';
import {
  SEND_PATH,
  SNIPPET_LANGS,
  type SnippetLang,
  countSnippet,
  pollSnippet,
  snippet,
} from '../lib/push-snippets';
const HLJS_LANG: Record<SnippetLang, string> = {
  cpp: 'cpp',
  csharp: 'csharp',
  go: 'go',
  java: 'java',
  node: 'typescript',
  python: 'python',
  rust: 'rust',
};

/// Where to go to fix it, when there is somewhere to go.
export function IntegrateSection({ project }: { project: string }) {
  const t = useT();
  const [lang, setLang] = useState<SnippetLang>('go');
  const base = window.location.origin;

  return (
    <div className="flex flex-col gap-3">
      <Panel title={t('push.integrateEndpoint')}>
        {/* Two rows of one spec, not a paragraph about a spec. The
            method and the scope are both constraints on the same call,
            so they read as the same shape: a mono label in the gutter,
            the value beside it. */}
        <div className="grid grid-cols-[auto_1fr] items-baseline gap-x-3 gap-y-2 p-3.5">
          <span className="font-mono text-[11px] uppercase tracking-wide text-fg-subtle">
            POST
          </span>
          <div className="min-w-0">
            <Copyable text={`${base}${SEND_PATH}`} />
          </div>

          <span className="font-mono text-[11px] uppercase tracking-wide text-fg-subtle">
            SCOPE
          </span>
          <div className="flex min-w-0 flex-wrap items-baseline gap-x-2.5 gap-y-1 text-xs">
            <code className="font-mono text-fg">api</code>
            {/* The likely mistake is reaching for the token the app
                already ships with. It authenticates and then 403s on
                this route, which reads as an outage. */}
            <span className="text-fg-subtle">
              {t('push.integrateScopeNote')}
            </span>
            {/* Settings ▸ Tokens, not Push ▸ Credentials. Those are
                the vendor's keys; this is the token that authorises
                the call, and sending someone to the wrong one is
                worse than sending them nowhere. */}
            <Link
              to={`/settings?tab=tokens&project=${project}`}
              className="text-accent hover:underline"
            >
              {t('push.integrateMint')} →
            </Link>
          </div>
        </div>
      </Panel>

      <Panel
        title={t('push.integrateSend')}
        action={
          <div className="flex flex-wrap gap-1">
            {SNIPPET_LANGS.map((l) => (
              <Action
                key={l.id}
                type="button"
                onClick={() => setLang(l.id)}
                className={clsx(
                  'rounded px-1.5 py-0.5 font-mono text-[11px] transition-colors',
                  lang === l.id
                    ? 'bg-raised text-fg'
                    : 'text-fg-subtle hover:text-fg-muted',
                )}
              >
                {l.label}
              </Action>
            ))}
          </div>
        }
      >
        <CodeBlock text={snippet(lang, base)} language={HLJS_LANG[lang]} />
      </Panel>

      <Panel title={t('push.integratePoll')}>
        <p className="px-3.5 pt-3 text-xs text-fg-muted">
          {t('push.integratePollWhy')}
        </p>
        <CodeBlock text={pollSnippet(base)} language="bash" />
      </Panel>

      <Panel title={t('push.integrateCount')}>
        <p className="px-3.5 pt-3 text-xs text-fg-muted">
          {t('push.integrateCountWhy')}
        </p>
        <CodeBlock text={countSnippet(base)} language="bash" />
      </Panel>
    </div>
  );
}

/// A block of code, lit by its own grammar, and a way to take it.
function CodeBlock({ text, language }: { language: string; text: string }) {
  const t = useT();
  const [result,setResult] = useState<{text:string;ok:boolean}|null>(null);
  const copied = result?.text === text && result.ok;
  const failed = result?.text === text && !result.ok;
  return (
    <div className="relative min-w-0 max-w-full">
      {/* The grammar's own escaped output, over text this module
          holds as a constant. `devtools/check-highlight.mjs` strips
          the tags back off and compares. */}
      <pre
        className="block max-w-full overflow-x-auto px-3.5 py-3 font-mono text-xs leading-relaxed text-fg"
        dangerouslySetInnerHTML={{ __html: highlightBlock(text, language) }}
      />
      <Action
        type="button"
        title={t(failed?'setup.copyFailed':copied?'identity.copied':'identity.copyHint')}
        aria-label={t(failed?'setup.copyFailed':copied?'identity.copied':'identity.copyHint')}
        className="absolute right-2 top-2 rounded border border-border bg-surface p-1 text-fg-subtle hover:text-fg"
        onClick={async () => {
          try {await navigator.clipboard.writeText(text);setResult({text,ok:true});}
          catch {setResult({text,ok:false});}
        }}
      >
        {copied ? (
          <Check className="size-3.5 text-ok" />
        ) : (
          <Copy className="size-3.5" />
        )}
      </Action>
      {failed && <p role="status" className="text-xs text-danger p-3">{t('setup.copyFailed')}</p>}
    </div>
  );
}

/// One value, and a way to take it.
function Copyable({ text }: { text: string }) {
  const t = useT();
  const [result,setResult] = useState<{text:string;ok:boolean}|null>(null);
  const copied = result?.text === text && result.ok;
  const failed = result?.text === text && !result.ok;
  return (
    <Action
      type="button"
      title={t(failed?'setup.copyFailed':copied?'identity.copied':'identity.copyHint')}
        aria-label={t(failed?'setup.copyFailed':copied?'identity.copied':'identity.copyHint')}
      className="group inline-flex max-w-full min-w-0 items-center gap-1.5 text-left font-mono text-sm text-fg"
      onClick={async () => {
        try {await navigator.clipboard.writeText(text);setResult({text,ok:true});}
        catch {setResult({text,ok:false});}
      }}
    >
      <span className="min-w-0 break-all">{text}{failed && <span role="status" className="block text-xs text-danger">{t('setup.copyFailed')}</span>}</span>
      {copied ? (
        <Check className="size-3 shrink-0 text-ok" />
      ) : (
        <Copy className="size-3 shrink-0 text-fg-subtle opacity-0 transition-opacity group-hover:opacity-100" />
      )}
    </Action>
  );
}

// ── devices ─────────────────────────────────────────────────────────

/// Getting a credential in, for someone who has never done it.
///
/// The old form assumed you arrived holding the right file and
/// knowing what a Team ID was. Both assumptions fail in the same
/// direction: you paste something plausible, it saves, and you learn
/// that night. So this section is four things in order — where to get
/// it, how to recognise it, what we made of what you pasted, and what
/// the vendor said about it.
export function ProviderGuide({ provider }: { provider: Provider }) {
  const t = useT();
  const spec = PROVIDER_SPECS.find((s) => s.provider === provider);
  if (!spec) return null;

  return (
    <Panel title={t('push.specHowTo')}>
      <div className="flex flex-col gap-3 p-3.5">
        <ol className="flex flex-col gap-1.5">
          {spec.steps.map((step, i) => (
            <li key={step} className="flex gap-2.5 text-xs leading-relaxed">
              <span className="w-3 shrink-0 pt-px text-right font-mono tabular-nums text-fg-subtle">
                {i + 1}
              </span>
              <span className="text-fg-muted">{t(step as MessageKey)}</span>
            </li>
          ))}
        </ol>

        {spec.href !== undefined && (
          <a
            href={spec.href}
            target="_blank"
            rel="noreferrer noopener"
            className="flex items-center gap-1.5 min-w-0 max-w-full break-all self-start font-mono text-xs text-accent hover:underline"
          >
            <ExternalLink aria-hidden className="size-3" />
            {spec.href}
          </a>
        )}

        {/* What the download looks like, so it can be recognised
            before it is pasted. Not translated: a localised file name
            is a file name nobody finds. */}
        <div className="grid grid-cols-[auto_1fr] items-baseline gap-x-3 gap-y-1 border-t border-border/60 pt-3">
          {spec.spec.map((row) => (
            <Fragment key={row.label + row.value}>
              <span className="font-mono text-[11px] uppercase tracking-wide text-fg-subtle">
                {t(row.label as MessageKey)}
              </span>
              <code className="min-w-0 break-all font-mono text-xs text-fg">
                {row.value}
              </code>
            </Fragment>
          ))}
        </div>
      </div>
    </Panel>
  );
}

/// The form, with the file read for you.
///
/// Two of the four APNs fields are typed by hand off a web page,
/// which is where transposed characters come from. The Key ID is in
/// the file name; Firebase's project is in the file. Both are lifted
/// rather than asked for.
