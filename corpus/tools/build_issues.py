#!/usr/bin/env python3
"""Build corpus/entries/ftfy-issues.json from real user-reported mojibake.

Sources: GitHub issues on rspeer/python-ftfy (see
corpus/sources/ftfy-issues-research.md for the raw research notes).
Wherever the corruption chain was confirmed in the issue thread, the
mojibake string here is *computed* from the intended text via that
chain, so transcription noise (or hallucinated "mojibake-looking"
strings) cannot enter the corpus. Entries whose chain is unknown are
included verbatim and permanently marked unverified.
"""

from __future__ import annotations

import json
import sys
import unicodedata
from pathlib import Path

import ftfy

sys.path.insert(0, str(Path(__file__).parent))
from transforms import apply_chain  # noqa: E402

OUT = Path(__file__).resolve().parent.parent / "entries" / "ftfy-issues.json"

U8 = {"encode": "utf-8"}
D1252 = {"decode": "sloppy-windows-1252"}
DLAT1 = {"decode": "latin-1"}


def issue(n: int) -> str:
    return f"https://github.com/rspeer/python-ftfy/issues/{n}"


# (id, label, lang, intended, chain, issue#, notes)
COMPUTED = [
    ("issue-229-eszett-lossy-Y",
     "German ß as UTF-8/cp1252 with Ÿ flattened to Y by an email program",
     "de", "Straße",
     [U8, D1252, {"replace": ["Ÿ", "Y"]}],
     229, "Surname case; the ÃY pattern is unrecoverable and ftfy leaves it. "
          "Intended word here is representative (real surname not public)."),
    ("issue-141-apostrophe-inverted-question",
     "Dutch curly apostrophes flattened to ¿ (lossy cp850-ish chain)",
     "nl", "zo’n beetje alle camera’s",
     [{"replace": ["’", "¿"]}],
     141, "Reporter confirmed intent; information-destroying, unrecoverable."),
    ("issue-29-euckr-latin1",
     "Korean EUC-KR bytes decoded as Latin-1 (song title)",
     "ko", "소리엘 - 사랑하는 자여",
     [{"encode": "euc-kr"}, {"decode": "latin-1"}],
     29, "ftfy declines (not a UTF-8 mixup); naive fixers mangle it further."),
    ("issue-168-utf8-iso88592",
     "German UTF-8 read as ISO-8859-2 (ftfy guesses windows-1250 instead)",
     "de", "Schlüsselwörter",
     [U8, {"decode": "iso-8859-2"}],
     168, "Ambiguous between iso-8859-2 and cp1250; ftfy's choice yields "
          "SchlßsselwÜrter (wrong-fix case)."),
    ("issue-222-string-start",
     "Swedish UTF-8/Latin-1 mojibake at the very start of the string",
     "sv", "åklagarmyndighets",
     [U8, DLAT1],
     222, "ftfy <6.1 failed specifically when mojibake began the string."),
    ("issue-119-shiftjis-cp850",
     "Japanese Shift-JIS filename decoded as cp850 (DOS box-drawing soup)",
     "ja", "時間試験観点（アニメパス）_10秒.png",
     [{"encode": "shift_jis"}, {"decode": "cp850"}],
     119, "ftfy does not attempt Shift-JIS recovery; genuine unfixable mojibake."),
    ("issue-231-utf8-koi8r",
     "Russian email disclaimer: UTF-8 bytes decoded as KOI8-R",
     "ru", "Если Вы не являетесь адресатом такой информации",
     [U8, {"decode": "koi8-r"}],
     231, "Box-drawing Cyrillic soup (п∙я│…); reporter supplied both sides."),
    ("issue-192-istanbul",
     "Turkish İstanbul as UTF-8/cp1252",
     "tr", "İstanbul",
     [U8, D1252], 192, "Confirmed via iconv in the issue."),
    ("issue-192-riga",
     "Latvian Rīga as UTF-8/cp1252",
     "lv", "Rīga",
     [U8, D1252], 192, ""),
    ("issue-217-truncated-chinese",
     "Chinese UTF-8 truncated mid-character, then read as cp1252",
     "zh-Hans", "Python 删除列表中的元素",
     [U8, {"bytes": "drop_last"}, D1252],
     217, "Final 素 loses its last byte; ftfy's recovery of the tail differs."),
    ("issue-203-alesund",
     "Norwegian Ålesund as UTF-8/cp1252",
     "no", "Ålesund",
     [U8, D1252], 203, ""),
    ("issue-210-lithuanian-1257",
     "Lithuanian UTF-8 read as windows-1257 (Baltic)",
     "lt", "Sąrašai",
     [U8, {"decode": "sloppy-windows-1257"}],
     210, "windows-1257 support added to ftfy 6.1; plsfix README notes this "
          "codec is missing there — expected differential!"),
    ("issue-202-partial-mojibake-dash",
     "Danish: only å garbled, legit en-dash nearby (ftfy once 'fixed' the dash)",
     "da", "Bremer/Mccoy – Dråber",
     [{"replace": ["å", "Ã¥"]}],
     202, "Wrong-fix regression case: ftfy 5.x produced 'РDr̴'."),
    ("issue-146-dropped-a-circumflex",
     "Dutch en-dashes as cp1252 mojibake with the â byte dropped",
     "nl", "–Echinacea en Salvia–",
     [U8, D1252, {"replace": ["â", ""]}],
     146, "€“ pattern; ftfy 5.1 added partial handling."),
    ("issue-154-emoji-1254",
     "Emoji as UTF-8 read as windows-1254 (Turkish)",
     "emoji", "😊",
     [U8, {"decode": "sloppy-windows-1254"}],
     154, "ğŸ˜Š pattern; cp1254 support added in response."),
    ("issue-180-latin1-cp437",
     "Spanish receipt printer: Latin-1 bytes shown as CP437",
     "es", "CUÉNTENOS EN ESPAÑOL",
     [{"encode": "latin-1"}, {"decode": "cp437"}],
     180, "CU╔NTENOS EN ESPA╤OL; single-byte→single-byte confusion."),
    ("issue-169-yael",
     "French name Yaël as UTF-8/Latin-1",
     "fr", "Yaël",
     [U8, DLAT1], 169, ""),
    ("issue-103-koenig-latin1",
     "König as UTF-8/Latin-1", "de", "König", [U8, DLAT1], 103, ""),
    ("issue-103-koenig-macroman",
     "König as UTF-8/MacRoman", "de", "König",
     [U8, {"decode": "macroman"}], 103,
     "K√∂nig; the issue that motivated MacRoman support."),
    ("issue-103-koenig-cp437-raw",
     "König stored as cp437, byte 0x94 surfacing as a C1 control",
     "de", "König",
     [{"encode": "cp437"}, {"decode": "latin-1"}], 103,
     "K\\x94nig: bare legacy byte, not UTF-8 mojibake."),
    ("issue-164-pieta-nbsp",
     "Italian pietà as UTF-8/Latin-1 — second byte of à is NBSP",
     "it", "pietà?",
     [U8, DLAT1], 164,
     "ftfy's heuristics once rejected the fix because of the real \\xa0."),
    ("issue-159-truncated-korean",
     "Korean UTF-8 truncated mid-character, then read as cp1252",
     "ko", "준다고",
     [U8, {"bytes": "drop_last"}, D1252], 159, ""),
    ("issue-157-nbsp-adjacent",
     "Swedish partial mojibake with a genuine NBSP elsewhere in the string",
     "sv", "Linköpings Universitet, LiU",
     [{"replace": ["ö", "Ã¶"]}], 157,
     "Only ö was garbled; the real NBSP confused ftfy <6.0."),
    ("issue-123-mixed-legit-accents",
     "Spanish: garbled á next to legitimate accented capitals",
     "es", "Colombia, Boyaca, PUERTO BOYACÁ, Boyacá. Puerto Boyacá",
     [{"replace": ["á", "Ã¡"]}], 123,
     "ftfy once gave up because the legit Á made the text look 'already fine'."),
    ("issue-120-utf16-nuls",
     "UTF-16-BE filename bytes read as Latin-1 (NUL interleaving)",
     "en", "standard 07_51",
     [{"encode": "utf-16-be"}, {"decode": "latin-1"}], 120,
     "Declared out of scope for ftfy; both libraries should leave it (or "
     "at most strip the NULs as control characters)."),
    ("issue-97-ongeevenaard",
     "Dutch ongeëvenaard as UTF-8/Latin-1",
     "nl", "ongeëvenaard",
     [U8, DLAT1], 97,
     "Once fixable by fix_one_step but rejected by the weirdness heuristic."),
    ("issue-83-question-mark-loss",
     "English curly quotes as cp1252 mojibake with €/œ/™ flattened to ?",
     "en", "They say, “Let the woman take care of you” and lover’s",
     [U8, D1252, {"replace": ["€", "?"]}, {"replace": ["œ", "?"]},
      {"replace": ["™", "?"]}],
     83, "â?? pattern: information destroyed, unrecoverable by design."),
    ("issue-77-vietnamese",
     "Vietnamese headline as UTF-8/cp1252",
     "vi", "Nối lại tuần tra, chính quyền Trump xóa tan nghi ngờ ở Biển Đông",
     [U8, D1252], 77,
     "Failed in ftfy 4.x; Vietnamese stacks diacritics densely."),
    ("issue-52-a-for-a-circumflex",
     "Bullet/en-dash mojibake with â corrupted to plain a",
     "en", "• Strong telecom background in BSS area – Preferably",
     [U8, D1252, {"replace": ["â", "a"]}], 52,
     "a€¢ / a€“ pattern; lossy and unrecoverable."),
    ("issue-96-fur-burkinabe",
     "für / Burkinabé as UTF-8/Latin-1 (uncurl_quotes interaction bug)",
     "de", "für Burkinabé",
     [U8, DLAT1], 96, ""),
    ("issue-128-u-escapes",
     "German mojibake further wrapped in literal \\uXXXX escapes",
     "de", "Demgemäß fiel die Wahl auf 3×6 Meter",
     [U8, DLAT1, {"transform": "u_escape_upper"}], 128,
     "Neither library unescapes \\u literals; documents the boundary."),
    ("issue-66-double-escaped-ncr",
     "cp1252 numeric character references, HTML-escaped once more",
     "en", "I’m blue, da ba dee da ba doo…",
     [{"replace": ["’", "&#x92;"]}, {"replace": ["…", "&#133;"]},
      {"transform": "html_escape"}], 66,
     "&amp;#x92; — entities of cp1252 control-range codepoints."),
    ("issue-190-fffd-adjacent",
     "Dutch beëindiging with the ë lost to a private replacement glyph",
     "nl", "beëindiging",
     [{"replace": ["ë", "ï¿œ"]}], 190,
     "ï¿œ is U+FFFD's UTF-8 bytes in cp1252 with the half-ring replaced by œ "
     "(byte 0xBD→œ is windows-1257-ish); chain reconstructed: utf-8 bytes of "
     "U+FFFD (EF BF BD) where BD was shown as œ. Known ftfy failure."),
]

