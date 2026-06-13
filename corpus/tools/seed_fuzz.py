#!/usr/bin/env python3
"""Seed the cargo-fuzz corpora from the verified mojibake corpus.

Each entry's ``mojibake`` string is written (UTF-8) as a seed file into
every string-input fuzz target's corpus directory, so the fuzzer starts
from realistic encoding-corruption shapes instead of random bytes.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
ENTRIES = ROOT / "corpus" / "entries"
FUZZ_CORPUS = ROOT / "fuzz" / "corpus"

# Targets whose input is the raw bytes of a (UTF-8) string.
STRING_TARGETS = [
    "fix_text_no_panic",
    "fix_and_explain_no_panic",
    "idempotent",
    "roundtrip_mojibake",
]


def main() -> None:
    seeds = []
    for path in sorted(ENTRIES.glob("*.json")):
        for entry in json.loads(path.read_text(encoding="utf-8")):
            seeds.append(entry["mojibake"].encode("utf-8"))

    for target in STRING_TARGETS:
        out = FUZZ_CORPUS / target
        out.mkdir(parents=True, exist_ok=True)
        for blob in seeds:
            (out / hashlib.sha1(blob).hexdigest()[:16]).write_bytes(blob)
        print(f"seeded {len(seeds)} files into fuzz/corpus/{target}/")


if __name__ == "__main__":
    main()
