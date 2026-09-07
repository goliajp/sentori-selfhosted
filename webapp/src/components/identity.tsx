// The user identity chip.
//
// A userKey is a PII-safe hash: 64 characters that mean nothing to
// a human eye. Truncating it printed noise ("a0bafc276e1fd92c…") —
// unreadable, unclickable, unselectable, and meaningless even in
// full. What a triager actually needs from it: tell users APART at
// a glance, and get the full value into a query when it matters.
// So: a colour derived from the hash (stable per user, instantly
// comparable across rows) + a four-character tag, and the full key
// one click away on the clipboard.

import { Check } from 'lucide-react';
import { useState } from 'react';

import { useT } from '../i18n';

/** Fold the key into a hue (0-359). Same key → same colour, and
 *  two users almost never share one. */
function hashHue(key: string): number {
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) | 0;
  return ((h % 360) + 360) % 360;
}

export function UserChip({
  userKey,
  label,
  muted = false,
}: {
  userKey: string;
  /** Human-meaningful text instead of the hash tag — the occurrence
   *  list numbers users per issue ("U1", "U2"), because a hash
   *  fragment carries no meaning for a reader (insight A6). */
  label?: string;
  /** Dimmed when the field doesn't vary across the list. */
  muted?: boolean;
}) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const tag = label ?? `#${userKey.replace(/[^a-zA-Z0-9]/g, '').slice(0, 4) || '····'}`;
  const hue = hashHue(userKey);
  return (
    <button
      type="button"
      title={copied ? t('identity.copied') : t('identity.copyHint')}
      onClick={(e) => {
        e.stopPropagation();
        void navigator.clipboard.writeText(userKey).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        });
      }}
      className={`inline-flex items-center gap-1.5 rounded border border-transparent px-1 py-0 font-mono hover:border-border hover:bg-raised ${
        muted ? 'text-fg-subtle/60' : 'text-inherit'
      }`}
    >
      <span
        aria-hidden
        className="h-2 w-2 rounded-full"
        style={{ backgroundColor: `hsl(${hue} 55% 55%)` }}
      />
      {copied ? (
        <Check aria-hidden className="h-3 w-3 text-ok" />
      ) : (
        <span>{tag}</span>
      )}
    </button>
  );
}