# Verbatim from issues; chain unknown or not reproducible — never verified.
VERBATIM = [
    ("issue-149-pallas-wrongfix",
     "Dutch ë garbled by an unknown nonstandard chain; ftfy wrongly emits U+26B4",
     "nl", "Officiële gecreëerde patatten", "Officiâš´le gecreâš´erde patatten",
     149, "Wrong-fix case: ftfy 6.x produces 'Offici⚴le' (PALLAS symbol)."),
    ("issue-34-gb-soup",
     "GB-series Chinese pushed through a cp1252 sieve, multiply corrupted",
     "zh-Hans", None,
     "Ã¨Â¢â€¹Ã¨Â¢âdcx€¹Ã¤Â¸Å½Ã¦Å\"â€¹Ã¥Ââ€¹Ã¤Â»Â¬Ã§â€ÂµÃ¥Â­ÂÃ¥â€¢â€",
     34, "Intended text unknown; ftfy has no GB18030 support (documented "
         "limitation). Transcribed from the issue; treat string shape, not "
         "exact bytes, as the test target."),
    ("issue-225-greek-fffd",
     "Greek multi-layer mojibake with U+FFFD byte loss; intended text only "
     "inferred by an LLM in-thread (explicit hallucination risk!)",
     "el", None,
     "ÃƒÅ½Ã‚Â¤ÃƒÅ½Ã¢â‚¬Ëœ",
     225, "The thread's claimed decoding 'ΤΑΞΙΔΙ ΞΑΝΘΗ' is an unverified LLM "
          "inference — kept as a cautionary example per the corpus README."),
]

