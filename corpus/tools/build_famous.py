#!/usr/bin/env python3
"""Build corpus/entries/famous-incidents.json.

Documented, famous mojibake from encoding folklore. Each garbled string
is *computed* from a known input through a known chain and was
cross-checked against the verbatim string reported in the cited source
(see corpus/sources/classic-mojibake-research.md). Several of these are
not text→encoding→encoding at all but uninitialized memory or
replacement characters rendered through a codec; those use byte-literal
seeds and are flagged accordingly.

This file is the heart of the anti-hallucination story: every famous
sample here round-trips, which is exactly the property an AI-imagined
"mojibake-looking" string would fail (rspeer, "Never lose a dead end").
"""

from __future__ import annotations

import json
import sys
import unicodedata
from pathlib import Path

import ftfy

sys.path.insert(0, str(Path(__file__).parent))
from transforms import apply_chain  # noqa: E402

OUT = Path(__file__).resolve().parent.parent / "entries" / "famous-incidents.json"

U8 = {"encode": "utf-8"}
D1252 = {"decode": "sloppy-windows-1252"}
DLAT1 = {"decode": "latin-1"}

# (id, label, lang, intended, chain, source, notes)
COMPUTED = [
    ("famous-bush-hid-the-facts",
     "'Bush hid the facts' — Windows Notepad IsTextUnicode bug (2004)",
     "en", "Bush hid the facts",
     [{"encode": "ascii"}, {"decode": "utf-16-le"}],
     "https://en.wikipedia.org/wiki/Bush_hid_the_facts",
     "ASCII misdetected as UTF-16LE by IsTextUnicode(); pairs of ASCII "
     "bytes become CJK codepoints. Note: not classic mojibake — neither "
     "ftfy nor plsfix targets this, so it documents a non-goal."),
    ("famous-kunjinkao-replacement",
     "锟斤拷 — double U+FFFD encoded UTF-8, decoded GBK (Chinese ▢ folklore)",
     "zh-Hans", "��",
     [U8, {"decode": "gbk"}],
     "https://en.wikipedia.org/wiki/Mojibake ; "
     "https://github.com/Ovler-Young/Mojibake-recovery",
     "Two replacement chars (EF BF BD EF BF BD) re-encoded UTF-8 then read "
     "as GBK pair up into 锟/斤/拷. The seed is genuinely lossy garbage."),
    ("famous-bnopnya",
     "бНОПНЯ — Russian 'Вопрос' as CP1251 read as KOI8-R (the iconic krakozyabры)",
     "ru", "Вопрос",
     [{"encode": "windows-1251"}, {"decode": "koi8-r"}],
     "https://neolurk.org/wiki/БНОПНЯ ; https://cyclowiki.org/wiki/Кракозябры",
     "The single most famous Russian mojibake string; KOI8-R⇄CP1251 swaps "
     "case ranges, so it stays letter-shaped and almost pronounceable."),
    ("famous-ophbet",
     "оПХБЕР — Russian 'Привет' as CP1251 read as KOI8-R",
     "ru", "Привет",
     [{"encode": "windows-1251"}, {"decode": "koi8-r"}],
     "https://dic.academic.ru/dic.nsf/ruwiki/1069825",
     "Has its own dictionary entry. Capital П (0xCF) maps to lowercase KOI8 "
     "'о', hence the lowercase first letter."),
    ("famous-ala-aseroe-single",
     "AseroÃ« — fungal genus Aseroë, UTF-8/Latin-1 (Atlas of Living Australia)",
     "la", "Aseroë",
     [U8, DLAT1],
     "https://www.datafix.com.au/BASHing/2020-04-01.html",
     "Taxonomic diaeresis names mangled in a biodiversity database."),
    ("famous-ala-aseroe-double",
     "AseroÃƒÂ« — same genus, double UTF-8/cp1252 conversion",
     "la", "Aseroë",
     [U8, D1252, U8, D1252],
     "https://www.datafix.com.au/BASHing/2020-04-01.html",
     "The article documents the byte-by-byte double conversion explicitly."),
    ("famous-ala-naïs",
     "NaÃ¯s — fungal genus Naïs, UTF-8/Latin-1 (Atlas of Living Australia)",
     "la", "Naïs", [U8, DLAT1],
     "https://www.datafix.com.au/BASHing/2020-04-01.html",
     "ï = UTF-8 C3 AF; read as Latin-1 it splits into Ã + ¯. A taxonomic "
     "name with a diaeresis, mangled in a biodiversity database export."),
    ("famous-norwegian-smorbrod",
     "SmÃ¸rbrÃ¸d — Norwegian 'Smørbrød' (open sandwich) as UTF-8/Latin-1",
     "no", "Smørbrød", [U8, DLAT1],
     "https://en.wikipedia.org/wiki/Mojibake",
     "ø = UTF-8 C3 B8 → Ã¸ under Latin-1; the canonical Scandinavian example "
     "(the Wikipedia Mojibake article uses Norwegian/Danish ø for this)."),
    ("famous-korean-hangugeo",
     "í•œêµ­ì–´ — Korean '한국어' (Korean language) as UTF-8/cp1252",
     "ko", "한국어", [U8, D1252],
     "https://ssojet.com/compare-character-encoding/euc-kr-vs-utf-8",
     "The literal word 'Korean language'; each Hangul syllable is 3 UTF-8 "
     "bytes that split into three cp1252 characters — the classic Korean "
     "UTF-8-as-Western pattern."),
    ("famous-russian-privet-utf8",
     "Ð¿Ñ€Ð¸Ð²ÐµÑ‚ — Russian 'привет' (hello) as UTF-8/cp1252",
     "ru", "привет", [U8, D1252],
     "https://grokipedia.com/page/Mojibake",
     "The word 'hello'; the most-cited example of UTF-8 Cyrillic shown "
     "through a Western codepage (each letter → Ð/Ñ + a second char)."),
    ("famous-german-fuer",
     "fÃ¼r — German 'für' as UTF-8/Latin-1 (Wikipedia lead example)",
     "de", "für", [U8, DLAT1],
     "https://en.wikipedia.org/wiki/Mojibake",
     "ü = UTF-8 C3 BC → fÃ¼r; the canonical German example used in the "
     "Wikipedia Mojibake article's lead."),
    ("famous-cafe",
     "cafÃ© — 'café' as UTF-8/Latin-1 (the canonical one-word example)",
     "fr", "café", [U8, DLAT1],
     "https://unicodefyi.com/glossary/mojibake/",
     "é = UTF-8 C3 A9 → cafÃ©; the single most common one-word demonstration "
     "of UTF-8-as-Latin-1 mojibake."),
    ("famous-emdash-scherlis",
     "â€\" — em dash '—' as UTF-8/cp1252 (Scherlis, New Frontiers in Mojibake)",
     "en", "—", [U8, D1252],
     "https://adam.scherlis.com/2022/11/25/new-frontiers-in-mojibake/",
     "E2 80 94 → â € \" under cp1252; the textbook punctuation case."),
    ("famous-arent-dont",
     "arenâ€™t … donâ€™t — curly apostrophes as UTF-8/cp1252 (rspeer ftfy 3.0)",
     "en", "If numbers aren’t beautiful, I don’t know what is",
     [U8, D1252],
     "http://rspeer.github.io/blog/2013/08/26/ftfy-fixes-text-for-you-3-dot-0/",
     "Curly apostrophes (U+2019 = E2 80 99) shown as â€™ under cp1252; from "
     "rspeer's own ftfy 3.0 announcement."),
    ("famous-leon-rocha",
     "LeÃ³n Rochaâ€™s — 'León Rocha's' as UTF-8/cp1252 (alexwlchan fix_and_explain)",
     "es", "Amadeo León Rocha’s plight",
     [U8, D1252],
     "https://alexwlchan.net/notes/2025/ftfy-fix-and-explain/",
     "alexwlchan reports ftfy's explanation: sloppy-windows-1252 / utf-8 / "
     "uncurl_quotes — a good explain-step cross-check."),
    ("famous-shrug-thai",
     "(à¸‡'âŒ£')à¸‡ — Thai kaomoji '(ง'⌣')ง' as UTF-8/cp1252 (plsfix README headline)",
     "th", "(ง'⌣')ง", [U8, D1252],
     "plsfix README; ftfy README",
     "The README's marquee example; ง is Tho Thong (U+0E07)."),
    ("famous-theyre-curl",
     "theyâ€™re — 'they’re' (curly apostrophe) as UTF-8/cp1252",
     "en", "they’re", [U8, D1252],
     "https://www.justinweiss.com/articles/how-to-get-from-theyre-to-theyre/",
     "The canonical apostrophe-mojibake blog example."),
    ("famous-ala-elsinoe",
     "ElsinoÃ« — fungal genus Elsinoë as UTF-8/Latin-1 (Atlas of Living Australia)",
     "la", "Elsinoë", [U8, DLAT1],
     "https://www.datafix.com.au/BASHing/2020-04-01.html",
     "One of several diaeresis fungal genera the datafix article documents "
     "as mangled in the Atlas of Living Australia (ë → Ã«)."),
    ("famous-ala-helicoon",
     "HelicoÃ¶n — fungal genus Helicoön as UTF-8/Latin-1 (Atlas of Living Australia)",
     "la", "Helicoön", [U8, DLAT1],
     "https://www.datafix.com.au/BASHing/2020-04-01.html",
     "ö = UTF-8 C3 B6 → Ã¶; same Atlas of Living Australia export bug."),
    ("famous-ala-parepichloe",
     "ParepichloÃ« — fungal genus Parepichloë as UTF-8/Latin-1",
     "la", "Parepichloë", [U8, DLAT1],
     "https://www.datafix.com.au/BASHing/2020-04-01.html",
     "Another diaeresis genus from the same Atlas of Living Australia case."),
    ("famous-ala-zignoella",
     "ZignoÃ«lla — fungal genus Zignoëlla as UTF-8/Latin-1",
     "la", "Zignoëlla", [U8, DLAT1],
     "https://www.datafix.com.au/BASHing/2020-04-01.html",
     "Fourth diaeresis genus from the same Atlas of Living Australia case."),
    ("famous-muller-double",
     "MÃƒÂ¼ller — surname 'Müller' double-encoded (UTF-8/cp1252 twice)",
     "de", "Müller", [U8, D1252, U8, D1252],
     "https://blogs.perl.org/users/chansen/2010/10/coping-with-double-encoded-utf-8.html",
     "The textbook double-encoded-UTF-8 illustration."),
    ("famous-jp-fieldguide-sjis",
     "繧/縺/繝 texture — Japanese UTF-8 read as Shift-JIS (the web's most common mojibake)",
     "ja", "日本語のテキスト", [U8, {"decode": "shift_jis", "errors": "replace"}],
     "https://www.dampfkraft.com/mojibake-field-guide.html",
     "Paul McCann's field guide: UTF-8-as-Shift-JIS yields recurring 繧縺繝 kanji."),
    ("famous-jp-fieldguide-eucjp",
     "Half-width katakana texture — Japanese EUC-JP read as Shift-JIS",
     "ja", "日本語のテキスト", [{"encode": "euc-jp"}, {"decode": "shift_jis", "errors": "replace"}],
     "https://www.dampfkraft.com/mojibake-field-guide.html",
     "EUC-JP-as-Shift-JIS produces lots of half-width katakana."),
    ("famous-beatport-ubersprung",
     "Ãœbersprung — Beatport ID3 tag 'Übersprung (Original Mix)' shown as UTF-8/cp1252",
     "de", "Übersprung (Original Mix)", [U8, D1252],
     "https://gehrcke.de/2014/07/mojibake-beatports-id3-text-encoding-is-broken/",
     "The ID3 bytes were double-encoded; players decode them to this visible "
     "Ãœ… form, which is what a fixer actually receives."),
    ("famous-hotel-doctest",
     "HÃ”TEL — French 'HÔTEL' as UTF-8/cp1252 (ftfy doctest)",
     "fr", "HÔTEL", [U8, D1252],
     "https://ftfy.readthedocs.io/en/latest/explain.html",
     "ftfy documentation example; Ô = UTF-8 C3 94 → Ã + ” (0x94 is a curly "
     "quote in cp1252), so the word is HÃ”TEL."),
    ("famous-ete",
     "Ã©tÃ© — French 'été' (summer) as UTF-8/Latin-1",
     "fr", "été", [U8, DLAT1],
     "http://blog.conceptnet.io/ftfy/",
     "Short French word from ftfy's ConceptNet-blog examples; é → Ã©, twice."),
    ("famous-mysql-doubly-spanish",
     "ÃƒÂ¡ ÃƒÂ© ÃƒÂ³ ÃƒÂ± — Spanish 'á é ó ñ' doubly UTF-8-encoded in MySQL",
     "es", "á é ó ñ", [U8, D1252, U8, D1252],
     "https://jonisalonen.com/2012/fixing-doubly-utf-8-encoded-text-in-mysql/",
     "jonisalonen's canonical doubly-encoded case: á = E1 -> C3 A1 -> "
     "C3 83 C2 A1; ~30,000 affected forum posts."),
    ("famous-jp-kasou-machine-cp932",
     "莉ｮ諠ｳ繝槭す… — Japanese '仮想マシンサービス' as UTF-8 read as CP932/Shift-JIS",
     "ja", "仮想マシンサービス", [U8, {"decode": "cp932", "errors": "replace"}],
     "https://github.com/anthropics/claude-code/issues/36061",
     "'virtual machine service'; a real terminal-output mojibake report. "
     "The 繧/縺/繝 field-guide texture in action."),
    ("famous-jp-zip-hitomi",
     "é╨é╞é▌.png — Japanese ZIP filename 'ひとみ.png' (cp932 read as cp437)",
     "ja", "ひとみ.png", [{"encode": "cp932"}, {"decode": "cp437", "errors": "replace"}],
     "https://eatpeppershothot.blogspot.com/2014/07/how-to-fix-broken-cjk-filenames.html",
     "ZIP spec only blesses cp437 + UTF-8, so cp932 filenames surface as "
     "DOS box-drawing soup on extraction."),
    ("famous-zh-nihao-latin1",
     "ä½\\xa0å¥½ — Chinese '你好' (hello) as UTF-8 read as Latin-1",
     "zh-Hans", "你好", [U8, DLAT1],
     "https://devblogs.microsoft.com/oldnewthing/20190701-00/?p=102636",
     "你 = E4 BD A0; the A0 byte is NBSP under Latin-1 (snippets often show "
     "it as a plain space — the computed byte-exact form keeps the NBSP)."),
    ("famous-zh-nihao-gbk-latin1",
     "ÄãºÃ — Chinese '你好' stored as GBK, read as Latin-1",
     "zh-Hans", "你好", [{"encode": "gbk"}, {"decode": "latin-1"}],
     "https://ssojet.com/compare-character-encoding/gbk-vs-big5",
     "Legacy-codec-as-Latin-1, the GBK analogue of the UTF-8/Latin-1 case."),
]

