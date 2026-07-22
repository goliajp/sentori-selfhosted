-- Sentori core migration 0034 — Stripe webhook dedup + worker ledger.
--
-- Billing Phase 2 (2026-07-21) unifies the SaaS control plane into
-- the single shared DB (the multi-DB-per-tenant design in
-- `saas/migrations/0001` was superseded by the 1:N workspace pivot
-- in 0033). The Stripe webhook receiver + async billing worker now
-- run inside `sentori-server` against this table instead of the
-- never-executed `saas` control-plane database.
--
-- Flow:
--   1. `POST /webhooks/stripe` verifies the signature, then does an
--      idempotent INSERT keyed on `stripe_event_id`. Stripe
--      redelivers a webhook up to 3× when our ack is slow, so the
--      UNIQUE + ON CONFLICT DO NOTHING makes redeliveries no-ops.
--   2. A background worker polls `processed_state = 'pending'`
--      rows (oldest first) and applies each to the owning
--      workspace's billing row via `BillingService::set_plan /
--      set_status`, then marks the row 'processed' (or 'failed'
--      with `process_error` on a non-retryable mapping error).
--
-- The payload is stored verbatim so the worker can be re-run / a
-- new event type can be handled retroactively without re-fetching
-- from Stripe.
CREATE TABLE IF NOT EXISTS stripe_events (
    id                 UUID         PRIMARY KEY,
    -- Stripe's own event id (evt_…). UNIQUE = the dedup key.
    stripe_event_id    TEXT         NOT NULL UNIQUE,
    -- e.g. 'checkout.session.completed',
    -- 'customer.subscription.updated'.
    event_type         TEXT         NOT NULL,
    -- The full event JSON as delivered.
    payload            JSONB        NOT NULL,
    -- 'pending' | 'processed' | 'failed'
    processed_state    TEXT         NOT NULL DEFAULT 'pending'
                                     CHECK (processed_state IN ('pending', 'processed', 'failed')),
    -- Non-null only when processed_state = 'failed'; the worker's
    -- diagnostic (unmapped customer, unknown plan, …).
    process_error      TEXT,
    received_at        TIMESTAMPTZ  NOT NULL DEFAULT now(),
    processed_at       TIMESTAMPTZ
);

-- Worker poll path: cheap scan of just the unprocessed backlog,
-- oldest first. Partial index keeps it tiny once the bulk of rows
-- have flipped to 'processed'.
CREATE INDEX IF NOT EXISTS idx_stripe_events_pending
    ON stripe_events (received_at)
    WHERE processed_state = 'pending';
