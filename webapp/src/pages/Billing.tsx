// Billing page — the caller's own workspace subscription.
//
// Shows current plan / status / usage, and (when Stripe is
// configured) drives self-serve upgrade via Checkout + management
// via the Billing Portal. In a self-hosted deployment with no
// Stripe keys the mutating controls hide and plan changes are an
// operator concern.

import { useCallback, useState } from 'react';
import { useSearchParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api, BillingInfo, PlanName, UsageCounter } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  ErrorBanner,
  PageHeader,
  formatNumber,
} from '../components/ui';

// Enterprise limits ship as i64::MAX, which lands here as a ~9.2e18
// float; anything past the safe-integer range means "no cap".
const UNLIMITED = 1e15;

const PLAN_LABEL: Record<PlanName, string> = {
  free: 'Free',
  pro: 'Pro',
  enterprise: 'Enterprise',
};

export default function Billing() {
  const t = useT();
  const {
    data,
    loading,
    error,
    reload,
    setError,
  } = useAsyncData<BillingInfo>(() => api.billing(), [], String);
  const [busy, setBusy] = useState(false);
  const [params, setParams] = useSearchParams();
  const checkout = params.get('checkout');

  const startCheckout = useCallback(
    async (plan: 'pro' | 'enterprise') => {
      setBusy(true);
      try {
        const { url } = await api.billingCheckout(plan);
        window.location.href = url;
      } catch (e) {
        setError(String(e));
        setBusy(false);
      }
    },
    [setError],
  );

  const openPortal = useCallback(async () => {
    setBusy(true);
    try {
      const { url } = await api.billingPortal();
      window.location.href = url;
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }, [setError]);

  function dismissBanner() {
    params.delete('checkout');
    setParams(params, { replace: true });
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('billing.title')}
        subtitle={t('billing.subtitle')}
      />

      {checkout === 'success' && (
        <Banner tone="ok" onClose={dismissBanner}>
          Checkout complete — your new plan activates as soon as Stripe
          confirms the payment (usually seconds).
        </Banner>
      )}
      {checkout === 'cancel' && (
        <Banner tone="warn" onClose={dismissBanner}>
          Checkout canceled — no changes were made.
        </Banner>
      )}
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {loading && !data ? (
        <div className="py-12 text-center text-sm text-fg-subtle">Loading…</div>
      ) : data ? (
        <>
          <PlanCard info={data} onManage={openPortal} busy={busy} />
          <UsageCard info={data} />
          {data.stripe_enabled && !data.webhook_configured && (
            <Card>
              <CardBody>
                <p className="text-sm text-warn">
                  {t('billing.webhookMissing')}
                </p>
                <p className="mt-1 text-xs text-fg-subtle">
                  {t('billing.webhookMissingHint')}
                </p>
              </CardBody>
            </Card>
          )}
          {data.stripe_enabled ? (
            <UpgradeCard
              info={data}
              onUpgrade={startCheckout}
              onRefresh={reload}
              busy={busy}
            />
          ) : (
            <Card>
              <CardHeader title={t('billing.unavailable')} />
              <CardBody>
                <p className="text-sm text-fg-subtle">
                  This deployment has no Stripe keys configured, so plan
                  changes are managed by the operator. Contact your admin
                  to change tiers.
                </p>
              </CardBody>
            </Card>
          )}
        </>
      ) : null}
    </div>
  );
}

function PlanCard({
  info,
  onManage,
  busy,
}: {
  info: BillingInfo;
  onManage: () => void;
  busy: boolean;
}) {
  const t = useT();
  const downgraded = info.effective_plan !== info.plan;
  return (
    <Card>
      <CardHeader
        title={t('billing.current')}
        action={
          info.has_customer ? (
            <Button variant="secondary" onClick={onManage} disabled={busy}>
              {t('billing.manage')} →
            </Button>
          ) : undefined
        }
      />
      <CardBody>
        <div className="flex flex-wrap items-center gap-6">
          <Field label={t('settings.plan')}>
            <Badge tone={info.plan === 'free' ? 'neutral' : 'info'}>
              {PLAN_LABEL[info.plan]}
            </Badge>
          </Field>
          <Field label={t('crash.status')}>
            <Badge tone={statusTone(info.status)}>{info.status}</Badge>
          </Field>
          {info.current_period_end && (
            <Field label={t('billing.renews')}>
              <span className="font-mono text-sm text-fg-muted">
                {new Date(info.current_period_end).toLocaleDateString()}
              </span>
            </Field>
          )}
          {downgraded && (
            <Field label={t('billing.enforced')}>
              <Badge tone="warn">{PLAN_LABEL[info.effective_plan]}</Badge>
            </Field>
          )}
        </div>
        {downgraded && (
          <p className="mt-3 text-xs text-warn">{t('billing.downgradedNote')}</p>
        )}
        {info.has_customer && (
          <p className="mt-3 text-xs text-fg-subtle">{t('billing.manageHint')}</p>
        )}
      </CardBody>
    </Card>
  );
}

