# iOS Main Thread Sampler — Privacy & App Store Review Notes

> **Status:** Phase 29 sub-A step 1. Documents the Mach + pthread + dyld
> APIs the upcoming `SentoriThreadSampler.swift` will call, the App
> Store review risk for each, and the privacy boundary. Written before
> the Swift implementation lands so we can rule API choices in or out
> before writing code we'd later need to rip out.

## Why we need this

`SentoriHangWatchdog.swift` currently captures
`Thread.callStackSymbols` from the watchdog thread itself, not from
main, because main is wedged when we want to sample it. The captured
stack therefore points into our own timer machinery, which is useless
for diagnosing the user's hang. (The file's own comment at line 100
flags this as a stop-gap.)

To get the actual wedged main-thread frames we have to walk a remote
thread's frame pointer chain — main is alive but not running our
code, so we resolve it via Mach + pthread APIs. This document checks
those APIs against App Store Review Guideline 2.5.1 ("Apps must use
only public APIs") before we commit to them.

## API inventory

### Public, App Store-safe

| API | Header | Purpose | Risk |
|---|---|---|---|
| `pthread_main_np()` | `<pthread.h>` | bool: am I on main? Sanity-check before sampling. | none |
| `pthread_self()` | `<pthread.h>` | get current pthread for `pthread_mach_thread_np`. | none |
| `pthread_mach_thread_np(pthread_t)` | `<pthread.h>` | pthread → mach port. The `_np` suffix is Apple's mark for "non-portable extension"; it's a public API, just not POSIX-portable. | none |
| `mach_task_self()` | `<mach/mach.h>` | this process's task port. We pass our own task only; we never look up another. | none |
| `thread_get_state(thread, ARM_THREAD_STATE64, ...)` | `<mach/thread_act.h>` | read main thread's PC / FP / SP / LR. Same call sentry-cocoa, Firebase Crashlytics, and Bugsnag use. | low — public Mach API; reviewers expect to see it from crash-reporter SDKs. |
| `vm_read_overwrite(self_task, addr, size, dst, ...)` | `<mach/vm_map.h>` | safe (no SIGSEGV) read of own-process memory. We restrict to `mach_task_self()` and small reads (each frame is two pointers). | low — public; flagged only when used to read other processes' memory. |
| `_dyld_image_count()` / `_dyld_get_image_header()` / `_dyld_get_image_vmaddr_slide()` / `_dyld_get_image_uuid()` | `<mach-o/dyld.h>` | LC_UUID for dSYM matching, ASLR slide for offset calc. Public dyld API. | none |

### Explicitly NOT used (private API risk)

| API | Why we don't use it |
|---|---|
| `_pthread_main_thread_np` (underscore prefix) | private alias of `pthread_main_np()`; underscore prefix in Apple SDK = SPI, rejection-grade. |
| `task_threads(other_task, ...)` cross-task | requires `task-port` entitlement and is reviewer-flagged. We only ever look at our own task. |
| `task_for_pid` | gated by entitlement; not appropriate for our same-process use. |
| `__platform_call_*` / `kdebug_trace` / signal hooking | private. |
| `_dyld_register_func_for_*` introspection beyond UUID | not needed for stack walking. |

## Why this is App Store safe

Direct prior art shipping the same call set, no review issues:

- **sentry-cocoa** — uses `thread_get_state` + `vm_read_overwrite` for
  slow-frame and ANR sampling, in millions of apps.
- **Firebase Crashlytics** — same primitives for ANR + hang capture.
- **Bugsnag**, **Embrace** — likewise.
- **Apple's MetricKit** (`MXCallStackTree`) walks call stacks via the
  same public Darwin primitives. Apple themselves consume this
  surface.

Apple's stance: Review Guideline 2.5.1 forbids non-public APIs, but
"non-public" means undocumented / underscore-prefixed / not in the
public SDK. All calls in the public-safe table above are documented
in Apple's developer reference and shipped in `<mach/...>` /
`<pthread.h>` / `<mach-o/dyld.h>` headers that come with Xcode by
default.

## Privacy considerations

What we capture per hang event:

- Up to 64 PC values (program counter addresses) from the main
  thread's frame pointer chain.
- The `LC_UUID` of each loaded image and its ASLR vmaddr slide, so
  the server can match PCs back to a dSYM (Phase 22 sub-B field).
- Hang duration in milliseconds (already captured today).

What we explicitly do NOT capture:

- Other processes' memory. We only pass `mach_task_self()` to
  `vm_read_overwrite`.
- Function arguments, local variables, or register contents — PC
  only, never the rest of the `arm_thread_state64_t` struct.
- Heap content, NSString contents, user-typed text, PII.
- Continuous-rate samples. Sampling fires only on a detected hang
  (≥ 2s main-thread block) and is one-shot per hang (see watchdog
  `reportedThisHang` flag at `SentoriHangWatchdog.swift:54`).

Symbolication happens **server-side** against the uploaded dSYM
(`server/src/symbolicate.rs`). On-device,
`frames[].instructionAddress` is an ASLR-slid pointer with no
semantic content until paired with the dSYM.

For the user-facing privacy doc (what data Sentori collects and why),
see `docs/legal/privacy.md`.

## Rejection contingency

If Apple Review ever rejects with reference to `vm_read_overwrite` or
`thread_get_state`:

1. Confirm sentry-cocoa / Firebase / Bugsnag are still shipping the
   same call set. They're the canary; rejection there means a policy
   change everyone needs to handle.
2. Switch to `backtrace()` from `<execinfo.h>` — pure libc, walks
   only the *current* thread, which is the watchdog thread, not
   main. Lower fidelity (back where we started), zero risk.
3. Last resort: ship without main-thread sampler; emit hang events
   with empty `frames[]` and `tags.source = "sentori.hangWatchdog.no-sampler"`
   so the dashboard can flag the gap. Feature degrades, doesn't
   break.

## Implementation references

- `sdk/react-native/ios/SentoriThreadSampler.swift` — to be created in
  Phase 29 sub-A step 2: `captureMainThreadFrames(maxFrames: Int = 64) -> [(pc: UInt64, fp: UInt64)]`
- `sdk/react-native/ios/SentoriHangWatchdog.swift` — current
  `Thread.callStackSymbols` capture (line 108) replaced by sampler
  call in step 4.
- `server/src/symbolicate.rs` — dSYM lookup + frame resolution
  (existing, Phase 22 sub-B).
- `docs/protocol.md` — `frames[].instructionAddress` + `debugId` +
  `arch` field definitions (existing, Phase 22 sub-B).
