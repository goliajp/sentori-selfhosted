// The dashboard's layout and data primitives.
//
// Colour, elevation, radius and both modes come from @goliapkg/gds —
// the same system golia.jp runs on. Nothing here names a palette
// value; everything reaches for a role (`accent`, `ok`, `danger`,
// `fg-muted`) so one definition serves light and dark alike.
//
// Two invariants this file exists to hold:
//   · a control's height is written once (`CONTROL_H`), never
//     inferred from padding;
//   · a card's contents sit at one inset (`px-5`), header and body
//     and table cells alike, so the left edge is a single line.

import { isValidElement, type ReactNode } from 'react';
import { Link } from 'react-router-dom';

import { useT } from '../i18n';

// ── Card ───────────────────────────────────────────────────

export function Card({
  children,
  className = '',
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={`rounded border border-border bg-surface ${className}`}
    >
      {children}
    </div>
  );
}

export function CardHeader({
  title,
  subtitle,
  action,
}: {
  title: string;
  subtitle?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex items-start justify-between border-b border-border px-5 py-4">
      <div>
        <h3 className="text-[15px] font-semibold tracking-tight text-fg">{title}</h3>
        {subtitle && (
          <p className="mt-0.5 text-xs text-fg-subtle">{subtitle}</p>
        )}
      </div>
      {action}
    </div>
  );
}

// ── Controls ───────────────────────────────────────────────

/**
 * The two control heights in the product. Everything that can sit in
 * a row with a button — buttons, inputs, selects — uses one of these
 * so the row has a single baseline. Before this the app had ten
 * different padding pairs standing in for a height, and no two
 * adjacent controls agreed.
 */
export const CONTROL_H = { sm: 'h-7', md: 'h-8' } as const;

/** Text input at the shared control height. */
export function Input(props: React.InputHTMLAttributes<HTMLInputElement>) {
  const { className = '', ...rest } = props;
  return (
    <input
      {...rest}
      className={`${CONTROL_H.md} w-full rounded border border-border bg-surface px-2.5 text-sm text-fg placeholder:text-fg-subtle focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-1 focus-visible:outline-accent ${className}`}
    />
  );
}

/** Select at the shared control height. */
export function Select(props: React.SelectHTMLAttributes<HTMLSelectElement>) {
  const { className = '', children, ...rest } = props;
  return (
    <select
      {...rest}
      className={`${CONTROL_H.md} rounded border border-border bg-surface px-2 text-sm text-fg focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-1 focus-visible:outline-accent ${className}`}
    >
      {children}
    </select>
  );
}

// ── Button ─────────────────────────────────────────────────

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md';

/**
 * The single source of a button's appearance, shared by `<Button>`
 * and `<LinkButton>`.
 *
 * Height is fixed, never inferred from padding: derive it from
 * padding plus line-height and a button carrying an icon or a count
 * ends up a pixel taller than the plain one beside it, and a toolbar
 * of six never lines up. Padding sets width only.
 */
export function buttonClass(
  variant: ButtonVariant = 'secondary',
  size: ButtonSize = 'md',
  icon = false,
): string {
  const base =
    'inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-md font-medium transition focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50';
  // An icon-only button is square. Left to padding it would be
  // narrower than its lettered neighbours and by a different amount
  // for every glyph — `⊘` and `↺` do not share a width.
  const sizes = icon
    ? { sm: `${CONTROL_H.sm} w-7 text-sm`, md: `${CONTROL_H.md} w-8 text-sm` }
    : { sm: `${CONTROL_H.sm} px-2.5 text-sm`, md: `${CONTROL_H.md} px-3 text-sm` };
  const variants = {
    primary: 'bg-accent text-accent-fg hover:opacity-90',
    secondary: 'border border-border-strong bg-surface text-fg hover:bg-raised',
    ghost: 'text-fg-muted hover:bg-raised hover:text-fg',
    danger:
      'border border-danger/40 bg-danger/10 text-danger hover:bg-danger/20',
  };
  return `${base} ${sizes[size]} ${variants[variant]}`;
}

export function Button({
  children,
  variant = 'secondary',
  size = 'md',
  onClick,
  disabled,
  type = 'button',
  title,
  icon = false,
}: {
  children: ReactNode;
  variant?: ButtonVariant;
  size?: ButtonSize;
  onClick?: () => void;
  disabled?: boolean;
  type?: 'button' | 'submit';
  /** Also the accessible name — an icon-only button has no text to
   *  read out, so this is the only thing a screen reader can announce. */
  title?: string;
  /** Square, for a lone glyph. */
  icon?: boolean;
}) {
  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled}
      title={title}
      aria-label={title}
      className={buttonClass(variant, size, icon)}
    >
      {children}
    </button>
  );
}

