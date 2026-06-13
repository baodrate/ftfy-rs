#!/usr/bin/env python3
"""Build corpus/entries/classic-documented.json.

These are *documented* mojibake scenarios: each one is attested in a
citable source (ftfy docs/README, rspeer's notes, Wikipedia's Mojibake
article, well-known bug-tracker lore). To keep hallucination risk at
zero, the garbled strings are not transcribed from memory or from web
snippets: where the corruption chain is known, the mojibake is
*computed* from the intended text; where it is not (rspeer's unsolved
"mysteries"), the bytes are copied verbatim from the vendored upstream
files and marked unverifiable.
"""

from __future__ import annotations

import json
import sys
import unicodedata
from pathlib import Path

import ftfy

sys.path.insert(0, str(Path(__file__).parent))
from transforms import apply_chain  # noqa: E402

OUT = Path(__file__).resolve().parent.parent / "entries" / "classic-documented.json"

U8 = {"encode": "utf-8"}
D1252 = {"decode": "sloppy-windows-1252"}
DLAT1 = {"decode": "latin-1"}

FTFY_DOCS = "third-party/python-ftfy (docs/, ftfy/__init__.py docstrings, README)"

# (id, label, lang, intended, chain, provenance-source, notes)
COMPUTED = [
    ("classic-lopez-shipping-label",
     "Ode to a Shipping Label: LóPEZ triple-HTML-escaped mojibake",
     "es", "LóPEZ",
     [U8, DLAT1, {"transform": "html_named"},
      {"transform": "html_escape"}, {"transform": "html_escape"},
      {"transform": "uppercase"}],
     "ftfy docs/explain.rst; poem by Carlos Bueno (https://imgur.com/4J7Il0m)",
     "Real shipping label addressed to 'L&AMP;AMP;ATILDE;&AMP;AMP;SUP3;PEZ'. "
     "The label printer uppercased the entity names."),

    ("classic-mona-lisa-triple",
     "Mona Lisa: triple UTF-8/Windows-1252 mojibake of a curly apostrophe",
     "en", "The Mona Lisa doesn’t have eyebrows.",
     [U8, D1252, U8, D1252, U8, D1252],
     FTFY_DOCS, "README headline example of multi-layer mojibake."),

    ("classic-humanite-curl",
     "l'humanité: mojibake with smart quotes applied on top",
     "fr", "l'humanité",
     [U8, D1252, {"transform": "curl_apostrophe"}],
     FTFY_DOCS,
     "Cannot be decoded consistently until the curled quote is straightened."),

    ("classic-perturber-nbsp",
     "à perturber: U+A0 in mojibake flattened to ASCII space",
     "fr", "à perturber la réflexion",
     [U8, D1252, {"transform": "nbsp_to_space"}],
     FTFY_DOCS, "Ã + NBSP for 'à'; NBSP became a plain space."),

    ("classic-perturber-nbsp-gone",
     "à perturber: U+A0 in mojibake deleted entirely",
     "fr", "à perturber la réflexion",
     [U8, D1252, {"transform": "nbsp_to_nothing"}],
     FTFY_DOCS, "Same as above but the NBSP was dropped, leaving bare Ã."),

    ("classic-perez-entities",
     "P&EACUTE;REZ: wrongly-capitalized HTML entity",
     "es", "PÉREZ",
     [{"transform": "html_named"}, {"transform": "uppercase"}],
     FTFY_DOCS, "Only 'P&Eacute;REZ' is valid HTML5; ftfy accepts the all-caps form."),

    ("classic-shrug-macr",
     "Shrug kaomoji with &macr; entities and UTF-8/Latin-1 arms",
     "ja", "¯\\_(ツ)_/¯",
     [{"transform": "macron_entity"}, U8, DLAT1],
     FTFY_DOCS, "fix_text docstring: '&macr;\\\\_(ã\\x83\\x84)_/&macr;'."),

    ("classic-face-cli-fixture",
     "Kaomoji ┒(⌣˛⌣)┎ as UTF-8/Windows-1252 (upstream CLI test fixture)",
     "und", "┒(⌣˛⌣)┎\n",
     [U8, D1252],
     "third-party/python-ftfy/tests/face.txt",
     "Used by upstream test_cli.py as a file of real mojibake."),

    ("classic-schoen",
     "schön as UTF-8/Windows-1252 (fix_and_explain docstring)",
     "de", "schön", [U8, D1252], FTFY_DOCS,
     "Docstring of ftfy.fix_and_explain (ftfy/__init__.py): mojibake = "
     "'schÃ¶n'; ö = UTF-8 C3 B6 read as cp1252 → Ã¶."),

    ("classic-so",
     "só as UTF-8/Windows-1252 (fix_encoding_and_explain docstring)",
     "pt", "só", [U8, D1252], FTFY_DOCS,
     "Docstring of ftfy.fix_encoding_and_explain: mojibake = 'sÃ³'; "
     "ó = UTF-8 C3 B3 read as cp1252 → Ã³."),

    ("classic-voila-nbsp",
     "voilà le travail with flattened NBSP (fix_encoding_and_explain docstring)",
     "fr", "voilà le travail",
     [U8, D1252, {"transform": "nbsp_to_space"}], FTFY_DOCS,
     "Docstring of ftfy.fix_encoding_and_explain: 'voilÃ\\xa0 le travail' "
     "with the recovered NBSP flattened to a space."),

    # --- Wikipedia "Mojibake" article scenarios (strings computed) ---
    ("wiki-mojibake-jp-utf8-latin1",
     "文字化け (the word 'mojibake') as UTF-8 read as Latin-1",
     "ja", "文字化け", [U8, DLAT1],
     "https://en.wikipedia.org/wiki/Mojibake",
     "The article's titular example renders as 'æ–‡å­—åŒ–ã'-style garbage."),

    ("wiki-mojibake-ru-krakozyabry",
     "кракозябры: Russian CP1251 bytes read as KOI8-R",
     "ru", "Кракозябры", [{"encode": "windows-1251"}, {"decode": "koi8-r"}],
     "https://en.wikipedia.org/wiki/Mojibake (Russian section)",
     "The classic Russian name for mojibake, garbled the classic way."),

    ("wiki-mojibake-ru-voprosy",
     "ВОПРОСЫ: Russian KOI8-R bytes read as CP1251 (бнопня lore)",
     "ru", "Вопросы", [{"encode": "koi8-r"}, {"decode": "windows-1251"}],
     "Russian internet lore; https://ru.wikipedia.org/wiki/Кракозябры",
     "KOI8-R⇄CP1251 confusion preserves letter-ness, producing readable-looking gibberish."),

    ("wiki-mojibake-de-umlauts",
     "German umlauts as UTF-8/Latin-1",
     "de", "Größenwahnsinniger Übermut für müde Wüstenfüchse",
     [U8, DLAT1], "https://en.wikipedia.org/wiki/Mojibake (German section)",
     "für→fÃ¼r is the canonical German example."),

    ("wiki-mojibake-sv-smorgas",
     "Swedish räksmörgås as UTF-8/Latin-1",
     "sv", "Beställ en räksmörgås på smörgåsbordet",
     [U8, DLAT1], "https://en.wikipedia.org/wiki/Mojibake (Swedish section)",
     "Swedish 'räksmörgås' (shrimp sandwich) is the article's Swedish test "
     "word; ä/ö/å each become Ã-prefixed pairs under Latin-1."),

    ("classic-mysql-latin1-chinese",
     "中文 as UTF-8 stored in a MySQL latin1 column",
     "zh-Hans", "中文编码", [U8, DLAT1],
     "Ubiquitous MySQL latin1/utf8 bug-report lore ('ä¸­æ–‡')",
     "The single most-reported CJK mojibake pattern on Stack Overflow."),

    ("classic-bom-artifact",
     "UTF-8 BOM read as Windows-1252 at start of a CSV (ï»¿)",
     "en", "﻿Name,Email", [U8, D1252],
     "Ubiquitous Excel/CSV bug-report lore",
     "ï»¿ at the start of files is the UTF-8 BOM misread as cp1252."),
]

