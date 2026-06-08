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
//!   isn't part of plsfix's public API. Instead, we use the same alternative
//!   ftfy itself asserts is equivalent: `fix_text` with every non-encoding
//!   fixer disabled.

use plsfix::{fix_text, TextFixerConfig};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::LazyLock;

static TEST_CASES_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/test-cases"));

static TEST_CASES: LazyLock<Vec<(PathBuf, Vec<TestCase>)>> = LazyLock::new(|| {
    let dir = &TEST_CASES_DIR;

    let test_case_files = fs::read_dir(dir.as_path())
        .expect("reading test-cases/")
        .map_while(Result::ok)
        .map(|entry| entry.path())
        .filter(|p| p.extension() == Some(OsStr::new("json")))
        .map(|p| p);

    let mut file_cases = Vec::new();
    for path in test_case_files {
        let file = File::open(&path).expect(&format!("opening test-cases: {:?}", path));
        let reader = BufReader::new(file);
        let cases: Vec<TestCase> =
            serde_json::from_reader(reader).expect(&format!("parsing test-cases: {:?}", path));
        assert!(!cases.is_empty(), "no test cases were read from {path:?}");
        file_cases.push((path, cases))
    }

    assert!(
        !file_cases.is_empty(),
        "no test cases were loaded from {dir:?}"
    );
    file_cases
});

#[derive(Deserialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct TestCase {
    label: String,
    original: String,
    fixed: String,
    #[serde(rename = "fixed-encoding")]
    fixed_encoding: Option<String>,
    expect: TestResult,
}

#[derive(Deserialize, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum TestResult {
    #[serde(rename(deserialize = "pass"))]
    Pass,
    #[serde(rename(deserialize = "fail"))]
    Fail,
}

#[derive(Deserialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct TestFailure(&'static str, TestCase, String);

#[derive(Deserialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct KnownFailureFailure {
    label: String,
    original: String,
    expected: String,
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

#[test]
fn test_json_pass_examples() {
    fn test_pass_case(case: &TestCase) -> Result<(), TestFailure> {
        // Make sure that we can fix the text as intended
        let result = fix_text(&case.original, None);
        if result != case.fixed {
            Err(TestFailure("fix_text", case.clone(), result))?;
        };

        // TODO: fix_and_explain -> apply_plan
        // Make sure that fix_and_explain outputs a plan that we can successfully
        // run to reproduce its result

        // TODO: fix_encoding_and_explain -> apply_plan
        // Do the same for fix_encoding_and_explain

        // Ask for the encoding fix a different way, by disabling all the other steps
        // in the config object
        let expected = case.fixed_encoding.as_deref().unwrap_or(&case.fixed);
        let result = fix_text(
            &case.original,
            Some(&TextFixerConfig {
                unescape_html: Some(false),
                remove_terminal_escapes: false,
                fix_character_width: false,
                fix_latin_ligatures: false,
                uncurl_quotes: false,
                fix_line_breaks: false,
                remove_control_chars: false,
                normalization: None,
                ..Default::default()
            }),
        );
        // Make sure we can decode the text as intended
        if result != expected {
            Err(TestFailure("fix_encoding", case.clone(), result))?
        }

        // Make sure we can decode as intended even with an extra layer of badness
        let extra_bad = encoding_rs::mem::decode_latin1(case.original.as_bytes());
        let result = fix_text(&extra_bad, None);
        if result != case.fixed {
            Err(TestFailure("extra_bad", case.clone(), result))?
        }

        Ok(())
    }

    let failures = TEST_CASES
        .iter()
        .map(|(path, cases)| {
            let case_failures = cases
                .into_iter()
                .filter(|c| c.expect == TestResult::Pass)
                .map(test_pass_case)
                .filter_map(|result| match result {
                    Ok(_) => None,
                    Err(e) => Some(e),
                })
                .collect::<Vec<_>>();
            (path.file_name().unwrap(), case_failures)
        })
        .collect::<BTreeMap<_, _>>();

    let sum_failures = failures
        .iter()
        .map(|(_, failures)| failures)
        .fold(0, |acc, failures| acc + failures.len());

    assert_eq!(
        sum_failures, 0,
        "{} passing example(s) did not match ftfy:\n{:#?}",
        sum_failures, failures
    );
}

/// Run an example from the data file that we believe will fail, due to
/// ftfy's heuristic being insufficient
#[test]
fn test_json_known_failures() {
    let encoding_config = encoding_only_config();

    let failures = TEST_CASES
        .iter()
        .map(|(path, cases)| {
            let case_failures = cases
                .into_iter()
                .filter(|c| c.expect == TestResult::Fail)
                .map(|case| {
                    let expected = case.fixed_encoding.as_deref().unwrap_or(&case.fixed);
                    let result = fix_text(&case.original, Some(&encoding_config));
                    if result == expected {
                        Err(KnownFailureFailure {
                            label: case.label.clone(),
                            original: case.original.clone(),
                            expected: expected.to_string(),
                        })?
                    };
                    Ok(())
                })
                .filter_map(|x: Result<(), KnownFailureFailure>| match x {
                    Ok(_) => None,
                    Err(e) => Some(e),
                })
                .collect::<Vec<_>>();
            (path, case_failures)
        })
        .collect::<Vec<_>>();

    let sum_failures = failures
        .iter()
        .map(|(_, failures)| failures)
        .fold(0, |acc, failures| acc + failures.len());

    assert_eq!(
        sum_failures, 0,
        "{} known failure(s) unexpectedly passing:\n{:#?}",
        sum_failures, failures
    );
}