function UsageCard({ info }: { info: BillingInfo }) {
  const t = useT();
  return (
    <Card>
      <CardHeader title={`${t('billing.usage')} · ${info.period_yyyymm}`} />
      <CardBody>
        <div className="space-y-4">
          <UsageBar label={t('events.title')} counter={info.usage.events} />
          <UsageBar label={t('overview.spans')} counter={info.usage.spans} />
          <UsageBar label={t('replays.title')} counter={info.usage.replays} />
        </div>
      </CardBody>
    </Card>
  );
}

function UsageBar({ label, counter }: { label: string; counter: UsageCounter }) {
  const t = useT();
  const unlimited = counter.limit >= UNLIMITED;
  const pct = unlimited
    ? 0
    : Math.min(100, Math.round((counter.count / Math.max(1, counter.limit)) * 100));
  const near = pct >= 90;
  return (
    <div>
      <div className="mb-1 flex items-baseline justify-between text-sm">
        <span className="text-fg-muted">{label}</span>
        <span className="font-mono text-xs text-fg-subtle">
          {formatNumber(counter.count)}
          {unlimited ? ' / ∞' : ` / ${formatNumber(counter.limit)}`}
          {counter.dropped > 0 && (
            <span className="ml-2 text-danger">
              {t('billing.dropped').replace('{n}', formatNumber(counter.dropped))}
            </span>
          )}
        </span>
      </div>
      <div className="h-1.5 w-full overflow-hidden rounded bg-raised">
        <div
          className={`h-full rounded ${near ? 'bg-danger/10' : 'bg-accent'}`}
          style={{ width: unlimited ? '4%' : `${pct}%` }}
        />
      </div>
    </div>
  );
}

function UpgradeCard({
  info,
  onUpgrade,
  onRefresh,
  busy,
}: {
  info: BillingInfo;
  onUpgrade: (plan: 'pro' | 'enterprise') => void;
  onRefresh: () => void;
  busy: boolean;
}) {
  const t = useT();
  const options: { plan: 'pro' | 'enterprise'; show: boolean }[] = [
    { plan: 'pro', show: info.upgradeable.pro },
    { plan: 'enterprise', show: info.upgradeable.enterprise },
  ];
  const anyUpgrade = options.some(o => o.show && info.plan !== o.plan);
  return (
    <Card>
      <CardHeader
        title={t('billing.change')}
        action={
          <Button variant="ghost" size="sm" onClick={onRefresh} disabled={busy}>{t('action.refresh')}</Button>
        }
      />
      <CardBody>
        {anyUpgrade ? (
          <div className="flex flex-wrap gap-3">
            {options
              .filter(o => o.show && info.plan !== o.plan)
              .map(o => (
                <Button
                  key={o.plan}
                  variant="primary"
                  onClick={() => onUpgrade(o.plan)}
                  disabled={busy}
                >
                  {(info.plan === 'free'
                    ? t('billing.upgradeTo')
                    : t('billing.switchTo')
                  ).replace('{plan}', PLAN_LABEL[o.plan])}{' '}
                  →
                </Button>
              ))}
          </div>
        ) : (
          <p className="text-sm text-fg-subtle">
            {info.has_customer
              ? t('billing.changeViaPortal')
              : t('billing.highestPlan')}
          </p>
        )}
      </CardBody>
    </Card>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <p className="mb-1 text-xs uppercase tracking-wide text-fg-subtle">
        {label}
      </p>
      <div>{children}</div>
    </div>
  );
}

function Banner({
  tone,
  onClose,
  children,
}: {
  tone: 'ok' | 'warn';
  onClose: () => void;
  children: React.ReactNode;
}) {
  const t = useT();
  const cls =
    tone === 'ok'
      ? 'border-accent/40 bg-accent/40 text-accent'
      : 'border-warn/40 bg-warn/40 text-warn';
  return (
    <div
      className={`flex items-start justify-between gap-4 rounded border px-4 py-3 text-sm ${cls}`}
    >
      <span>{children}</span>
      <button
        onClick={onClose}
        className="shrink-0 text-xs opacity-70 hover:opacity-100"
      >{t('action.dismiss')}</button>
    </div>
  );
}

function statusTone(
  status: string,
): 'neutral' | 'ok' | 'warn' | 'danger' | 'info' {
  switch (status) {
    case 'active':
    case 'trialing':
      return 'ok';
    case 'past_due':
      return 'warn';
    case 'canceled':
    case 'unpaid':
      return 'danger';
    default:
      return 'neutral';
  }
}
