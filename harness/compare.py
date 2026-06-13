#!/usr/bin/env python3
"""Differential test harness: plsfix (this repo) vs reference python-ftfy.

For every corpus entry (and a set of programmatic edge cases) this runs
both libraries and compares every observable:

* **output** — does ``fix_text`` produce the same string? When they
  differ, which one (if either) recovers the entry's known ``intended``
  text?
* **explain** — do the explanation steps agree, both exactly and at the
  level of named transforms (the apply-subsequence)?
* **idempotence** — is ``fix_text(fix_text(x)) == fix_text(x)`` for each?
* **robustness** — does either raise / panic on an input?

It writes a machine-readable ``report.json`` and prints a human summary.
Exit code is non-zero if any *regression* bucket is non-empty (output
differs and ftfy recovered the intended text but plsfix did not, or
either library errored). Buckets that reflect documented, intentional
differences (see README "Differences") do not fail the run unless
``--strict`` is given.

Usage:
    compare.py [--strict] [--report PATH] [--limit N] [--quiet]
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import traceback
import unicodedata
from pathlib import Path

import ftfy
import plsfix

sys.path.insert(0, str(Path(__file__).parent))
import explain as explain_mod  # noqa: E402
from edge_cases import edge_cases  # noqa: E402

CORPUS = Path(__file__).resolve().parent.parent / "corpus" / "entries"


def load_corpus() -> list[dict]:
    cases = []
    for path in sorted(CORPUS.glob("*.json")):
        for entry in json.loads(path.read_text(encoding="utf-8")):
            entry["_file"] = path.name
            cases.append(entry)
    return cases


def safe(fn, *args):
    """Run fn; return (ok, value_or_None, error_repr_or_None)."""
    try:
        return True, fn(*args), None
    except BaseException as exc:  # PyO3 panics surface as BaseException
        return False, None, f"{type(exc).__name__}: {exc}"


def run_one(entry: dict) -> dict:
    moji = entry["mojibake"]
    intended = entry.get("intended")
    nfc_intended = unicodedata.normalize("NFC", intended) if intended else None

    f_ok, f_out, f_err = safe(ftfy.fix_text, moji)
    p_ok, p_out, p_err = safe(plsfix.fix_text, moji)

    result = {
        "id": entry["id"], "file": entry["_file"], "lang": entry.get("lang"),
        "verified": entry.get("verified"),
        "input": moji, "intended": intended,
        "ftfy_ok": f_ok, "plsfix_ok": p_ok,
        "ftfy_error": f_err, "plsfix_error": p_err,
        "ftfy_output": f_out, "plsfix_output": p_out,
        "output_match": f_ok and p_ok and f_out == p_out,
    }

    # Which library recovered the intended text?
    if nfc_intended is not None:
        result["ftfy_recovers"] = f_ok and f_out == nfc_intended
        result["plsfix_recovers"] = p_ok and p_out == nfc_intended
    else:
        result["ftfy_recovers"] = result["plsfix_recovers"] = None

    # Idempotence of each fixer.
    if f_ok:
        ok2, f_out2, _ = safe(ftfy.fix_text, f_out)
        result["ftfy_idempotent"] = ok2 and f_out2 == f_out
    if p_ok:
        ok2, p_out2, _ = safe(plsfix.fix_text, p_out)
        result["plsfix_idempotent"] = ok2 and p_out2 == p_out

    # Explanation comparison.
    fe_ok, fe, _ = safe(ftfy.fix_and_explain, moji)
    pe_ok, pe, _ = safe(plsfix.fix_and_explain, moji)
    if fe_ok and pe_ok:
        cmp = explain_mod.compare(getattr(fe, "explanation", None),
                                  getattr(pe, "steps", None))
        result["explain"] = cmp
        # Sanity: explain text should equal fix_text output for each lib.
        result["ftfy_explain_text_consistent"] = fe.text == f_out
        result["plsfix_explain_text_consistent"] = pe.text == p_out

    result["bucket"] = classify(result)
    return result


def _is_surrogate_error(err: str | None) -> bool:
    return bool(err) and "surrogate" in err.lower()


def classify(r: dict) -> str:
    if not r["plsfix_ok"] and _is_surrogate_error(r["plsfix_error"]):
        # Rust strings can't hold lone surrogates; the PyO3 boundary rejects
        # them. Documented in README (#178) — an expected difference, not a bug.
        return "PLSFIX_REJECTS_SURROGATE"
    if not r["ftfy_ok"] or not r["plsfix_ok"]:
        return "ERROR"
    if r["output_match"]:
        # Outputs agree; is the explanation also in sync?
        ex = r.get("explain")
        if ex and not ex["apply_match"]:
            return "OUTPUT_MATCH_EXPLAIN_DIFFERS"
        if ex and ex["plsfix_omits_transcode"]:
            return "OUTPUT_MATCH_EXPLAIN_TRANSCODE_OMITTED"
        return "MATCH"
    # Outputs differ.
    fr, pr = r.get("ftfy_recovers"), r.get("plsfix_recovers")
    if fr and not pr:
        return "OUTPUT_DIFFERS_FTFY_BETTER"   # potential regression
    if pr and not fr:
        return "OUTPUT_DIFFERS_PLSFIX_BETTER"
    if fr and pr:
        return "OUTPUT_DIFFERS_BOTH_RECOVER"  # both right, normalization nit
    return "OUTPUT_DIFFERS_NEITHER_ORACLE"    # no oracle / both off


# Buckets that count as failures for a non-strict run.
REGRESSION_BUCKETS = {"ERROR", "OUTPUT_DIFFERS_FTFY_BETTER"}
# Buckets that are documented, intentional differences (fail only with --strict).
KNOWN_DIFF_BUCKETS = {
    "OUTPUT_MATCH_EXPLAIN_DIFFERS",
    "OUTPUT_MATCH_EXPLAIN_TRANSCODE_OMITTED",
    "OUTPUT_DIFFERS_BOTH_RECOVER",
    "OUTPUT_DIFFERS_NEITHER_ORACLE",
    "OUTPUT_DIFFERS_PLSFIX_BETTER",
    "PLSFIX_REJECTS_SURROGATE",
}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--strict", action="store_true",
                    help="also fail on documented/intentional differences")
    ap.add_argument("--report", type=Path,
                    default=Path(__file__).parent / "report.json")
    ap.add_argument("--limit", type=int, default=0, help="cap number of cases")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    cases = load_corpus() + edge_cases()
    if args.limit:
        cases = cases[: args.limit]

    results = []
    for entry in cases:
        try:
            results.append(run_one(entry))
        except Exception:
            results.append({
                "id": entry.get("id", "?"), "bucket": "HARNESS_ERROR",
                "traceback": traceback.format_exc(),
            })

    buckets: dict[str, list[str]] = {}
    for r in results:
        buckets.setdefault(r["bucket"], []).append(r["id"])

    report = {
        "ftfy_version": ftfy.__version__,
        "plsfix_version": getattr(plsfix, "__version__", "?"),
        "total": len(results),
        "bucket_counts": {k: len(v) for k, v in sorted(buckets.items())},
        "results": results,
    }
    # ensure_ascii=True so lone surrogates from edge cases survive as \udcXX.
    args.report.write_text(json.dumps(report, ensure_ascii=True, indent=2),
                           encoding="utf-8")

    if not args.quiet:
        print_summary(report, buckets, results)

    fail = set(REGRESSION_BUCKETS)
    if args.strict:
        fail |= KNOWN_DIFF_BUCKETS | {"HARNESS_ERROR"}
    else:
        fail |= {"HARNESS_ERROR"}
    n_fail = sum(len(buckets.get(b, [])) for b in fail)
    return 1 if n_fail else 0


def print_summary(report, buckets, results):
    print(f"\nplsfix {report['plsfix_version']} vs ftfy {report['ftfy_version']}"
          f" — {report['total']} cases\n")
    labels = {
        "MATCH": "identical output & explain",
        "OUTPUT_MATCH_EXPLAIN_TRANSCODE_OMITTED":
            "same output; plsfix omits encode/decode steps (documented)",
        "OUTPUT_MATCH_EXPLAIN_DIFFERS":
            "same output; named transforms differ",
        "OUTPUT_DIFFERS_BOTH_RECOVER":
            "output differs but both recover intended (normalization)",
        "OUTPUT_DIFFERS_PLSFIX_BETTER": "output differs; plsfix recovers, ftfy doesn't",
        "OUTPUT_DIFFERS_FTFY_BETTER":
            "output differs; ftfy recovers, plsfix doesn't  <-- REGRESSION",
        "OUTPUT_DIFFERS_NEITHER_ORACLE": "output differs; no oracle to judge",
        "ERROR": "a library raised/panicked  <-- REGRESSION",
        "HARNESS_ERROR": "harness bug",
    }
    for bucket, ids in sorted(buckets.items(), key=lambda kv: -len(kv[1])):
        print(f"  {len(ids):4d}  {bucket}")
        print(f"        {labels.get(bucket, '')}")
        if bucket in REGRESSION_BUCKETS and ids:
            for rid in ids[:10]:
                r = next(x for x in results if x["id"] == rid)
                print(f"          - {rid}: ftfy={r.get('ftfy_output')!r} "
                      f"plsfix={r.get('plsfix_output')!r} err={r.get('plsfix_error')}")
    # Transform-level breakdown of explanation disagreements: which named
    # transforms does one library record that the other doesn't?
    from collections import Counter
    only_ftfy, only_plsfix = Counter(), Counter()
    for r in results:
        ex = r.get("explain")
        if not ex or ex["apply_match"]:
            continue
        fa, pa = set(ex["ftfy_applies"]), set(ex["plsfix_applies"])
        for t in fa - pa:
            only_ftfy[t] += 1
        for t in pa - fa:
            only_plsfix[t] += 1
    if only_ftfy or only_plsfix:
        print("\n  explain disagreements by transform:")
        for t, n in only_ftfy.most_common():
            print(f"    {n:4d}  recorded by ftfy only:   {t}")
        for t, n in only_plsfix.most_common():
            print(f"    {n:4d}  recorded by plsfix only: {t}")

    # Idempotence / consistency tallies.
    non_idem_p = [r["id"] for r in results if r.get("plsfix_idempotent") is False]
    non_idem_f = [r["id"] for r in results if r.get("ftfy_idempotent") is False]
    inconsistent = [r["id"] for r in results
                    if r.get("plsfix_explain_text_consistent") is False]
    print(f"\n  non-idempotent: plsfix={len(non_idem_p)} ftfy={len(non_idem_f)}")
    print(f"  plsfix explain-text != fix_text output: {len(inconsistent)}")
    if inconsistent[:5]:
        print(f"    e.g. {inconsistent[:5]}")
    print(f"\n  full report: {report and 'report.json'}")


if __name__ == "__main__":
    sys.exit(main())