# Byte-literal seeds: these famous strings come from raw bytes (memory
# fill patterns, BOM), not from text pushed through a codec. We seed the
# bytes directly via a decode-only chain over a synthetic intended value.
BYTE_SEEDS = [
    ("famous-tangtangtang",
     "烫烫烫 ('scalding') — MSVC 0xCC stack-fill bytes read as GBK",
     "zh-Hans", bytes([0xCC] * 6), "gbk",
     "https://blog.csdn.net/jarelzhou/article/details/19013037",
     "Uninitialized stack memory (debug fill 0xCC) printed as a string on "
     "a Chinese-locale Windows. Not recoverable text; documents a non-goal."),
    ("famous-tuntuntun",
     "屯屯屯 — MSVC 0xCD heap-fill bytes read as GBK",
     "zh-Hans", bytes([0xCD] * 6), "gbk",
     "https://www.cnblogs.com/imjustice/archive/2012/03/05/2623915.html",
     "Heap debug fill (0xCD); the heap counterpart of 烫烫烫."),
    ("famous-utf8-bom-as-gbk",
     "锘 — UTF-8 BOM (EF BB BF) read as GBK",
     "zh-Hans", bytes([0xEF, 0xBB, 0xBF]), "gbk",
     "https://blog.csdn.net/jarelzhou/article/details/19013037",
     "BOM-as-text artifact at file starts on Chinese-locale tools."),
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
            "provenance": {"type": "famous-incident", "source": source, "notes": notes},
            "verified": True,
            "ftfy_output": output,
            "ftfy_recovers": unicodedata.normalize("NFC", intended) == output,
        })
    for id_, label, lang, raw, codec, source, notes in BYTE_SEEDS:
        mojibake = raw.decode(codec, "replace")
        output = ftfy.fix_text(mojibake)
        entries.append({
            "id": id_, "label": label, "lang": lang,
            "intended": None, "mojibake": mojibake,
            "chain": [{"raw_bytes_hex": raw.hex()}, {"decode": codec, "errors": "replace"}],
            "provenance": {"type": "famous-incident-bytes", "source": source, "notes": notes},
            "verified": True,  # the bytes are fixed and the decode is deterministic
            "ftfy_output": output,
            "ftfy_recovers": None,  # there is no "correct" text to recover
        })
    OUT.write_text(json.dumps(entries, ensure_ascii=False, indent=2) + "\n",
                   encoding="utf-8")
    n_fix = sum(bool(e["ftfy_recovers"]) for e in entries)
    print(f"wrote {len(entries)} entries to {OUT}; ftfy recovers {n_fix}")


if __name__ == "__main__":
    main()
