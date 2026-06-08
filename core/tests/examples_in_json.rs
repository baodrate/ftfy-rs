//! Port of ftfy's `tests/test_examples_in_json.py`.
//!
//! ftfy collects many real-world (and some synthetic) mojibake examples in
//! `tests/test-cases/*.json`. Each case carries the original text, the expected
//! `fix_text` output, and an optional `fixed-encoding` (the expected output of
//! just the encoding-repair stage; defaults to `fixed`). Cases are marked
//! `"pass"` (plsfix should reproduce ftfy's result) or `"fail"` (ftfy's
//! heuristic is known to be insufficient — kept as strict "known failures").
//!
//! The JSON files are copied into `tests/test-cases/` from ftfy's vendored
//! submodule (`third-party/python-ftfy/tests/test-cases`); refresh them from
//! there when bumping the ftfy reference.
//!
//! Differences from the Python original:
//! - ftfy also verifies that `fix_and_explain`'s plan can be replayed with
//!   `apply_plan`. plsfix has no `apply_plan`, and its explanation steps are
//!   plain labels rather than re-applicable operations, so that check is
//!   omitted here.
//! - ftfy obtains the encoding-only fix from `fix_encoding_and_explain`, which
//!   isn't part of plsfix's public API. Instead we use the same alternative
//!   ftfy itself asserts is equivalent: `fix_text` with every non-encoding
//!   fixer disabled.

use std::fs;
use std::path::PathBuf;

use plsfix::{fix_text, TextFixerConfig};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TestCase {
    label: String,
    original: String,
    fixed: String,
    #[serde(rename = "fixed-encoding")]
    fixed_encoding: Option<String>,
    expect: String,
}

impl TestCase {
    /// `fixed-encoding` falls back to `fixed` when unspecified, matching ftfy.
    fn expected_encoding(&self) -> &str {
        self.fixed_encoding.as_deref().unwrap_or(&self.fixed)
    }
}

fn test_cases_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/test-cases")
}

fn load_test_data() -> Vec<TestCase> {
    let dir = test_cases_dir();
    let entries = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read ftfy test cases at {dir:?}: {e}"));

    let mut cases = Vec::new();
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let contents = fs::read_to_string(&path).unwrap();
        let file_cases: Vec<TestCase> = serde_json::from_str(&contents)
            .unwrap_or_else(|e| panic!("failed to parse {path:?}: {e}"));
        cases.extend(file_cases);
    }

    assert!(!cases.is_empty(), "no test cases were loaded from {dir:?}");
    cases
}

/// A config that runs *only* the encoding-repair stage (and its sub-fixers),
/// mirroring the keyword arguments ftfy disables to isolate `fix_encoding`.
fn encoding_only_config() -> TextFixerConfig {
    TextFixerConfig {
        unescape_html: Some(false),
        remove_terminal_escapes: false,
        fix_character_width: false,
        fix_latin_ligatures: false,
        uncurl_quotes: false,
        fix_line_breaks: false,
        remove_control_chars: false,
        normalization: None,
        // `fix_encoding` and its sub-fixers (restore_byte_a0,
        // replace_lossy_sequences, decode_inconsistent_utf8, fix_c1_controls)
        // keep their enabled defaults, just like ftfy's standalone fix_encoding.
        ..Default::default()
    }
}

/// ftfy's `extra_bad`: re-break the text by encoding as UTF-8 and decoding the
/// bytes as Latin-1 (every byte becomes the codepoint of the same value).
fn extra_bad(text: &str) -> String {
    text.bytes().map(|b| b as char).collect()
}

#[test]
fn test_well_formed_examples() {
    for case in load_test_data() {
        assert!(
            case.expect == "pass" || case.expect == "fail",
            "{}: unexpected `expect` value {:?}",
            case.label,
            case.expect
        );
    }
}

#[test]
fn test_json_pass_examples() {
    let encoding_config = encoding_only_config();
    let mut failures = Vec::new();

    for case in load_test_data().into_iter().filter(|c| c.expect == "pass") {
        // Fix the text as intended.
        let fixed = fix_text(&case.original, None);
        if fixed != case.fixed {
            failures.push(format!(
                "[{}] fix_text\n  expected: {:?}\n  actual:   {:?}",
                case.label, case.fixed, fixed
            ));
        }

        // The encoding-only fix should match `fixed-encoding`.
        let encoding_fix = fix_text(&case.original, Some(&encoding_config));
        if encoding_fix != case.expected_encoding() {
            failures.push(format!(
                "[{}] fix_encoding\n  expected: {:?}\n  actual:   {:?}",
                case.label,
                case.expected_encoding(),
                encoding_fix
            ));
        }

        // The text should still be fixable with an extra layer of badness.
        let from_extra_bad = fix_text(&extra_bad(&case.original), None);
        if from_extra_bad != case.fixed {
            failures.push(format!(
                "[{}] fix_text(extra_bad)\n  expected: {:?}\n  actual:   {:?}",
                case.label, case.fixed, from_extra_bad
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} passing example(s) did not match ftfy:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn test_known_failures() {
    // These are ftfy's strict xfail cases: the encoding fix is expected *not*
    // to reach the target. If plsfix unexpectedly reproduces it, that's an
    // "xpass" — surfaced here as a failure, the same way ftfy's
    // `@pytest.mark.xfail(strict=True)` would.
    let encoding_config = encoding_only_config();
    let mut unexpected_passes = Vec::new();

    for case in load_test_data().into_iter().filter(|c| c.expect == "fail") {
        let encoding_fix = fix_text(&case.original, Some(&encoding_config));
        if encoding_fix == case.expected_encoding() {
            unexpected_passes.push(format!(
                "[{}] now produces the target output {:?}",
                case.label,
                case.expected_encoding()
            ));
        }
    }

    assert!(
        unexpected_passes.is_empty(),
        "{} known-failure case(s) unexpectedly succeeded (update test-cases if intended):\n\n{}",
        unexpected_passes.len(),
        unexpected_passes.join("\n\n")
    );
}
