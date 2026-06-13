#!/usr/bin/env python3
"""Mechanically verify the mojibake corpus.

For every entry in corpus/entries/*.json:

1. If the entry has a ``chain``, replay it on ``intended`` and check the
   result equals ``mojibake`` exactly. A mismatch demotes the entry to
   ``verified: false`` — the sample may be mis-transcribed or
   hallucinated (see corpus README).
2. Run reference ftfy on ``mojibake`` and record what it produces and
   whether it recovers ``intended`` (``ftfy_output`` / ``ftfy_recovers``
   fields, refreshed in place).

Usage:
    verify_corpus.py [--write] [files...]

Without --write, reports drift but modifies nothing. With --write,
updates the verification fields in the JSON files.
"""

from __future__ import annotations

import argparse
import json
import sys
import unicodedata
from pathlib import Path

import ftfy

sys.path.insert(0, str(Path(__file__).parent))
from transforms import ChainError, apply_chain  # noqa: E402

ENTRIES_DIR = Path(__file__).resolve().parent.parent / "entries"


def verify_entry(entry: dict) -> list[str]:
    """Update verification fields in place; return list of problems."""
    problems = []
    mojibake = entry["mojibake"]
    intended = entry.get("intended")

    chain = entry.get("chain")
    seeds_raw_bytes = bool(chain) and "raw_bytes_hex" in chain[0]
    if chain and (intended is not None or seeds_raw_bytes):
        # A raw-bytes seed makes the chain self-contained (no intended needed).
        try:
            reproduced = apply_chain(intended, chain)
            entry["verified"] = reproduced == mojibake
            if not entry["verified"]:
                problems.append(
                    f"chain does not reproduce mojibake:\n"
                    f"    chain output: {reproduced!r}\n"
                    f"    corpus says:  {mojibake!r}"
                )
        except ChainError as exc:
            entry["verified"] = False
            problems.append(f"chain failed: {exc}")
    elif chain:
        problems.append("has chain but no intended text")
    else:
        # No chain: provenance-only entry. Never marked verified.
        entry["verified"] = False

    output = ftfy.fix_text(mojibake)
    entry["ftfy_output"] = output
    if intended is not None:
        # NFC-compare: ftfy normalizes to NFC by default
        entry["ftfy_recovers"] = unicodedata.normalize("NFC", intended) == output
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="*", type=Path)
    parser.add_argument("--write", action="store_true", help="update JSON files in place")
    args = parser.parse_args()

    files = args.files or sorted(ENTRIES_DIR.glob("*.json"))
    total = verified = recovered = problem_count = 0
    for path in files:
        entries = json.loads(path.read_text(encoding="utf-8"))
        for entry in entries:
            total += 1
            problems = verify_entry(entry)
            verified += bool(entry.get("verified"))
            recovered += bool(entry.get("ftfy_recovers"))
            for p in problems:
                problem_count += 1
                print(f"[{path.name}] {entry['id']}: {p}")
        if args.write:
            path.write_text(
                json.dumps(entries, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )

    print(
        f"\n{total} entries: {verified} chain-verified, "
        f"{recovered} recovered by ftfy {ftfy.__version__}, "
        f"{problem_count} problems"
    )
    return 1 if (problem_count and not args.write) else 0


if __name__ == "__main__":
    sys.exit(main())