/**
 * A link that is a button.
 *
 * `← All` was a hand-styled `<Link>` sitting in a row of `<Button>`s
 * and it stood 4px taller than every one of them, because the height
 * had been written twice and the two copies drifted. Anything that
 * looks like a button now derives its class from the same place.
 */
export function LinkButton({
  to,
  children,
  variant = 'secondary',
  size = 'md',
}: {
  to: string;
  children: ReactNode;
  variant?: ButtonVariant;
  size?: ButtonSize;
}) {
  return (
    <Link to={to} className={buttonClass(variant, size)}>
      {children}
    </Link>
  );
}

// ── Badge ──────────────────────────────────────────────────

export function Badge({
  children,
  tone = 'neutral',
}: {
  children: ReactNode;
  tone?: 'neutral' | 'ok' | 'warn' | 'danger' | 'info';
}) {
  // Tinted from the semantic colour itself rather than picked off the
  // raw palette: `bg-green-950` is a near-black green, which reads as
  // a badge only while the card behind it is dark. Deriving the fill
  // from the same token as the text means one definition serves both
  // modes, and a status colour can never drift from its own tint.
  const tones = {
    neutral: 'bg-raised text-fg-muted',
    ok: 'bg-ok/12 text-ok',
    warn: 'bg-warn/12 text-warn',
    danger: 'bg-danger/12 text-danger',
    info: 'bg-accent/12 text-accent',
  };
  // `whitespace-nowrap` is not cosmetic here. CJK has no word
  // boundaries, so a wrapped badge breaks between any two characters:
  // 未対応 became three stacked glyphs and pushed the table row to
  // three lines. `tracking-wide` is scoped to the Latin case for the
  // same reason — letter-spacing between kanji reads as broken, not
  // deliberate.
  return (
    <span
      className={`inline-flex items-center whitespace-nowrap rounded px-1.5 py-0.5 text-xs font-medium uppercase ${tones[tone]}`}
    >
      {children}
    </span>
  );
}

// ── DataTable ──────────────────────────────────────────────

/**
 * A cell's value, rendered as what it is.
 *
 * This used to be `String(value ?? '—')`, which turns anything that is
 * not a primitive into the literal text `[object Object]` — including
 * a React element, which is exactly what seven pages put in their rows
 * instead of writing a `render` callback for every column. The whole
 * projects table read `[object Object]` five times across.
 *
 * `String()` is the wrong tool because it never fails: it converts
 * everything, so a type mismatch shows up as plausible-looking text
 * rather than as an error anyone would notice.
 */
function cell(row: unknown, key: PropertyKey): ReactNode {
  const v = (row as Record<PropertyKey, unknown>)[key];
  if (v === null || v === undefined || v === '') return '—';
  if (isValidElement(v)) return v;
  if (typeof v === 'object') return JSON.stringify(v);
  return String(v);
}

