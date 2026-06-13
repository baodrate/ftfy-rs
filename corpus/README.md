# Mojibake corpus

A corpus of mojibake (encoding-corruption) samples for differential and
edge-case testing of `plsfix` (this repo) against the reference
implementation, `python-ftfy`.

## Design principle: every sample must round-trip

The central rule, borrowed from how ftfy itself works and from rspeer's
warning about AI-hallucinated mojibake ("Never lose a dead end"): **real
mojibake is mechanically invertible.** A genuine garbled string is the
output of a concrete chain of encode/decode/mangle steps applied to some
real intended text. A string that merely *looks* like mojibake — for
instance one an LLM invented — will almost never decode back through a
real codec chain.

So an entry earns `verified: true` only when replaying its `chain` on its
`intended` text reproduces its `mojibake` bytes **exactly**. The verifier
(`tools/verify_corpus.py`) enforces this and will demote any entry whose
chain drifts. This is the corpus's built-in hallucination filter: a
fabricated sample cannot be given a working chain, so it can never pass.

Samples whose chain is unknown (rspeer's unsolved "mysteries", a couple of
verbatim-from-issue strings where the corruption was lossy/irreproducible)
are kept for realism but are permanently `verified: false` and are never
used as pass/fail oracles — only as "don't crash / don't make it worse"
inputs.

## Entry schema

```jsonc
{
  "id": "famous-bnopnya",            // stable unique id
  "label": "...",                    // human description
  "lang": "ru",                      // BCP-47-ish tag, or "und"/"emoji"/"mixed"
  "intended": "Вопрос",              // the original correct text (null if unknown)
  "mojibake": "бНОПНЯ",              // the garbled input to feed the fixers
  "chain": [                         // how mojibake was produced from intended
    {"encode": "windows-1251"},
    {"decode": "koi8-r"}
  ],
  "provenance": {
    "type": "famous-incident",       // generated | documented | issue-report | ...
    "source": "https://...",         // citation
    "notes": "..."
  },
  "verified": true,                  // chain reproduces mojibake exactly
  "ftfy_output": "Вопрос",           // what reference ftfy produces (cached)
  "ftfy_recovers": true              // ftfy_output == NFC(intended)? (null if N/A)
}
```

### Chain step kinds (see `tools/transforms.py`)

| Step | Effect |
|---|---|
| `{"encode": CODEC}` / `{"encode": CODEC, "errors": MODE}` | str → bytes |
| `{"decode": CODEC}` / `{"decode": CODEC, "errors": MODE}` | bytes → str |
| `{"transform": NAME}` | str → str mangling (NBSP flattening, quote curling, HTML escaping, uppercasing, …) |
| `{"replace": [OLD, NEW]}` | targeted lossy substitution (e.g. `’`→`¿`) |
| `{"bytes": "drop_last"\|"drop_first"}` | truncate bytes mid-character |
| `{"raw_bytes_hex": "cccccc"}` | seed raw bytes (memory-fill / BOM artifacts) |

`CODEC` may be any Python codec plus ftfy's `sloppy-windows-*` and
`cesu-8` (the latter implemented in `transforms.py` for Java/Oracle-style
surrogate pairs). `sloppy-*` codecs never reject a byte, which is how real
broken decoders behave.

## Files

```
corpus/
  entries/
    famous-incidents.json     # бНОПНЯ, 锟斤拷, Bush hid the facts, ALA fungi, …
    classic-documented.json   # ftfy docs/README examples + rspeer's mysteries
    ftfy-issues.json          # real user reports from rspeer/python-ftfy issues
    generated-verified.json   # 40+ language seeds × corruption chains (the bulk)
  sources/
    classic-mojibake-research.md  # provenance notes for famous/classic samples
    ftfy-issues-research.md       # provenance notes for issue-mined samples
  tools/
    transforms.py             # the chain DSL + codecs
    generate_corpus.py        # builds generated-verified.json
    build_famous.py           # builds famous-incidents.json
    build_curated.py          # builds classic-documented.json
    build_issues.py           # builds ftfy-issues.json
    verify_corpus.py          # round-trip checker + ftfy-output refresher
```

## Regenerating / verifying

```bash
# (with a venv that has the pinned reference ftfy installed)
python corpus/tools/generate_corpus.py
python corpus/tools/build_famous.py
python corpus/tools/build_curated.py
python corpus/tools/build_issues.py
python corpus/tools/verify_corpus.py --write   # refresh verified/ftfy_* fields
```

`verify_corpus.py` (no `--write`) is what CI runs: it exits non-zero if any
entry's chain stops reproducing its mojibake, catching both accidental
corruption of the JSON and any sample that was mistakenly added without a
real chain.

## Provenance and confidence

Every sample cites a source. Famous/classic strings were gathered from
documentation and encoding folklore, then **re-derived** from their
intended text and compared byte-for-byte against the cited string before
inclusion — transcription noise and hallucinations are filtered out at
build time, not trusted on faith. The research that produced these lists
(including which strings were seen verbatim vs reconstructed) is recorded
in `sources/`.
