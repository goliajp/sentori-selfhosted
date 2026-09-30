#!/usr/bin/env bash
# The mail ports serve the same certificate the web port does.
#
#   bash scripts/check-mail-cert.sh [host]
#
# `SENTORI_SMTP_HOST` terminates TLS with a certificate that is a
# *copy* of the one Caddy renews for the web port. The copy was made by
# hand, twice (2026-05-17 and 2026-07-31), and nothing syncs it — so
# every ninety days Caddy renews, the copy does not, and outgoing mail
# stops. It has now happened at least twice.
#
# Two conditions, both of which have to hold. The first is the one that
# catches it early: a drifted copy shows a different expiry the moment
# Caddy renews, which is thirty days before anything breaks. The second
# is the backstop.
#
# This is a diagnostic, not a preflight gate: it needs the network and
# it checks a host this repository does not own. Run it when mail looks
# wrong, and from whatever watches production.

set -uo pipefail

HOST="${1:-${SENTORI_SMTP_HOST:-mail.golia.ai}}"
MIN_DAYS="${MIN_DAYS:-21}"

enddate() {
    local port="$1"; shift
    echo | timeout 15 openssl s_client -connect "${HOST}:${port}" \
        -servername "$HOST" "$@" 2>/dev/null |
        openssl x509 -noout -enddate 2>/dev/null | cut -d= -f2
}

epoch() {
    # macOS and GNU date disagree on everything except -j / -d.
    date -j -f '%b %d %T %Y %Z' "$1" +%s 2>/dev/null ||
        date -d "$1" +%s 2>/dev/null
}

# Reachability first, and said as its own thing. A laptop behind a
# network that cannot open 443 to this host gets the same empty string
# as a server with no certificate, and reporting that as a certificate
# fault sends the reader after the wrong problem.
if ! timeout 8 bash -c "</dev/tcp/${HOST}/443" 2>/dev/null; then
    echo "! cannot reach ${HOST}:443 from here — nothing was checked." >&2
    echo "  Run this from a host that can, such as the production runner:" >&2
    echo "    ssh <runner> 'bash -s' < scripts/check-mail-cert.sh" >&2
    exit 2
fi

web=$(enddate 443)
if [ -z "$web" ]; then
    echo "✗ ${HOST}:443 is reachable and served no certificate." >&2
    exit 1
fi

problems=0
printf '  %-22s %s\n' "443 (web, renewed)" "$web"

for spec in "587:-starttls smtp" "465:" "993:"; do
    port="${spec%%:*}"
    # shellcheck disable=SC2086
    got=$(enddate "$port" ${spec#*:})
    if [ -z "$got" ]; then
        printf '  %-22s %s\n' "$port" "(no certificate)"
        echo "✗ ${HOST}:${port} served no certificate." >&2
        problems=$((problems + 1))
        continue
    fi
    printf '  %-22s %s\n' "$port" "$got"
    if [ "$got" != "$web" ]; then
        echo "✗ ${HOST}:${port} expires ${got}; :443 expires ${web}." >&2
        echo "  The mail ports are serving a stale copy — the sync is broken." >&2
        problems=$((problems + 1))
    fi
    left=$(( ( $(epoch "$got") - $(date +%s) ) / 86400 ))
    if [ "$left" -lt "$MIN_DAYS" ]; then
        echo "✗ ${HOST}:${port} has ${left} day(s) left, under ${MIN_DAYS}." >&2
        echo "  Caddy renews at 30 days, so anything under this means the copy is not following." >&2
        problems=$((problems + 1))
    fi
done

if [ "$problems" -gt 0 ]; then
    echo >&2
    echo "Outgoing mail fails with 'certificate has expired' once the date passes," >&2
    echo "and the server does not log it: with no notification subscriptions it never" >&2
    echo "attempts a send, so the failure is invisible from here." >&2
    exit 1
fi

echo "✓ ${HOST}: mail ports serve the web certificate, expiring ${web}"