export function DataTable<T>({
  columns,
  rows,
  empty,
  rowKey,
}: {
  columns: { key: keyof T | string; label: string; render?: (row: T) => ReactNode; width?: string }[];
  rows: T[];
  empty?: string;
  rowKey?: (row: T) => string;
}) {
  // Resolved here rather than as a default parameter: a default is
  // evaluated where the signature is written, which has no hook and
  // so no locale, and every caller that omitted `empty` was getting
  // the English string regardless of language.
  const t = useT();
  if (rows.length === 0) {
    return (
      <div className="p-8 text-center text-sm text-fg-subtle">
        {empty ?? t('table.empty')}
      </div>
    );
  }
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead className="border-b border-border bg-bg/50">
          <tr>
            {columns.map((c) => (
              <th
                key={String(c.key)}
                className="whitespace-nowrap px-5 py-2.5 text-left text-xs font-medium uppercase tracking-wide text-fg-subtle"
                style={c.width ? { width: c.width } : undefined}
              >
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {rows.map((r, i) => (
            <tr key={rowKey ? rowKey(r) : String(i)} className="hover:bg-surface/50">
              {columns.map((c) => (
                <td key={String(c.key)} className="px-5 py-3 text-fg-muted">
                  {c.render ? c.render(r) : cell(r, c.key)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ── Page header ────────────────────────────────────────────

export function PageHeader({
  title,
  subtitle,
  action,
  actions,
}: {
  title: string;
  subtitle?: string;
  action?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="mb-6 flex items-start justify-between">
      <div>
        <h2 className="text-2xl font-semibold tracking-tight">{title}</h2>
        {subtitle && (
          <p className="mt-1 text-sm text-fg-subtle">{subtitle}</p>
        )}
      </div>
      {action ?? actions}
    </div>
  );
}

// ── Empty state ────────────────────────────────────────────

export function EmptyState({
  title,
  hint,
  action,
}: {
  title: string;
  hint?: string;
  action?: ReactNode;
}) {
  return (
    <div className="rounded border border-border bg-surface p-12 text-center">
      <p className="text-fg-muted">{title}</p>
      {hint && <p className="mt-2 text-sm text-fg-subtle">{hint}</p>}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}

// ── Error banner ───────────────────────────────────────────

export function ErrorBanner({ children }: { children: ReactNode }) {
  return (
    <div className="rounded border border-danger/40 bg-danger/50 p-3 text-sm text-danger">
      {children}
    </div>
  );
}

// ── Section ────────────────────────────────────────────────

/**
 * A card's content, at the same inset as its header.
 *
 * This used to be `<Section>` doing double duty — page-level
 * grouping *and* card body — which is why card content sat flush
 * against the border with a stray 32px gap under it: the grouping
 * component's `mb-8` was landing inside the card. One element, one
 * job.
 */
export function CardBody({
  children,
  className = '',
}: {
  children: ReactNode;
  className?: string;
}) {
  return <div className={`px-5 py-4 ${className}`}>{children}</div>;
}

/** A titled group of cards on a page. Not a card's interior — that
 *  is `CardBody`. */
export function Section({
  title,
  children,
  action,
}: {
  title?: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <section className="mb-8">
      {(title || action) && (
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-xs font-semibold uppercase tracking-[0.12em] text-fg-subtle">
            {title}
          </h3>
          {action}
        </div>
      )}
      {children}
    </section>
  );
}

// ── Tabs ───────────────────────────────────────────────────

export function Tabs({
  value,
  onChange,
  options,
}: {
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
}) {
  return (
    <div className="flex gap-1 border-b border-border">
      {options.map((o) => (
        <button
          key={o.value}
          onClick={() => onChange(o.value)}
          className={`border-b-2 px-3 py-2 text-sm transition ${
            value === o.value
              ? 'border-accent text-fg'
              : 'border-transparent text-fg-muted hover:text-fg'
          }`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

// ── Format helpers ─────────────────────────────────────────

/**
 * "8 minutes ago", in the reader's language.
 *
 * This used to append the English suffix by hand — `${n}m ago` — so a
 * Japanese dashboard still said "15m ago" beside 最終発生. Every
 * platform ships the plural rules and the word order for this;
 * `Intl.RelativeTimeFormat` is the whole answer, and the `narrow`
 * style keeps the string short enough for a table cell.
 *
 * The locale comes from the document rather than a prop because this
 * is called from ten files, most of them deep inside table cell
 * renderers where threading a locale down would mean touching every
 * column definition to fix a suffix.
 */
export function formatRelative(
  iso: string | null | undefined,
  now: number = Date.now(),
): string {
  // A formatter handed no timestamp has nothing to format. Several
  // columns are genuinely nullable — `resolved_at` on an unresolved
  // issue, `next_attempt_at` on a send that will not be retried — and
  // `new Date(null).getTime()` is NaN. `Intl.RelativeTimeFormat`
  // *throws* on a non-finite number rather than printing one, so the
  // NaN that the old hand-rolled version rendered as a cosmetic
  // "NaNs ago" now takes the whole page down with it.
  const ms = iso == null ? NaN : new Date(iso).getTime();
  if (!Number.isFinite(ms)) return '—';
  const sec = Math.abs(now - ms) / 1000;
  const [value, unit]: [number, Intl.RelativeTimeFormatUnit] =
    sec < 60
      ? [Math.max(1, Math.round(sec)), 'second']
      : sec < 3600
        ? [Math.round(sec / 60), 'minute']
        : sec < 86_400
          ? [Math.round(sec / 3600), 'hour']
          : sec < 86_400 * 30
            ? [Math.round(sec / 86_400), 'day']
            : sec < 86_400 * 365
              ? [Math.round(sec / 86_400 / 30), 'month']
              : [Math.round(sec / 86_400 / 365), 'year'];
  const locale =
    typeof document === 'undefined' ? 'en' : document.documentElement.lang || 'en';
  return new Intl.RelativeTimeFormat(locale, { numeric: 'always', style: 'narrow' })
    .format(-value, unit);
}

export function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

export function clsx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(' ');
}
