#!/usr/bin/env python3
"""Build corpus/entries/ai-hallucinated.json.

The cautionary class of the corpus. These mojibake strings are
*mechanically valid* — they round-trip through a real codec chain — but
they have **no genuine encoding-error provenance**: they were produced by
a large language model imitating what mojibake-laden text "looks like",
not by a real text being mis-decoded.

The canonical example is from Robyn Speer's blog post "Never Lose a Dead
End" (2024-10-31, https://posts.arborelia.net/never-lose-a-dead-end/).
While hunting the OSCAR web-crawl for a *real* sample of Windows-1257
(Baltic) mojibake to test a new ftfy heuristic, she found a Lithuanian
page full of convincing mojibake — then realized the whole page was
LLM-generated. The model "generated fake encoding errors because that's
what it believed Lithuanian looks like." There was never a real encoding
error.

Why this matters for the corpus design: a round-trip check proves a
sample is mechanically real, but it CANNOT distinguish a genuine
encoding-error artifact from AI-generated text that imitates one — the
fake mojibake here decodes just as cleanly as the real thing. Only
*provenance* separates them. That is precisely why every corpus entry is
required to carry a citation or a detailed justification, and why these
entries are quarantined under their own provenance type with
``real_encoding_error: false``.

These entries are kept (a) because the user explicitly asked to watch for
AI-hallucinated mojibake, and (b) as a live test that the fixers behave
sanely on LLM-imitated mojibake. Their ``intended`` text is the form ftfy
*would* decode them to; it is itself LLM-generated and not meaningful
Lithuanian (Google Translate rendered the lead sentence as the vague
"Anyway, it's interesting how you imagine.").
"""

from __future__ import annotations

import json
import sys
import unicodedata
from pathlib import Path

import ftfy

sys.path.insert(0, str(Path(__file__).parent))
from transforms import apply_chain  # noqa: E402

OUT = Path(__file__).resolve().parent.parent / "entries" / "ai-hallucinated.json"

POST = "https://posts.arborelia.net/never-lose-a-dead-end/ (Robyn Speer, 2024-10-31)"
ORIGIN = ("found in the OSCAR web-crawl corpus on the now-parked domain ratu.lt; "
          "the source page was entirely LLM-generated 'AI slop'")

D1257 = {"decode": "sloppy-windows-1257"}
U8 = {"encode": "utf-8"}

# (id, label, intended-as-ftfy-would-decode, chain, notes)
ENTRIES = [
    ("ai-deadend-sentence-flat",
     "'Å iaip ÄÆdomu…' — the lead example from 'Never Lose a Dead End' (NBSP flattened)",
     "Šiaip įdomu, kaip įsivaizduoji.",
     [U8, D1257, {"transform": "nbsp_to_space"}],
     "The exact sentence rspeer found. UTF-8 decoded as Windows-1257; the "
     "first word's NBSP (Å<NBSP>iaip) was flattened to a space. ftfy 6.3.0 "
     "decoded it in the post, but ftfy 6.3.1 and plsfix both now DECLINE it "
     "(the badness gate rejects it) — they agree. Google-translates to the "
     "vague 'Anyway, it's interesting how you imagine.'"),
    ("ai-deadend-sentence-nbsp",
     "'Å\\xa0iaip ÄÆdomu…' — same sentence with the NBSP intact",
     "Šiaip įdomu, kaip įsivaizduoji.",
     [U8, D1257],
     "The pre-flattening form (Å + U+00A0). Included to test the NBSP path "
     "the post highlights."),
    ("ai-deadend-vaikysteje",
     "'vaikystÄ—je' — hallucinated word containing a real em dash",
     "vaikystėje",
     [U8, D1257],
     "ė = UTF-8 C4 97; Windows-1257 byte 0x97 is U+2014 EM DASH, so the "
     "mojibake literally contains an em dash (the post notes it sits right "
     "after a correctly-used em dash). 'vaikystėje' ~ 'in childhood'."),
    ("ai-deadend-kokybes",
     "'kokybÄ—s' — hallucinated 'kokybės' (quality)",
     "kokybės",
     [U8, D1257],
     "Another ė→Ä— word the post cites as an LLM-believed Lithuanian word."),
    ("ai-deadend-idomu",
     "'ÄÆdomu' — hallucinated 'įdomu' (interesting)",
     "įdomu",
     [U8, D1257],
     "į = UTF-8 C4 AF; Windows-1257 0xAF is U+00C6 Æ, hence ÄÆ. The post "
     "lists 'ÄÆdomu' among words the LLM thought were real Lithuanian."),
]


def main() -> None:
    out = []
    for id_, label, intended, chain, notes in ENTRIES:
        mojibake = apply_chain(intended, chain)
        assert mojibake != intended, f"{id_}: chain is a no-op"
        output = ftfy.fix_text(mojibake)
        out.append({
            "id": id_, "label": label, "lang": "lt",
            "intended": intended, "mojibake": mojibake, "chain": chain,
            "provenance": {
                "type": "ai-hallucinated",
                "source": POST,
                "origin": ORIGIN,
                "real_encoding_error": False,
                "notes": notes,
            },
            "verified": True,            # round-trips: mechanically valid mojibake
            "ftfy_output": output,
            "ftfy_recovers": unicodedata.normalize("NFC", intended) == output,
        })
    OUT.write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n",
                   encoding="utf-8")
    rec = sum(bool(e["ftfy_recovers"]) for e in out)
    print(f"wrote {len(out)} entries to {OUT}; ftfy recovers {rec}/{len(out)} "
          f"(low is expected — these are AI-imitated, and current heuristics decline)")


if __name__ == "__main__":
    main()
