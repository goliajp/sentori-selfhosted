// v1.1 WASM kernel — trust score math.
//
// Mirrors `server/src/api/trust_score.rs::score_from_counts` exactly:
// `100 - sum(weight[i] * count[i])`, floored at 0, ceilinged at 100.
// Bound at 64 kinds; the Rust trust scoring engine never produces
// more than that in 24h since each kind is a single string the
// server emits.
//
// API contract (raw WebAssembly, no bindgen — keeps the .wasm tiny
// and the JS loader trivial):
//
//   set_pair(idx, weight, count)  // write into the staging buffers
//   score(len)                    // compute over [0..len)
//   reset()                       // optional; zeroes the buffers
//
// Callers stage `len` rows via `set_pair`, then read the result of
// `score(len)`. No allocation; no panics; no JS-side memory layout
// concerns. The single Memory instance holds the buffers as static
// globals.

#![no_std]
#![allow(static_mut_refs)]

const MAX_KINDS: usize = 64;

static mut WEIGHTS: [i32; MAX_KINDS] = [0; MAX_KINDS];
static mut COUNTS: [i32; MAX_KINDS] = [0; MAX_KINDS];

#[unsafe(no_mangle)]
pub extern "C" fn set_pair(idx: usize, weight: i32, count: i32) {
    if idx >= MAX_KINDS {
        return;
    }
    unsafe {
        WEIGHTS[idx] = weight;
        COUNTS[idx] = count;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn score(len: usize) -> i32 {
    let len = if len > MAX_KINDS { MAX_KINDS } else { len };
    let mut penalty: i32 = 0;
    for i in 0..len {
        let take = unsafe { WEIGHTS[i].saturating_mul(COUNTS[i]) };
        penalty = penalty.saturating_add(take);
    }
    let s = 100i32.saturating_sub(penalty);
    if s < 0 {
        0
    } else if s > 100 {
        100
    } else {
        s
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn reset() {
    unsafe {
        WEIGHTS = [0; MAX_KINDS];
        COUNTS = [0; MAX_KINDS];
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn max_kinds() -> usize {
    MAX_KINDS
}

// `panic = abort` in Cargo.toml drives the actual abort, so the
// handler exists only to satisfy `no_std`. Will never be reached.
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