# Provenance-only entries: real wild mojibake whose intended text is
# unknown or only partially deciphered. Strings copied verbatim from
# rspeer's notes/mysteries.txt — never marked verified.
MYSTERIES = [
    ("mystery-melanie-triple-utf8",
     "Comment signature 'MÃÂ©ÃÂ¬ÃÂ¡nie' (triple-UTF-8 for 'M鬡nie', probably a French name)",
     "fr", None, "MÃÂ©ÃÂ¬ÃÂ¡nie",
     "third-party/python-ftfy/notes/mysteries.txt; https://www.nipette.com/article-6358031.html",
     "rspeer: 'This happens to be triple-UTF-8 for M鬡nie, but that's probably "
     "not the name they meant.' Plausibly Mélanie through a lossy chain."),
    ("mystery-tadeas-nested",
     "Czech name Tadeáš nested ~15 layers deep",
     "cs", None,
     "TadeÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¡ÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂÃÂ¡",
     "third-party/python-ftfy/notes/mysteries.txt; "
     "https://www.horoskopy-horoskop.cz/clanek/431-numerologicky-vyznam-jmena-jaromir",
     "rspeer's unsolved mystery; deeply nested UTF-8/Latin-1-ish with lost bytes."),
    ("mystery-montreal-cp850ish",
     "'montrã©al' — looks like Montréal but rspeer notes it isn't quite cp850",
     "fr", None, "montrã©al",
     "third-party/python-ftfy/notes/mysteries.txt; https://mtlurb.com/tags/arbres/",
     "Lowercase ã© instead of Ã©: a case-folding pass ran *after* the mojibake."),
]


def main() -> None:
    entries = []
    for id_, label, lang, intended, chain, source, notes in COMPUTED:
        mojibake = apply_chain(intended, chain)
        assert mojibake != intended, f"{id_}: chain is a no-op"
        output = ftfy.fix_text(mojibake)
        entries.append({
            "id": id_, "label": label, "lang": lang,
            "intended": intended, "mojibake": mojibake, "chain": chain,
            "provenance": {"type": "documented", "source": source, "notes": notes},
            "verified": True,
            "ftfy_output": output,
            "ftfy_recovers": unicodedata.normalize("NFC", intended) == output,
        })
    for id_, label, lang, intended, mojibake, source, notes in MYSTERIES:
        output = ftfy.fix_text(mojibake)
        entries.append({
            "id": id_, "label": label, "lang": lang,
            "intended": intended, "mojibake": mojibake, "chain": None,
            "provenance": {"type": "wild-unsolved", "source": source, "notes": notes},
            "verified": False,
            "ftfy_output": output,
            "ftfy_recovers": None,
        })
    OUT.write_text(json.dumps(entries, ensure_ascii=False, indent=2) + "\n",
                   encoding="utf-8")
    print(f"wrote {len(entries)} entries to {OUT}")


if __name__ == "__main__":
    main()
