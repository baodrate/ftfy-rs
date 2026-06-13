#!/usr/bin/env python3
"""Performance comparison: plsfix vs python-ftfy.

Times ``fix_text`` (and ``fix_and_explain``) for both libraries over the
corpus plus a few size-bucketed synthetic inputs, and reports per-call
latency and the speedup ratio. This measures the user-facing Python API
of both packages (plsfix's Rust core is reached through PyO3, so the FFI
boundary is included — that is the comparison a plsfix user actually
experiences).

Usage:
    benchmark.py [--iterations N] [--report PATH] [--corpus-only]
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
from pathlib import Path

import ftfy
import plsfix

CORPUS = Path(__file__).resolve().parent.parent / "corpus" / "entries"


def load_inputs() -> list[tuple[str, str]]:
    """(category, text) pairs to benchmark."""
    inputs = []
    for path in sorted(CORPUS.glob("*.json")):
        cat = path.stem
        for entry in json.loads(path.read_text(encoding="utf-8")):
            inputs.append((cat, entry["mojibake"]))
    return inputs


def synthetic_sizes() -> list[tuple[str, str]]:
    base = "The Mona Lisa doesnÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢t have eyebrows. "
    clean = "The quick brown fox jumps over the lazy dog. "
    return [
        ("synth-tiny-mojibake", "cafÃ©"),
        ("synth-small-mojibake", base),
        ("synth-medium-mojibake", base * 50),
        ("synth-large-mojibake", base * 1000),
        ("synth-small-clean", clean * 10),
        ("synth-large-clean", clean * 5000),
    ]


def time_fn(fn, text, iterations) -> float:
    """Best-of timing in microseconds per call (min is the least-noisy estimate)."""
    samples = []
    for _ in range(iterations):
        t0 = time.perf_counter()
        fn(text)
        samples.append((time.perf_counter() - t0) * 1e6)
    return min(samples)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--iterations", type=int, default=5)
    ap.add_argument("--report", type=Path,
                    default=Path(__file__).parent / "benchmark.json")
    ap.add_argument("--corpus-only", action="store_true")
    args = ap.parse_args()

    inputs = load_inputs()
    if not args.corpus_only:
        inputs += synthetic_sizes()

    rows = []
    cat_totals: dict[str, list[float]] = {}
    for cat, text in inputs:
        # Skip inputs Rust can't accept (lone surrogates) for timing parity.
        try:
            text.encode("utf-8")
        except UnicodeEncodeError:
            continue
        f_us = time_fn(ftfy.fix_text, text, args.iterations)
        p_us = time_fn(plsfix.fix_text, text, args.iterations)
        rows.append({
            "category": cat, "bytes": len(text.encode("utf-8")),
            "ftfy_us": f_us, "plsfix_us": p_us,
            "speedup": f_us / p_us if p_us else None,
        })
        cat_totals.setdefault(cat, []).append(f_us / p_us if p_us else 0.0)

    total_ftfy = sum(r["ftfy_us"] for r in rows)
    total_plsfix = sum(r["plsfix_us"] for r in rows)
    speedups = [r["speedup"] for r in rows if r["speedup"]]

    report = {
        "ftfy_version": ftfy.__version__,
        "plsfix_version": getattr(plsfix, "__version__", "?"),
        "iterations": args.iterations,
        "n_inputs": len(rows),
        "total_ftfy_us": total_ftfy,
        "total_plsfix_us": total_plsfix,
        "aggregate_speedup": total_ftfy / total_plsfix if total_plsfix else None,
        "median_speedup": statistics.median(speedups) if speedups else None,
        "rows": rows,
    }
    args.report.write_text(json.dumps(report, indent=2), encoding="utf-8")

    print(f"\nplsfix {report['plsfix_version']} vs ftfy {report['ftfy_version']}"
          f"  ({args.iterations} iterations, best-of)\n")
    print(f"  total wall time: ftfy {total_ftfy/1e6:.3f}s  "
          f"plsfix {total_plsfix/1e6:.3f}s")
    print(f"  aggregate speedup: {report['aggregate_speedup']:.2f}x")
    print(f"  median per-input speedup: {report['median_speedup']:.2f}x\n")

    print("  by category (median speedup, n inputs):")
    for cat, sp in sorted(cat_totals.items()):
        med = statistics.median([s for s in sp if s]) if any(sp) else 0
        print(f"    {med:6.2f}x  {cat}  (n={len(sp)})")

    print("\n  synthetic size scaling:")
    for r in rows:
        if r["category"].startswith("synth"):
            print(f"    {r['speedup']:6.2f}x  {r['category']:24s} "
                  f"{r['bytes']:>9d} B  ftfy {r['ftfy_us']:>10.1f}us  "
                  f"plsfix {r['plsfix_us']:>10.1f}us")

    print(f"\n  full report: benchmark.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
