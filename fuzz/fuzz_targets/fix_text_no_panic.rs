#![no_main]
//! Core invariant: `fix_text` must never panic on any valid UTF-8 input,
//! and its output must itself be valid UTF-8 (guaranteed by the `String`
//! return type, but we also assert the result is a fixed point under a
//! cheap sanity check elsewhere). libFuzzer hands us arbitrary bytes; we
//! only run on the ones that are valid UTF-8, which is the entire domain
//! of the public API.

use libfuzzer_sys::fuzz_target;
use plsfix::{fix_text, TextFixerConfig};

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let config = TextFixerConfig::default();
        let _ = fix_text(text, Some(&config));
    }
});