# Lone-surrogate cases (issue #178): Python-only, can't live in JSON that
# Rust must also parse; the harness constructs them programmatically.
SURROGATE_NOTE = {
    "id": "issue-178-surrogateescape",
    "label": "Latin-1 filenames surfaced via surrogateescape (as\\udced.mp3)",
    "see": issue(178),
    "construct": "'01-Basta de llamarme as' + chr(0xDCED) + '.mp3'",
    "note": "Handled in harness/compare.py as a programmatic edge case: "
            "Rust strings cannot hold lone surrogates, so plsfix's binding "
            "behavior (error vs replacement) is itself the observable.",
}


def main() -> None:
    entries = []
    for id_, label, lang, intended, chain, n, notes in COMPUTED:
        mojibake = apply_chain(intended, chain)
        assert mojibake != intended, f"{id_}: chain is a no-op"
        output = ftfy.fix_text(mojibake)
        entries.append({
            "id": id_, "label": label, "lang": lang,
            "intended": intended, "mojibake": mojibake, "chain": chain,
            "provenance": {"type": "issue-report", "source": issue(n), "notes": notes},
            "verified": True,
            "ftfy_output": output,
            "ftfy_recovers": unicodedata.normalize("NFC", intended) == output,
        })
    for id_, label, lang, intended, mojibake, n, notes in VERBATIM:
        output = ftfy.fix_text(mojibake)
        entries.append({
            "id": id_, "label": label, "lang": lang,
            "intended": intended, "mojibake": mojibake, "chain": None,
            "provenance": {"type": "issue-report-verbatim", "source": issue(n),
                           "notes": notes},
            "verified": False,
            "ftfy_output": output,
            "ftfy_recovers": (
                unicodedata.normalize("NFC", intended) == output
                if intended else None),
        })
    OUT.write_text(json.dumps(entries, ensure_ascii=False, indent=2) + "\n",
                   encoding="utf-8")
    n_fix = sum(bool(e["ftfy_recovers"]) for e in entries)
    print(f"wrote {len(entries)} entries to {OUT}; ftfy recovers {n_fix}")


if __name__ == "__main__":
    main()
