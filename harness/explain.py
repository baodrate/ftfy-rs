"""Normalize explain steps from ftfy and plsfix into a comparable form.

ftfy's ``fix_and_explain(...).explanation`` is a list of
``(action, parameter)`` tuples, e.g.::

    [('encode', 'sloppy-windows-1252'), ('decode', 'utf-8'),
     ('apply', 'uncurl_quotes')]

plsfix's ``fix_and_explain(...).steps`` is a list of flat
``transformation`` strings, e.g.::

    ['encode sloppy-windows-1252', 'decode Utf8', 'uncurl_quotes']

This module maps both onto a common token sequence so they can be diffed
meaningfully, and exposes the *apply-subsequence* (the named, semantic
transforms, ignoring transcode plumbing) which is the part that should
agree even when the two libraries record encode/decode plumbing
differently.
"""

from __future__ import annotations

# Codec spellings the two libraries use, mapped to a canonical name.
_CODEC_ALIASES = {
    "utf8": "utf-8", "utf-8": "utf-8",
    "latin-1": "latin-1", "latin1": "latin-1", "iso-8859-1": "latin-1",
    "sloppy-windows-1252": "sloppy-windows-1252",
    "windows-1252": "sloppy-windows-1252", "cp1252": "sloppy-windows-1252",
}

# Transform names that are semantically the same operation under either lib.
_APPLY_ALIASES = {
    "uncurl_quotes": "uncurl_quotes",
    "unescape_html": "unescape_html",
    "fix_c1_controls": "fix_c1_controls",
    "fix_character_width": "fix_character_width",
    "fix_latin_ligatures": "fix_latin_ligatures",
    "fix_line_breaks": "fix_line_breaks",
    "remove_control_chars": "remove_control_chars",
    "remove_terminal_escapes": "remove_terminal_escapes",
    "fix_surrogates": "fix_surrogates",
    "restore_byte_a0": "restore_byte_a0",
    "replace_lossy_sequences": "replace_lossy_sequences",
    "decode_inconsistent_utf8": "decode_inconsistent_utf8",
}

# ftfy actions that are transcode plumbing rather than named transforms.
_TRANSCODE_ACTIONS = {"encode", "decode", "transcode"}


def _canon_codec(name: str) -> str:
    return _CODEC_ALIASES.get(name.strip().lower(), name.strip().lower())


def _canon_apply(name: str) -> str:
    return _APPLY_ALIASES.get(name.strip(), name.strip())


def normalize_ftfy(explanation) -> list[tuple[str, str]]:
    """ftfy (action, param) tuples -> canonical token list."""
    out = []
    for action, param in explanation or []:
        if action in _TRANSCODE_ACTIONS:
            out.append((action, _canon_codec(str(param))))
        elif action == "apply":
            out.append(("apply", _canon_apply(str(param))))
        else:
            out.append((action, str(param)))
    return out


def normalize_plsfix(steps) -> list[tuple[str, str]]:
    """plsfix flat transformation strings -> canonical token list."""
    out = []
    for step in steps or []:
        text = step.transformation if hasattr(step, "transformation") else str(step)
        head, _, rest = text.partition(" ")
        if head in _TRANSCODE_ACTIONS and rest:
            out.append((head, _canon_codec(rest)))
        else:
            out.append(("apply", _canon_apply(text)))
    return out


def apply_subsequence(tokens: list[tuple[str, str]]) -> list[str]:
    """Just the named transforms, in order — the part that should agree."""
    return [param for action, param in tokens if action == "apply"]


def compare(ftfy_explanation, plsfix_steps) -> dict:
    """Compare two explanations. Returns a structured verdict."""
    f = normalize_ftfy(ftfy_explanation)
    p = normalize_plsfix(plsfix_steps)
    fa, pa = apply_subsequence(f), apply_subsequence(p)
    return {
        "ftfy_tokens": f,
        "plsfix_tokens": p,
        "exact_match": f == p,
        "apply_match": fa == pa,
        "ftfy_applies": fa,
        "plsfix_applies": pa,
        # plsfix is known to sometimes omit transcode plumbing it actually
        # performed; flag that specific shape so the report can bucket it.
        "plsfix_omits_transcode": (
            fa == pa
            and any(a in _TRANSCODE_ACTIONS for a, _ in f)
            and not any(a in _TRANSCODE_ACTIONS for a, _ in p)
        ),
    }
