# Source-map e2e

Proves that a real bundler's source map, uploaded over HTTP and matched
to a release by name, turns a minified stack frame back into a line of
source.

The unit tests in `self-hosted/server/src/symbolicate.rs` pin the
resolver's arithmetic against a hand-built map. They cannot tell you
that the upload endpoint, the blob store, the release lookup and the
ingest hook all line up — that is what this does.

## Run it

```sh
# a server with its own database, blobs on disk
docker run -d --rm --name sme2e-pg -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=sentori -p 55432:5432 postgres:18-alpine

cd self-hosted/server
SENTORI_DATABASE_URL=postgres://postgres:dev@localhost:55432/sentori \
SENTORI_SESSION_SECRET=$(openssl rand -hex 32) \
SENTORI_BOOTSTRAP_OWNER_EMAIL=ci@example.com \
SENTORI_BOOTSTRAP_OWNER_PASSWORD=ci-password-long-enough \
SENTORI_BIND=127.0.0.1:8099 \
SENTORI_ATTACHMENT_STORE=fs:/tmp/sme2e-blobs \
  cargo run &

SENTORI_BASE=http://localhost:8099 \
SENTORI_OWNER_EMAIL=ci@example.com \
SENTORI_OWNER_PASSWORD=ci-password-long-enough \
  bash scripts/sourcemap-e2e/run-v02.sh
```

`SENTORI_ATTACHMENT_STORE` has to be `fs:` — the default in-memory
store would make this pass without ever writing a blob to disk, hiding
the path a real deployment takes.

## What it asserts

- Something was symbolicated at all (`symbolicated: true` on a frame).
- The resolved line is **not** line 1. The bundle is a single line, so a
  mismatched map resolves everything back to it, and a test that only
  checked "a file name changed" would pass.
- The pre-symbolication coordinates survive. A stale map produces
  confident nonsense, and the original position is the only way anyone
  can tell.

## Files

- `app.js` — the fixture. Three nested calls so the stack has depth;
  plain JS so bundling needs no Babel preset.
- `throw-and-format.js` — runs the bundle via `vm.runInThisContext` with
  the bundle's filename, so the stack really points inside it, and
  shapes the result into a Sentori event.
- `run-v02.sh` — the driver.
