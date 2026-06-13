# Differential test harness: plsfix vs python-ftfy

Compares this repo's `plsfix` against the reference `python-ftfy` across
every observable behavior, over the mojibake corpus in `../corpus` plus a
set of programmatic edge cases.

## What it compares

| Dimension | How | Where |
|---|---|---|
| **Decoded output** | `fix_text(x)` byte-identical? If not, which library recovers the corpus entry's known `intended` text? | `compare.py` |
| **Explanation** | `fix_and_explain(x)` steps, normalized to a common token form; compared exactly and at the level of named transforms (the "apply subsequence") | `compare.py` + `explain.py` |
| **Idempotence** | `fix_text(fix_text(x)) == fix_text(x)` for each library | `compare.py` |
| **Explain/output consistency** | does each library's `fix_and_explain(...).text` equal its own `fix_text(...)`? | `compare.py` |
| **Robustness** | does either raise or panic? (PyO3 panics surface as `BaseException`) | `compare.py` |
| **Performance** | per-call latency and speedup, corpus + size-bucketed synthetics | `benchmark.py` |
| **Fuzzed inputs** | random and mutation-based inputs (see `../fuzz`) | property: plsfix must never panic and must agree with ftfy |

## Running

```bash
# one-time: a venv with the pinned reference ftfy and a built plsfix
uv venv .venv && . .venv/bin/activate
uv pip install -r ../corpus/tools/requirements.txt maturin
(cd ../python && maturin develop --release)

python harness/compare.py        # differential correctness/behavior
python harness/benchmark.py      # performance
```

`compare.py` writes `report.json` (every case, every field) and prints a
bucketed summary. `benchmark.py` writes `benchmark.json`.

## Result buckets (`compare.py`)

| Bucket | Meaning | Fails run? |
|---|---|---|
| `MATCH` | identical output **and** explanation | no |
| `OUTPUT_MATCH_EXPLAIN_TRANSCODE_OMITTED` | same output; plsfix didn't record the encode/decode plumbing ftfy did | no (documented) |
| `OUTPUT_MATCH_EXPLAIN_DIFFERS` | same output; the **named** transforms recorded differ | no (documented) |
| `OUTPUT_DIFFERS_BOTH_RECOVER` | outputs differ but both equal `NFC(intended)` — a normalization nit | no |
| `OUTPUT_DIFFERS_PLSFIX_BETTER` | outputs differ; plsfix recovers intended, ftfy doesn't | no |
| `OUTPUT_DIFFERS_NEITHER_ORACLE` | outputs differ; no oracle (or both wrong) — needs a human look | no |
| `PLSFIX_REJECTS_SURROGATE` | input had a lone surrogate; plsfix rejects it at the PyO3 boundary (Rust strings can't hold surrogates — README #178) | no (documented) |
| `OUTPUT_DIFFERS_FTFY_BETTER` | outputs differ; ftfy recovers intended, plsfix doesn't | **yes — regression** |
| `ERROR` | a library raised/panicked on an input it should handle | **yes** |

Pass `--strict` to also fail on the documented-difference buckets (useful
when chasing exact parity).

## Standing findings (ftfy 6.3.1 vs plsfix 0.1.x)

These are differences the harness surfaces today. None is an output
regression; all are either documented or benign:

1. **Explanation shape (435 cases).** plsfix's `fix_and_explain` often
   omits the `encode`/`decode` transcode steps it actually performed,
   reporting only named transforms (or nothing). ftfy reports the full
   plan. This is the documented "Explanation shape" difference and is why
   plsfix can't round-trip a plan via `apply_plan`.
2. **`decode_inconsistent_utf8` vs `fix_c1_controls` accounting (98
   cases).** When outputs agree, the two libraries still attribute the
   work to different named steps: ftfy records `decode_inconsistent_utf8`
   (43×) where plsfix folds it into its decode; plsfix records
   `fix_c1_controls` (84×) where ftfy applies it without a named step.
3. **`restore_byte_a0` on ambiguous input (3 edge cases).** On degenerate
   runs like `"Ã Ã "` (a `Ã` followed by a literal space, not the NBSP
   that real mojibake would carry), plsfix's `restore_byte_a0` consumes a
   trailing space that ftfy preserves, so plsfix returns `"à à"` where
   ftfy returns `"à à "`. The input is inherently ambiguous (there is no
   "correct" answer), but it is a reproducible divergence where plsfix
   loses a character. Properly-formed mojibake (`"Ã\xa0 Ã\xa0 "`) is
   recovered identically by both.
4. **Lone surrogates (3 edge cases).** plsfix raises at the binding rather
   than returning a replacement-character string; ftfy returns `�`. By
   design — see README.
5. **Performance.** plsfix is ~9–13× faster on mojibake-heavy input and
   ~2.5–5× faster on clean text (the encoding-repair fast path is where
   the Rust core wins most).

## Files

```
harness/
  compare.py      # differential runner -> report.json + summary
  benchmark.py    # performance runner  -> benchmark.json + summary
  explain.py      # explanation-step normalization & comparison
  edge_cases.py   # programmatic edge cases (surrogates, scale, boundaries)
  README.md
```
