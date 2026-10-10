/** Proposed commercial catalogue. This preview does not enforce quotas or collect payments. */
export const cloudPlans = [
  { id: 'free', name: 'Free', monthly: 0, annual: 0, events: 10_000, replays: 100, ingestGB: 1, artifactsGB: 1, projects: 3, eventDays: 7, replayDays: 7, rate: 20 },
  { id: 'starter', name: 'Starter', monthly: 19, annual: 190, events: 100_000, replays: 1_000, ingestGB: 10, artifactsGB: 5, projects: 10, eventDays: 30, replayDays: 14, rate: 100 },
  { id: 'team', name: 'Team', monthly: 69, annual: 690, events: 500_000, replays: 5_000, ingestGB: 50, artifactsGB: 20, projects: 30, eventDays: 90, replayDays: 30, rate: 300 },
  { id: 'scale', name: 'Scale', monthly: 199, annual: 1990, events: 2_000_000, replays: 20_000, ingestGB: 200, artifactsGB: 50, projects: 100, eventDays: 90, replayDays: 30, rate: 1_000 },
] as const;
export type CloudPlan = typeof cloudPlans[number];
export type BillingCycle = 'monthly' | 'yearly';
export const capacityPacks = {
  events: { units: 100_000, ingestGB: 5, price: 10 },
  replays: { units: 1_000, ingestGB: 5, price: 10 },
  data: { ingestGB: 10, price: 5 },
} as const;
export function estimatePlan(plan: CloudPlan, events: number, replays: number, ingestGB: number, cycle: BillingCycle = 'monthly') {
  if (![events, replays, ingestGB].every(n => Number.isFinite(n) && n >= 0)) throw new RangeError('Usage must be finite and non-negative');
  const eventPacks = Math.max(0, Math.ceil((events - plan.events) / capacityPacks.events.units));
  const replayPacks = Math.max(0, Math.ceil((replays - plan.replays) / capacityPacks.replays.units));
  const dataPacks = Math.max(0, Math.ceil((ingestGB - plan.ingestGB - (eventPacks + replayPacks) * 5) / 10));
  const eligible = plan.id !== 'free' || eventPacks + replayPacks + dataPacks === 0;
  const addons = (eventPacks + replayPacks) * 10 + dataPacks * 5;
  return { plan, eligible, eventPacks, replayPacks, dataPacks, addons, total: (cycle === 'yearly' ? plan.annual / 12 : plan.monthly) + addons };
}
export function recommendPlan(events: number, replays: number, ingestGB: number, cycle: BillingCycle = 'monthly') {
  return cloudPlans.map(plan => estimatePlan(plan, events, replays, ingestGB, cycle)).filter(result => result.eligible).sort((a,b) => a.total - b.total || b.plan.events - a.plan.events)[0]!;
}
export const selfHostedServices = [
  { id: 'launch', price: 499, hours: 4 },
  { id: 'care', price: 149, hours: 1 },
  { id: 'partner', price: 599, hours: 4 },
] as const;
