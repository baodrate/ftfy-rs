"""Transform DSL for reproducing mojibake from intended text.

A corpus entry's ``chain`` is a list of steps applied to the *intended*
string. If applying the chain yields the entry's ``mojibake`` string
exactly, the entry is mechanically verified: the garbled text really is
the result of a concrete, nameable corruption process, not a
hallucinated imitation of one.

Step kinds
----------
``{"encode": CODEC}``                 str -> bytes
``{"encode": CODEC, "errors": MODE}``
``{"decode": CODEC}``                 bytes -> str
``{"decode": CODEC, "errors": MODE}``
``{"transform": NAME}``               str -> str lossy mangling
``{"replace": [OLD, NEW]}``           str -> str targeted substitution
``{"bytes": "drop_last"}``            bytes -> bytes truncation mid-character

Codecs may be any Python codec plus ftfy's ``sloppy-*`` and
``utf-8-variants`` codecs (registered by importing ``ftfy.bad_codecs``),
which is how real-world decoders behave (no byte is ever undefined).
"""

from __future__ import annotations

import html
import re

import ftfy.bad_codecs  # noqa: F401  (registers sloppy-* and utf-8-variants)

# fmt: off
TRANSFORMS = {
    # U+00A0 no-break space flattened to ASCII space (common in HTML pipelines)
    "nbsp_to_space":   lambda s: s.replace(" ", " "),
    # U+00A0 dropped entirely
    "nbsp_to_nothing": lambda s: s.replace(" ", ""),
    # "smart quotes" applied on top of mojibake by a blogging/word-processing tool
    "curl_apostrophe": lambda s: s.replace("'", "’"),
    "curl_quotes":     lambda s: _curl_double_quotes(s.replace("'", "’")),
    # HTML-escape pass (entities end up visible in plain text)
    "html_escape":     lambda s: html.escape(s),
    "html_named":      lambda s: _entity_encode(s),
    "html_unescape":   lambda s: html.unescape(s),
    # whitespace squeezed by an HTML renderer
    "collapse_spaces": lambda s: re.sub(" {2,}", " ", s),
    # lone trailing spaces stripped by a CMS
    "strip":           lambda s: s.strip(),
    # an all-caps pass (label printers, SHOUTING databases)
    "uppercase":       lambda s: s.upper(),
    # U+00AF written as its named entity (seen in the ftfy shrug example)
    "macron_entity":   lambda s: s.replace("¯", "&macr;"),
    # non-ASCII written as literal \uXXXX escapes (ftfy issue #128)
    "u_escape_upper":  lambda s: "".join(
        ch if ord(ch) < 128 else f"\\u{ord(ch):04X}" for ch in s),
}

BYTES_OPS = {
    "drop_last":  lambda b: b[:-1],
    "drop_first": lambda b: b[1:],
}
# fmt: on


def _curl_double_quotes(s: str) -> str:
    out = []
    open_q = True
    for ch in s:
        if ch == '"':
            out.append("“" if open_q else "”")
            open_q = not open_q
        else:
            out.append(ch)
    return "".join(out)


def _entity_encode(s: str) -> str:
    """Encode non-ASCII as named entities where possible, else numeric."""
    out = []
    for ch in s:
        if ord(ch) < 128:
            out.append(html.escape(ch) if ch in "&<>" else ch)
        else:
            name = html.entities.codepoint2name.get(ord(ch))
            out.append(f"&{name};" if name else f"&#{ord(ch)};")
    return "".join(out)


def cesu8_encode(s: str) -> bytes:
    """Encode like broken Java/Oracle stacks do: astral characters become a
    UTF-16 surrogate pair with each surrogate UTF-8-encoded (6 bytes)."""
    out = bytearray()
    for ch in s:
        cp = ord(ch)
        if cp >= 0x10000:
            cp -= 0x10000
            hi, lo = 0xD800 + (cp >> 10), 0xDC00 + (cp & 0x3FF)
            for surrogate in (hi, lo):
                out += chr(surrogate).encode("utf-8", "surrogatepass")
        else:
            out += ch.encode("utf-8")
    return bytes(out)


def apply_chain(intended, chain: list[dict]) -> str:
    """Apply a corruption chain to the intended text; returns the mojibake.

    ``intended`` may be ``None`` if the chain begins with a ``raw_bytes_hex``
    seed step that supplies its own starting bytes.
    """
    value = intended
    for i, step in enumerate(chain):
        try:
            if "encode" in step:
                assert isinstance(value, str), f"step {i}: encode needs str"
                if step["encode"] == "cesu-8":
                    value = cesu8_encode(value)
                else:
                    value = value.encode(step["encode"], step.get("errors", "strict"))
            elif "decode" in step:
                assert isinstance(value, bytes), f"step {i}: decode needs bytes"
                value = value.decode(step["decode"], step.get("errors", "strict"))
            elif "transform" in step:
                assert isinstance(value, str), f"step {i}: transform needs str"
                value = TRANSFORMS[step["transform"]](value)
            elif "replace" in step:
                assert isinstance(value, str), f"step {i}: replace needs str"
                old, new = step["replace"]
                value = value.replace(old, new)
            elif "bytes" in step:
                assert isinstance(value, bytes), f"step {i}: bytes op needs bytes"
                value = BYTES_OPS[step["bytes"]](value)
            elif "raw_bytes_hex" in step:
                # Discard the (synthetic) running value and seed raw bytes.
                value = bytes.fromhex(step["raw_bytes_hex"])
            else:
                raise ValueError(f"step {i}: unknown step kind {step!r}")
        except (UnicodeError, LookupError) as exc:
            raise ChainError(f"step {i} ({step!r}) failed: {exc}") from exc
    assert isinstance(value, str), "chain must end with a str"
    return value


class ChainError(Exception):
    pass
