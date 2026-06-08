use std::panic;

use ::plsfix::{ExplainedText, ExplanationStep, Normalization, TextFixerConfig};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

// ftfy's tri-state `unescape_html`: "auto", True, or False.
#[derive(Debug, Clone, FromPyObject)]
enum UnescapeHtml {
    Bool(bool),
    Mode(String),
}

// Accepted as a function argument, so it needs the `FromPyObject` derive
// (opt-in as of pyo3 0.28).
#[pyclass(name = "TextFixerConfig", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyTextFixerConfig {
    pub unescape_html: Option<bool>,
    pub remove_terminal_escapes: bool,
    pub fix_encoding: bool,
    pub restore_byte_a0: bool,
    pub replace_lossy_sequences: bool,
    pub decode_inconsistent_utf8: bool,
    pub fix_c1_controls: bool,
    pub fix_latin_ligatures: bool,
    pub fix_character_width: bool,
    pub uncurl_quotes: bool,
    pub fix_line_breaks: bool,
    pub fix_surrogates: bool,
    pub remove_control_chars: bool,
    pub normalization: Option<Normalization>,
    pub max_decode_length: i32,
    pub explain: bool,
}

#[pymethods]
impl PyTextFixerConfig {
    // Matches ftfy's field names, order, and defaults so ftfy code constructs
    // it unchanged.
    #[new]
    #[pyo3(signature = (
        unescape_html=None,
        remove_terminal_escapes=true,
        fix_encoding=true,
        restore_byte_a0=true,
        replace_lossy_sequences=true,
        decode_inconsistent_utf8=true,
        fix_c1_controls=true,
        fix_latin_ligatures=true,
        fix_character_width=true,
        uncurl_quotes=true,
        fix_line_breaks=true,
        fix_surrogates=true,
        remove_control_chars=true,
        normalization="NFC".to_string(),
        max_decode_length=1_000_000,
        explain=true,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        unescape_html: Option<UnescapeHtml>,
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
        fix_surrogates: bool,
        remove_control_chars: bool,
        normalization: Option<String>,
        max_decode_length: i32,
        explain: bool,
    ) -> PyResult<Self> {
        // "auto" / not-passed -> None; explicit True/False -> Some(bool).
        let unescape_html = match unescape_html {
            None => None,
            Some(UnescapeHtml::Bool(b)) => Some(b),
            Some(UnescapeHtml::Mode(mode)) if mode == "auto" => None,
            Some(UnescapeHtml::Mode(other)) => {
                return Err(PyValueError::new_err(format!(
                    "invalid unescape_html {other:?}, expected True, False, or \"auto\""
                )))
            }
        };
        let normalization = match normalization {
            None => None,
            Some(name) => Some(match name.as_str() {
                "NFC" => Normalization::NFC,
                "NFKC" => Normalization::NFKC,
                "NFD" => Normalization::NFD,
                "NFKD" => Normalization::NFKD,
                other => {
                    return Err(PyValueError::new_err(format!(
                        "invalid normalization {other:?}, expected NFC/NFKC/NFD/NFKD or None"
                    )))
                }
            }),
        };
        Ok(Self {
            unescape_html,
            remove_terminal_escapes,
            fix_encoding,
            restore_byte_a0,
            replace_lossy_sequences,
            decode_inconsistent_utf8,
            fix_c1_controls,
            fix_latin_ligatures,
            fix_character_width,
            uncurl_quotes,
            fix_line_breaks,
            fix_surrogates,
            remove_control_chars,
            normalization,
            max_decode_length,
            explain,
        })
    }
}

// Output-only, never extracted from Python, so skip the `FromPyObject` derive.
#[pyclass(name = "ExplanationStep", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyExplanationStep {
    pub transformation: String,
}

#[pymethods]
impl PyExplanationStep {
    #[getter]
    fn transformation(&self) -> String {
        self.transformation.clone()
    }
}

// Output-only, never extracted from Python, so skip the `FromPyObject` derive.
#[pyclass(name = "ExplainedText", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyExplainedText {
    pub text: String,
    pub steps: Option<Vec<PyExplanationStep>>,
}

#[pymethods]
impl PyExplainedText {
    #[getter]
    fn text(&self) -> String {
        self.text.clone()
    }

    #[getter]
    fn steps(&self) -> Option<Vec<PyExplanationStep>> {
        self.steps.clone()
    }
}

impl From<PyTextFixerConfig> for TextFixerConfig {
    fn from(config: PyTextFixerConfig) -> Self {
        TextFixerConfig {
            unescape_html: config.unescape_html,
            remove_terminal_escapes: config.remove_terminal_escapes,
            fix_encoding: config.fix_encoding,
            restore_byte_a0: config.restore_byte_a0,
            replace_lossy_sequences: config.replace_lossy_sequences,
            decode_inconsistent_utf8: config.decode_inconsistent_utf8,
            fix_c1_controls: config.fix_c1_controls,
            fix_latin_ligatures: config.fix_latin_ligatures,
            fix_character_width: config.fix_character_width,
            uncurl_quotes: config.uncurl_quotes,
            fix_line_breaks: config.fix_line_breaks,
            remove_control_chars: config.remove_control_chars,
            normalization: config.normalization,
            max_decode_length: config.max_decode_length,
        }
    }
}

impl From<ExplanationStep> for PyExplanationStep {
    fn from(step: ExplanationStep) -> Self {
        PyExplanationStep {
            transformation: step.transformation,
        }
    }
}

impl From<ExplainedText> for PyExplainedText {
    fn from(text: ExplainedText) -> Self {
        PyExplainedText {
            text: text.text,
            steps: match text.steps {
                Some(steps) => Some(steps.into_iter().map(|step| step.into()).collect()),
                None => None,
            },
        }
    }
}

#[pyfunction]
#[pyo3(signature = (text, config=None))]
pub fn fix_text(text: &str, config: Option<PyTextFixerConfig>) -> String {
    let config = config.map(PyTextFixerConfig::into);
    let config_ref = config.as_ref();

    let result = panic::catch_unwind(|| ::plsfix::fix_text(text, config_ref));

    match result {
        Ok(result) => result,
        Err(_) => text.to_string(),
    }
}

#[pyfunction]
#[pyo3(signature = (text, config=None))]
pub fn fix_and_explain(text: &str, config: Option<PyTextFixerConfig>) -> PyExplainedText {
    // ftfy controls explanations via `config.explain` (default True), not a
    // separate argument.
    let explain = config.as_ref().map_or(true, |c| c.explain);
    let config = config.map(PyTextFixerConfig::into);
    let config_ref = config.as_ref();

    let result = panic::catch_unwind(|| ::plsfix::fix_and_explain(text, explain, config_ref));

    match result {
        Ok(result) => result.into(),
        Err(_) => PyExplainedText {
            text: text.to_string(),
            steps: None,
        },
    }
}

#[pymodule]
fn plsfix(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTextFixerConfig>()?;
    m.add_class::<PyExplainedText>()?;
    m.add_class::<PyExplanationStep>()?;
    m.add_function(wrap_pyfunction!(fix_text, m)?)?;
    m.add_function(wrap_pyfunction!(fix_and_explain, m)?)?;
    Ok(())
}
