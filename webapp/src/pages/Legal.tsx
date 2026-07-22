// Public legal pages: terms, privacy, and the 特定商取引法 disclosure.
//
// These live in the SPA rather than the marketing site because
// `/legal/*` already routes here — the marketing build only owns `/`
// and `/pricing`, and pointing more paths at it would mean editing the
// Caddyfile on the box, which this project forbids for good reason.
// Until now `/legal/terms` answered 200 with the empty dashboard shell,
// so the marketing footer had two links that led to nothing.
//
// Content comes from `src/legal`, which the hard-coded-text check does
// not scan: prose inside a page component is a bug, prose in a legal
// document is the point.

import { useParams, Link, Navigate } from 'react-router-dom';

import { useI18n } from '../i18n';
import { LEGAL_DOCS, LEGAL_NAV, type Locale as DocLocale } from '../legal/documents';

export default function Legal() {
  const { slug = '' } = useParams();
  const { locale } = useI18n();
  const doc = LEGAL_DOCS[slug];

  if (!doc) return <Navigate to="/legal/terms" replace />;

  // Japanese unless an English version exists and the reader is not
  // reading Japanese. `tokushoho` has no English version on purpose,
  // so it falls back to the Japanese original rather than to nothing.
  const want: DocLocale = locale === 'ja' ? 'ja' : 'en';
  const body = doc[want] ?? doc.ja ?? doc.en;
  if (!body) return <Navigate to="/legal/terms" replace />;

  const navLang: DocLocale = locale === 'ja' ? 'ja' : 'en';

  return (
    <div className="min-h-screen bg-canvas text-fg">
      <header className="border-b border-border">
        <nav className="mx-auto flex max-w-3xl items-center justify-between px-6 py-4 text-sm">
          <a href="/" className="font-semibold">
            Sentori
          </a>
          <div className="flex gap-4 text-fg-muted">
            {LEGAL_NAV.map(n => (
              <Link
                key={n.slug}
                to={`/legal/${n.slug}`}
                className={
                  n.slug === slug ? 'text-fg' : 'transition-colors hover:text-fg'
                }
              >
                {n[navLang]}
              </Link>
            ))}
          </div>
        </nav>
      </header>

      <main className="mx-auto max-w-3xl px-6 pt-12 pb-24">
        <h1 className="text-2xl font-semibold tracking-tight">{body.title}</h1>
        <p className="mt-2 font-mono text-xs text-fg-subtle">{body.updated}</p>

        {body.intro && (
          <p className="mt-6 text-sm leading-relaxed text-fg-muted">
            {body.intro}
          </p>
        )}

        {body.sections.map(s => (
          <section key={s.heading} className="mt-10">
            <h2 className="text-[15px] font-semibold">{s.heading}</h2>

            {s.body?.map(p => (
              <p key={p} className="mt-3 text-sm leading-relaxed text-fg-muted">
                {p}
              </p>
            ))}

            {s.rows && (
              // Two columns on anything wider than a phone, stacked
              // below it. A disclosure table that scrolls sideways on
              // the device most people read it on is not a disclosure.
              <dl className="mt-3 divide-y divide-border border-y border-border">
                {s.rows.map(([k, v]) => (
                  <div key={k} className="grid gap-1 py-3 sm:grid-cols-[13rem_1fr] sm:gap-4">
                    <dt className="text-xs text-fg-subtle sm:text-sm">{k}</dt>
                    <dd className="text-sm leading-relaxed text-fg-muted">{v}</dd>
                  </div>
                ))}
              </dl>
            )}
          </section>
        ))}
      </main>
    </div>
  );
}
