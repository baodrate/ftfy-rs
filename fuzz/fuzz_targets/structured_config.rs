#![no_main]
//! Fuzz the full configuration surface, not just the default. `arbitrary`
//! derives a config (every boolean toggle, the html tri-state, a
//! normalization choice, and a non-zero decode length) plus an input
//! string, so any combination of options is exercised. Invariants:
//! `fix_text` never panics for any config, and converges to a fixed point
//! within `MAX_PASSES` under that same config (single-pass idempotence is
//! not guaranteed — see `idempotent.rs`).

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::num::NonZeroUsize;

use plsfix::{fix_text, Normalization, TextFixerConfig};

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    text: String,
    unescape_html: Option<bool>,
    remove_terminal_escapes: bool,
    fix_encoding: bool,
    restore_byte_a0: bool,
    replace_lossy_sequences: bool,
    decode_inconsistent_utf8: bool,
    fix_c1_controls: bool,
    fix_latin_ligatures: bool,
    fix_character_width: bool,
    uncurl_quotes: bool,
    fix_line_breaks: bool,
    remove_control_chars: bool,
    normalization: Option<NormChoice>,
    // Keep the decode length small-ish so chunk boundaries are hit often.
    max_decode_length: u16,
}

#[derive(Arbitrary, Debug)]
enum NormChoice {
    Nfc,
    Nfkc,
    Nfd,
    Nfkd,
}

impl From<&NormChoice> for Normalization {
    fn from(c: &NormChoice) -> Self {
        match c {
            NormChoice::Nfc => Normalization::NFC,
            NormChoice::Nfkc => Normalization::NFKC,
            NormChoice::Nfd => Normalization::NFD,
            NormChoice::Nfkd => Normalization::NFKD,
        }
    }
}

fuzz_target!(|input: FuzzInput| {
    let config = TextFixerConfig {
        unescape_html: input.unescape_html,
        remove_terminal_escapes: input.remove_terminal_escapes,
        fix_encoding: input.fix_encoding,
        restore_byte_a0: input.restore_byte_a0,
        replace_lossy_sequences: input.replace_lossy_sequences,
        decode_inconsistent_utf8: input.decode_inconsistent_utf8,
        fix_c1_controls: input.fix_c1_controls,
        fix_latin_ligatures: input.fix_latin_ligatures,
        fix_character_width: input.fix_character_width,
        uncurl_quotes: input.uncurl_quotes,
        fix_line_breaks: input.fix_line_breaks,
        remove_control_chars: input.remove_control_chars,
        normalization: input.normalization.as_ref().map(Normalization::from),
        // Clamp to a valid NonZeroUsize; 0 is disallowed by the type.
        max_decode_length: NonZeroUsize::new(input.max_decode_length as usize + 1)
            .unwrap(),
    };

    const MAX_PASSES: usize = 16;
    let mut prev = fix_text(&input.text, Some(&config));
    for _ in 0..MAX_PASSES {
        let next = fix_text(&prev, Some(&config));
        if next == prev {
            return;
        }
        prev = next;
    }
    panic!(
        "fix_text did not converge within {MAX_PASSES} passes under config \
         {config:?} for input {:?}",
        input.text
    );
});
