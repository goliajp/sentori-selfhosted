#!/usr/bin/env node
// Every metric an alert rule or dashboard panel queries must be one
// the server actually emits.
//
// Written after finding that all six references were dead. The alert
// rules queried `sentori_pg_pool_in_use` and `sentori_pg_pool_max`
// (the real names are `sentori_db_pool_in_use` and
// `sentori_db_pool_size`); the dashboard queried
// `sentori_ingest_duration_seconds_bucket` and
// `sentori_quota_drops_total`, neither of which any code path has
// published. A rule that cannot fire and a panel that draws nothing
// both look, in review, exactly like monitoring.
//
// Only query expressions are checked — `rules[].expr` in the YAML and
// `panels[].targets[].expr` in the dashboard. Prose may name a metric
// to explain why it is gone; that is documentation, not a query.

import { readFileSync } from 'node:fs';

const ROOT = new URL('..', import.meta.url).pathname;
const SOURCE = 'self-hosted/server/src/handlers/metrics_prom.rs';
const ALERTS = 'ops/prometheus-alerts.yml';
const DASH = 'ops/grafana-sentori-overview.json';

const NAME = /sentori_[a-z0-9_]+/g;

// Emitted names come from string literals in the exposition handler.
// Comment lines are stripped first: this file documents three metrics
// it deliberately removed, and a comment is not an emission.
function emitted() {
    const out = new Set();
    for (const line of readFileSync(ROOT + SOURCE, 'utf8').split('\n')) {
        const code = line.trim();
        if (code.startsWith('//')) continue;
        for (const lit of line.match(/"[^"]*"/g) ?? []) {
            for (const m of lit.match(NAME) ?? []) out.add(m);
        }
    }
    return out;
}

function alertExprs() {
    // Deliberately not a YAML parser: this file has no dependency of
    // its own and preflight must run before `bun install` can matter.
    // `expr:` lines are either inline or a `|` block; both indent the
    // continuation, so take everything until the next same-level key.
    const src = readFileSync(ROOT + ALERTS, 'utf8').split('\n');
    const found = [];
    for (let i = 0; i < src.length; i++) {
        const m = src[i].match(/^(\s*)expr:\s*(.*)$/);
        if (!m) continue;
        const [, indent, rest] = m;
        let text = rest === '|' || rest === '>' ? '' : rest;
        for (let j = i + 1; j < src.length; j++) {
            const l = src[j];
            if (l.trim() === '') continue;
            const lead = l.match(/^\s*/)[0].length;
            if (lead <= indent.length) break;
            if (l.trim().startsWith('#')) continue;
            text += '\n' + l;
        }
        found.push({ where: `${ALERTS}:${i + 1}`, text });
    }
    return found;
}

function dashExprs() {
    const d = JSON.parse(readFileSync(ROOT + DASH, 'utf8'));
    const found = [];
    for (const p of d.panels ?? []) {
        for (const t of p.targets ?? []) {
            if (t.expr) found.push({ where: `${DASH} — ${p.title}`, text: t.expr });
        }
    }
    return found;
}

const have = emitted();
const problems = [];
for (const { where, text } of [...alertExprs(), ...dashExprs()]) {
    for (const name of new Set(text.match(NAME) ?? [])) {
        if (!have.has(name)) problems.push({ where, name });
    }
}

if (problems.length > 0) {
    console.error('✗ query expressions name metrics the server does not emit:\n');
    for (const { where, name } of problems) {
        console.error(`  ${name}`);
        console.error(`    queried at: ${where}`);
    }
    console.error(`\n  emitted by ${SOURCE}:`);
    for (const n of [...have].sort()) console.error(`    ${n}`);
    console.error('\n  Either emit the metric or stop querying it. A rule that');
    console.error('  cannot fire is not coverage.');
    process.exit(1);
}

console.log(
    `✓ ${have.size} metrics emitted; every alert and panel expression names one that exists`,
);
