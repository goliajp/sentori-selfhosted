# Contributing

Thanks for looking. Read the first section before opening a pull request — it will save you the work.

## What this repository is

`goliajp/sentori-selfhosted` is a **mirror**, not the upstream. Development happens in a private repository and lands here through a workflow that rebuilds this history from scratch and force-pushes `master` on every release:

```
git init -b master && git add -A && git commit && git push -f origin master
```

Two consequences, and neither is negotiable from this side:

- **A merged pull request would not survive.** Merging into `master` here puts a commit on a branch that the next mirror run replaces wholesale. The commit, and your authorship of it, would disappear at the next release with nothing to show it had been there.
- **There is no CI here.** Workflow files are not mirrored, so nothing runs on a push or a PR. Whatever you see green, you ran yourself.

The workflow that produces this repo described its purpose as "fork / inspect / **contribute**" for months. The first two are true. The third was not, and this file exists because saying so is better than letting someone find out by spending an afternoon on a patch.

## So how do you contribute

**Open an issue.** Issues live in this repository, are not touched by the mirror, and are read. A good bug report — what you ran, what happened, what you expected, the `version` field from `/healthz` — is worth more than a patch, because it is the half that cannot be written by someone who does not have your setup.

**Send a patch inside the issue** if you have one. A diff, a branch on your fork, or a PR left open as a reference — any of them work. What happens next is that it gets applied upstream, and arrives back here on the next release with a note crediting you in the changelog. Your PR gets closed rather than merged. That is the mechanism being honest, not the patch being rejected.

**For anything large, open the issue first.** Not process for its own sake: the upstream tree has surfaces this mirror does not carry, and a redesign that is clean here can be impossible three files away.

## Running the checks

Upstream has one command (`bun run preflight`, about thirty checks). It does not work here — several of its checks read the workflow files and mirror configuration, which are not part of this repository. Run the pieces that apply:

```sh
# Rust: the shared libraries and the server binary
cd core               && cargo fmt --check \
                      && cargo clippy --workspace --all-targets -- -D warnings \
                      && cargo test --workspace \
                      && RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cd self-hosted/server && cargo fmt --check \
                      && cargo clippy --all-targets -- -D warnings \
                      && cargo test

# Dashboard
cd webapp && bun install --frozen-lockfile && bun run check

# SDKs
bun install --frozen-lockfile && bun run test:sdks
```

`cargo doc` is in that list deliberately. A broken intra-doc link is the cheapest available detector for documentation that names something no longer in the tree, and this project has shipped a lot of that.

The end-to-end suite brings the real stack up with docker and asserts against it:

```sh
bash self-hosted/tests/e2e/smoke.sh   # needs docker compose v2, jq, curl
```

## Architecture

Every change should sit in one of three tiers ([cement-stone](https://github.com/goliajp/global-config/blob/master/methodology/steel-cement-stone.md)):

1. **石头 (stone)** — no business coupling, would work in another project. `core/crates/{privacy-salt, issue-fingerprint, rate-limiter, …}`. Bench, fuzz, proptest, high coverage.
2. **钢筋 (steel)** — knows the domain, not the flow. `core/crates/{ingest-token, attachment-store, push-provider, notifier}`.
3. **水泥 (cement)** — the composition that runs. `self-hosted/server`.

`core/README.md` has the full assignment, taken from each crate's own `Cargo.toml`. Don't mix tiers: business types in a stone, or a general-purpose utility in cement, are both reasons a change gets sent back.

## Code style

- Rust 2024 edition. `cargo fmt` is the source of truth; there is nothing to discuss about formatting.
- `clippy::pedantic + nursery` on by default. A per-crate `#![allow]` is fine when the lint fights an otherwise-clean idiom — document why, inline.
- `#![forbid(unsafe_code)]` across `core/`.
- Migrations are embedded at compile time and run at boot, so they re-apply on every restart of every installation. Make them idempotent.

## The client zero-cost rule

The SDK must never cost the host application anything: no perceptible main-thread work, no chatty network, no failure that reaches the host, and bounded memory and disk. The five event verbs are synchronous, return no promise, and never throw — a Sentori bug must not become the host app's bug.

A patch that adds a `throw` to an SDK path, or work to a hot one, needs a measurement showing what it costs. `sdk/react-native/src/__tests__/iron-rule.test.ts` is where those limits are asserted.

## License

By contributing you agree your work is dual-licensed **Apache-2.0 OR MIT**, matching the repository. Copyright is held by GOLIA K.K. — see `LICENSE-APACHE`, `LICENSE-MIT` and `NOTICES.md`.
