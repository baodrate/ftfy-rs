# Differential & fuzz testing: findings

Summary of what comparing `plsfix` against reference `python-ftfy` 6.3.1
surfaced, across a verified mojibake corpus, a differential harness, and
coverage-guided fuzzing. See [`corpus/`](corpus/), [`harness/`](harness/),
and [`fuzz/`](fuzz/) for the machinery.

## Headline

**plsfix tracks ftfy faithfully.** Across ~1,470 corpus cases plus
programmatic edge cases and millions of fuzz executions, there is **no
output regression** — no input where ftfy recovers the intended text and
plsfix fails to. Every divergence found is one of:

1. a **documented, intentional** difference (explanation shape, lone
   surrogates, byte-vs-codepoint decode length), or
2. a behavior **shared identically with ftfy** (multi-pass convergence,
   single-segment vs per-line handling).

Notably, the two subtle behaviors the fuzzer flagged turned out to match
ftfy *pass-for-pass and byte-for-byte*, which is strong evidence the port
reproduces ftfy's algorithm rather than merely its happy path.

## What was compared

| Observable | Result |
|---|---|
| `fix_text` output | identical on ~1,068 of 1,470 cases; the rest differ only in explanation, normalization, or documented edge cases — never a recovery regression |
| `fix_and_explain` steps | named transforms agree on most cases; plsfix records the encode/decode plumbing differently (documented "explanation shape" difference) |
| idempotence / convergence | both libraries converge in ≤2 passes; neither is single-pass idempotent (shared quirk) |
| robustness | neither panics on any corpus or fuzz input; plsfix rejects lone surrogates at the binding (documented) |
| performance | plsfix ~9–13× faster on mojibake, ~2.5–5× on clean text |

Fuzzing tally (corrected invariants, corpus-seeded): the five targets ran
clean — over **2 million executions** with zero crashes in the longer
campaign (`structured_config` 1.19M, `fix_text_no_panic` 318k,
`idempotent` 287k, `roundtrip_mojibake` 266k), on top of the initial
60s-per-target smoke pass.

## Findings in detail

### 1. Explanation shape (documented, ~594 cases)
plsfix's `fix_and_explain` often omits the `encode`/`decode` transcode
steps it actually performed, reporting only named transforms. ftfy reports
the full replayable plan. This is the documented reason plsfix can't
support `apply_plan`. Output is unaffected.

### 2. Transform accounting (documented, ~121 cases)
When outputs agree, the libraries still attribute the work to different
named steps: ftfy records `decode_inconsistent_utf8` (54×) where plsfix
folds it into its decode; plsfix records `fix_c1_controls` (104×) where
ftfy applies it without a named step. Output is identical in all of these.

### 3. `restore_byte_a0` on ambiguous input (3 edge cases)
On a degenerate run like `"Ã Ã "` — a `Ã` followed by a *literal space*
rather than the NBSP real mojibake would carry — plsfix's
`restore_byte_a0` consumes a trailing space that ftfy preserves
(`"à à"` vs `"à à "`). The input is genuinely ambiguous (no correct
answer), but it is a reproducible divergence where plsfix loses a
character. **Properly-formed** mojibake (`"Ã\xa0 Ã\xa0 "`) is recovered
identically by both. This is the one place worth a second look if exact
parity on adversarial input ever matters.

### 4. Multi-pass convergence (shared with ftfy)
`fix_text` is **not** single-pass idempotent when a line break sits next
to mojibake: `"S√µ\ré"` → `"S√µ\né"` (line break fixed) → `"Sõ\né"`
(MacRoman mojibake fixed). The fuzzer found this immediately. ftfy
produces the identical output at every pass — it is an ordering property
of the pipeline, not a plsfix bug. Both converge within 2 passes and never
oscillate. The fuzz invariant was corrected from "idempotent" to
"converges within 16 passes".

### 5. `fix_text` vs `fix_and_explain` on multi-line input (shared with ftfy)
`fix_and_explain` fixes the whole input as one segment; `fix_text` splits
on line breaks and judges each line's encoding-badness in isolation. On
multi-line input they legitimately diverge (e.g. `"Ã¥klagarmyn\n…"` →
`fix_text` leaves `Ã¥`, `fix_and_explain` fixes it to `å`). ftfy diverges
in exactly the same way. The fuzz target now asserts equality only for
single-segment (line-break-free) input.

### 6. Lone surrogates (documented)
plsfix raises at the PyO3 boundary on lone surrogates (Rust `String` can't
hold them); ftfy returns a replacement-character string. By design.

## Anti-hallucination methodology (and what the source post actually says)

The corpus guards against AI-imagined mojibake with **two** independent
checks, because — as rspeer's "Never Lose a Dead End" (2024-10-31) shows —
one is not enough:

1. **Mechanical validity.** Every sample carries an explicit
   encode/decode/mangle **chain**, and is marked `verified` only when
   replaying that chain on the intended text reproduces the garbled bytes
   *exactly*. This rejects mis-transcribed or wholly-invented strings.

2. **Provenance.** Mechanical validity is *not* sufficient. In the post,
   rspeer hunts for a real Windows-1257 example and finds `Å iaip ÄÆdomu,
   kaip ÄÆsivaizduoji.` on a Lithuanian page — then realizes the whole page
   is LLM-generated, with the model emitting *fake mojibake* "because
   that's what it believed Lithuanian looks like." That fake mojibake
   **round-trips cleanly** (we verified it: UTF-8 → Windows-1257 →
   NBSP-flatten reproduces it byte-for-byte), so a chain check alone cannot
   catch it. Only a documented real-world encoding-error event can. Hence
   every entry carries a citation or detailed justification, and the
   LLM-imitated samples — including that exact sentence — are quarantined in
   [`corpus/entries/ai-hallucinated.json`](corpus/entries/ai-hallucinated.json)
   with `real_encoding_error: false`.

Synthetic ("generated") entries are honestly labelled as such — rspeer's
"artificial examples", good for exhaustively testing the mechanism — with
each seed cited (pangram collections, the "I Can Eat Glass" sampler, the
Iroha, ftfy fixtures) and each chain justified. Research provenance notes
live in [`corpus/sources/`](corpus/sources/).

## Coverage of ftfy-unsupported encodings

ftfy and plsfix only reverse UTF-8 mojibake whose mis-decode used one of
ten candidate encodings. The corpus deliberately includes ~760 cases from
*outside* that set — UTF-8 misread as cp850/cp852/cp866/cp874/koi8-r/
iso-8859-5/iso-8859-7/windows-1256/windows-1258, UTF-16 misreads, and
CJK/legacy-to-legacy cross-decodes — each flagged
`ftfy_repair_supported: false`. Both libraries correctly **leave these
unchanged** (a no-op is the right answer), so they land in `MATCH` and
confirm the no-false-positives guarantee. One mild divergence surfaced:
on Tamil run through UTF-8-as-windows-1256, the two libraries' partial
`decode_inconsistent_utf8` processing differs slightly — neither recovers
the Tamil (both emit garbage), so it is bucketed `NEITHER_ORACLE`, not a
regression.

Separately, the "Baltic gap" the plsfix README used to list is **closed**:
plsfix's candidate set now matches ftfy's (it includes `sloppy-windows-1257`),
and clean Lithuanian/Latvian mojibake (`Sąrašai`, `Žalgiris`, `Rīga`) is
recovered identically by both. The stale README note was removed.
