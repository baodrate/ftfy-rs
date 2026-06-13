#![no_main]
//! `fix_and_explain` must never panic. For **single-segment** input (no
//! line breaks) its reported `text` must equal what `fix_text` produces.
//!
//! The newline restriction is essential and matches reference ftfy:
//! `fix_and_explain` fixes the input as a single segment, while `fix_text`
//! splits on line breaks and fixes each line independently. On multi-line
//! input the two legitimately diverge (a line's encoding-badness is judged
//! in isolation by `fix_text`, but in aggregate by `fix_and_explain`), and
//! ftfy diverges in exactly the same way — so asserting equality there
//! would be testing a property neither library holds. We therefore only
//! assert the equality for line-break-free inputs, and otherwise just
//! require that neither call panics.

use libfuzzer_sys::fuzz_target;
use plsfix::{fix_and_explain, fix_text, TextFixerConfig};

fn has_line_break(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c, '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}'))
}

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let config = TextFixerConfig::default();
        let explained = fix_and_explain(text, true, Some(&config));
        let plain = fix_text(text, Some(&config));
        if !has_line_break(text) {
            assert_eq!(
                explained.text, plain,
                "fix_and_explain text disagrees with fix_text for \
                 single-segment input {text:?}"
            );
        }
    }
});
