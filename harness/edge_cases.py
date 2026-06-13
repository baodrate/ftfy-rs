"""Programmatic edge cases for the differential harness.

These can't (or shouldn't) live in the JSON corpus: lone surrogates that
JSON-for-Rust can't carry, pathologically large inputs, degenerate
strings, and structural boundary cases. Each is returned in the same
shape as a corpus entry so ``compare.py`` treats them uniformly. They
carry ``intended: None`` where there is no single correct answer — the
point is to compare *behavior*, not to assert recovery.
"""

from __future__ import annotations


def _case(id_, label, mojibake, intended=None, lang="und"):
    return {
        "id": f"edge-{id_}", "label": label, "lang": lang,
        "intended": intended, "mojibake": mojibake, "chain": None,
        "provenance": {"type": "edge-case", "source": "harness/edge_cases.py"},
        "verified": False, "_file": "edge_cases.py",
    }


def edge_cases() -> list[dict]:
    cases = [
        # --- Degenerate / boundary ---
        _case("empty", "empty string", ""),
        _case("single-space", "a single space", " "),
        _case("ascii-only", "plain ASCII (must be untouched)",
              "The quick brown fox.", "The quick brown fox."),
        _case("nul", "embedded NUL byte", "a\x00b"),
        _case("all-controls", "every C0 control char",
              "".join(chr(c) for c in range(0x20))),
        _case("c1-controls", "every C1 control char",
              "".join(chr(c) for c in range(0x80, 0xA0))),
        _case("only-combining", "lone combining marks",
              "́̂̃"),
        _case("bom-only", "a lone UTF-8 BOM", "﻿"),
        _case("rtl-override", "RTL override + text", "‮abc‬"),
        _case("zero-width", "zero-width joiner soup", "a‍b‌c‍"),

        # --- Mojibake-adjacent stressors ---
        _case("mojibake-then-ascii", "mojibake glued to ASCII with no space",
              "cafÃ©menu"),
        _case("repeated-mojibake", "the same one-byte mojibake repeated 500x",
              "Ã©" * 500),
        _case("interleaved", "mojibake interleaved with clean text",
              "a Ã© b Ã¨ c Ã  d"),
        _case("nbsp-real", "real NBSP adjacent to mojibake (ftfy #157 shape)",
              "LinkÃ¶pings\xa0Universitet"),
        _case("partial-utf8-tail", "valid UTF-8 mojibake with a dangling lead byte",
              "caf\xc3"),
        # restore_byte_a0 heuristic divergence: on ambiguous "Ã "+space runs,
        # plsfix's restore_byte_a0 drops a trailing space ftfy keeps.
        _case("byte-a0-ambiguous-1", "single ambiguous Ã+space (both agree)",
              "Ã "),
        _case("byte-a0-ambiguous-2", "two ambiguous Ã+space runs (plsfix drops a space)",
              "Ã Ã "),
        _case("byte-a0-ambiguous-3", "three ambiguous Ã+space runs",
              "Ã Ã Ã "),

        # --- Quotes / HTML / entities ---
        _case("nested-entities", "doubly-escaped ampersand entity",
              "Tom &amp;amp; Jerry"),
        _case("mixed-entities", "named + numeric + hex entities together",
              "&eacute; &#233; &#xe9;"),
        _case("long-entity", "an over-long named entity (past HTML_ENTITY_RE cap)",
              "&CounterClockwiseContourIntegral;"),
        _case("bare-ampersands", "ampersands that are not entities",
              "Black & white & read all over"),
        _case("curly-already", "already-correct curly quotes (must not regress)",
              "It’s “fine”", "It’s “fine”"),

        # --- Width / ligatures / line breaks ---
        _case("fullwidth", "fullwidth Latin 'LOUD NOISES'",
              "ＬＯＵＤ　ＮＯＩＳＥＳ"),
        _case("ligatures", "Latin ligatures",
              "ﬂuﬀeﬅt"),
        _case("crlf", "Windows CRLF line breaks", "line1\r\nline2\r\n"),
        _case("lone-cr", "old-Mac lone CR line breaks", "a\rb\rc"),
        _case("mixed-breaks", "mixed CR / LF / NEL / LS",
              "a\r\nb\rc\nd\x85e f"),

        # --- Terminal escapes ---
        _case("ansi-color", "CSI color escape", "\x1b[31mred\x1b[0m"),
        _case("ansi-cursor", "CSI cursor move", "\x1b[2J\x1b[Hclear"),
        _case("osc-title", "OSC title set (not stripped by either, per README)",
              "\x1b]0;title\x07body"),

        # --- Scale ---
        _case("very-long-clean", "1 MB of clean ASCII",
              "abcdefghij" * 100_000, "abcdefghij" * 100_000),
        _case("very-long-mojibake", "200k of repeated mojibake", "Ã©Ã¨Ã " * 50_000),
        _case("many-lines", "100k short lines", "x\n" * 100_000),
    ]

    # --- Lone surrogates (ftfy #178): cannot be expressed in Rust-bound JSON.
    # Rust's String can't hold lone surrogates, so plsfix must either reject
    # the input or replace it; we record whichever it does as the observable.
    cases.append(_case(
        "lone-surrogate-low",
        "Latin-1 filename via surrogateescape: '01-as' + U+DCED + '.mp3'",
        "01-Basta de llamarme as\udced.mp3"))
    cases.append(_case(
        "lone-surrogate-high", "lone high surrogate U+D800", "a\ud800b"))
    cases.append(_case(
        "surrogate-pair-order", "low then high surrogate (wrong order)",
        "\udce5\ud800"))

    return cases
