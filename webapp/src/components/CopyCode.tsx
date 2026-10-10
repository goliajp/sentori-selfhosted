import { Check, Copy } from 'lucide-react';
import { useState } from 'react';

import { useT } from '../i18n';

export function CopyCode({ code, label }: { code: string; label: string }) {
  const t = useT();
  const [result, setResult] = useState<{ code: string; ok: boolean } | null>(null);
  const copied = result?.code === code && result.ok;
  return (
    <div className="overflow-hidden rounded-xl border border-border bg-bg">
      <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-2">
        <span className="text-xs font-medium text-fg-muted">{label}</span>
        <button
          type="button"
          aria-label={t('setup.copyCode')}
          className="flex min-h-8 items-center gap-1.5 rounded px-2 text-xs text-fg-muted hover:bg-raised hover:text-fg"
          onClick={() => {
            if (!navigator.clipboard) { setResult({ code, ok: false }); return; }
            void navigator.clipboard.writeText(code).then(
              () => setResult({ code, ok: true }),
              () => setResult({ code, ok: false }),
            );
          }}
        >
          {copied ? <Check className="h-3.5 w-3.5" aria-hidden /> : <Copy className="h-3.5 w-3.5" aria-hidden />}
          {t(copied ? 'setup.copied' : 'setup.copy')}
        </button>
      </div>
      <pre tabIndex={0} className="overflow-x-auto p-4 font-mono text-xs leading-7 text-fg sm:text-sm"><code>{code}</code></pre>
      {result?.code === code && !result.ok && <p role="status" className="px-4 pb-3 text-xs text-danger">{t('setup.copyFailed')}</p>}
    </div>
  );
}
