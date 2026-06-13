#![no_main]
//! Generate *genuine* mojibake from an arbitrary seed and check recovery
//! behavior. A seed string is UTF-8-encoded, then each byte is
//! reinterpreted as a Latin-1 codepoint — exactly the UTF-8/Latin-1
//! mix-up that produces real-world mojibake (`café` -> `cafÃ©`). Then:
//!
//!   * `fix_text` must not panic on real mojibake shapes;
//!   * repeatedly fixing must converge to a fixed point within `MAX_PASSES`
//!     (see `idempotent.rs` for why single-pass idempotence is not the
//!     right invariant — the same multi-pass behavior exists in ftfy).
//!
//! This is the "does our decoder survive real mojibake shapes" stress test,
//! complementing the differential corpus run on the Python side.

use libfuzzer_sys::fuzz_target;
use plsfix::{fix_text, TextFixerConfig};

const MAX_PASSES: usize = 16;

/// UTF-8 bytes of `seed`, each byte reinterpreted as a Latin-1 codepoint.
fn latin1_mojibake(seed: &str) -> String {
    seed.as_bytes().iter().map(|&b| b as char).collect()
}

fuzz_target!(|seed: &str| {
    let mojibake = latin1_mojibake(seed);
    let config = TextFixerConfig::default();

    let mut prev = fix_text(&mojibake, Some(&config));
    for _ in 0..MAX_PASSES {
        let next = fix_text(&prev, Some(&config));
        if next == prev {
            return;
        }
        prev = next;
    }
    panic!(
        "mojibake fix did not converge within {MAX_PASSES} passes: \
         seed {seed:?} -> moji {mojibake:?}"
    );
});
