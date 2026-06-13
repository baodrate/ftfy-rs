#![no_main]
//! Convergence: repeatedly applying `fix_text` must reach a fixed point
//! quickly and must never oscillate or grow without bound.
//!
//! Note: single-pass idempotence (`fix(fix(x)) == fix(x)`) does NOT hold —
//! and crucially, it does not hold for reference ftfy either. When a line
//! break sits next to mojibake, the first pass normalizes the break and
//! only the second pass repairs the now-adjacent encoding. plsfix
//! replicates that behavior exactly. What both libraries do guarantee is
//! that the process *converges*: here we assert a fixed point is reached
//! within `MAX_PASSES`, which catches real oscillation/divergence bugs
//! without flagging the shared multi-pass quirk.

use libfuzzer_sys::fuzz_target;
use plsfix::{fix_text, TextFixerConfig};

const MAX_PASSES: usize = 16;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let config = TextFixerConfig::default();
        let mut prev = fix_text(text, Some(&config));
        for pass in 0..MAX_PASSES {
            let next = fix_text(&prev, Some(&config));
            if next == prev {
                return; // converged
            }
            prev = next;
            let _ = pass;
        }
        panic!(
            "fix_text did not converge within {MAX_PASSES} passes for input {text:?}"
        );
    }
});
