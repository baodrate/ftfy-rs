# Mojibake corpus

A corpus of mojibake (encoding-corruption) samples for differential and
edge-case testing of `plsfix` (this repo) against the reference
implementation, `python-ftfy`.

## Design principle: every sample must round-trip

Two independent properties matter, and the corpus tracks both.

**1. Mechanical validity (the `chain`).** Real mojibake is the output of a
concrete chain of encode/decode/mangle steps applied to some intended
text. An entry earns `verified: true` only when replaying its `chain` on
its `intended` text reproduces its `mojibake` bytes **exactly**
(`tools/verify_corpus.py` enforces this and demotes any entry whose chain
drifts). This rejects garbled strings that were transcribed wrong or
imagined whole-cloth: they can't be given a working chain.

**2. Provenance (citation or justification).** Mechanical validity is
*not* sufficient to tell real-world mojibake from a convincing imitation.
This is the sharp lesson of rspeer's "Never Lose a Dead End"
(https://posts.arborelia.net/never-lose-a-dead-end/, 2024-10-31): hunting
for a real Windows-1257 example, she found `Å iaip ÄÆdomu, kaip
ÄÆsivaizduoji.` on a Lithuanian web page — only to discover the whole page
was LLM-generated, and the model had produced *fake mojibake* "because
that's what it believed Lithuanian looks like." Crucially, **that fake
mojibake round-trips just fine** (it decodes as clean UTF-8/Windows-1257);
a chain check alone cannot catch it. What separates it from the real thing
is a documented, real-world encoding-error event. So **every entry carries
a citation or a detailed justification**, and the LLM-imitated samples are
quarantined in `ai-hallucinated.json` with `provenance.real_encoding_error:
false` (this very example is included — see that file).

Provenance categories:

* **famous / documented / issue-report** — real-world, cited (Wikipedia,
  ftfy docs & issues, encoding folklore, bug trackers).
* **generated** — synthetic, *honestly labelled as such*; the seed
  sentence is cited (pangram collections, the "I Can Eat Glass" sampler,
  the Iroha, ftfy fixtures) and the corruption chain is justified. These
  are rspeer's "artificial examples": good for exhaustively testing the
  *mechanism* and codec coverage, never passed off as found-in-the-wild.
* **ai-hallucinated** — round-trips, but the source text is LLM-generated,
  not a real encoding error. The cautionary class.
* **wild-unsolved / verbatim** — chain unknown (rspeer's "mysteries",
  lossy/irreproducible issue strings). Permanently `verified: false`,
  never used as a pass/fail oracle — only "don't crash / don't make it
  worse" inputs.

### Encodings beyond ftfy's repair set

ftfy (and plsfix) only reverse UTF-8 mojibake whose mis-decode used one of
ten candidate encodings (latin-1, sloppy-windows-1250/1251/1252/1253/1254/
1257, iso-8859-2, macroman, cp437). The corpus deliberately includes
mojibake from **outside** that set — UTF-8 misread as cp850/cp852/cp866/
cp874/koi8-r/iso-8859-5/iso-8859-7/windows-1256/windows-1258, UTF-16
misreads, and CJK/legacy-to-legacy cross-decodes. These are genuine,
hand-reversible mojibake that both libraries correctly **leave unchanged**.
Each such entry is flagged `provenance.ftfy_repair_supported: false`; a
no-op is the expected, predictable result and a direct test of the
"no false positives" guarantee.

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
    ai-hallucinated.json      # LLM-imitated mojibake ("Never Lose a Dead End")
    generated-verified.json   # 60+ language seeds × corruption chains (the bulk,
                              #   incl. UTF-8-misread-as-unsupported-codec cases)
  sources/
    classic-mojibake-research.md  # provenance notes for famous/classic samples
    ftfy-issues-research.md       # provenance notes for issue-mined samples
  tools/
    transforms.py             # the chain DSL + codecs
    generate_corpus.py        # builds generated-verified.json
    build_famous.py           # builds famous-incidents.json
    build_curated.py          # builds classic-documented.json
    build_issues.py           # builds ftfy-issues.json
    build_ai_hallucinated.py  # builds ai-hallucinated.json
    seed_fuzz.py              # seeds the cargo-fuzz corpora from this corpus
    verify_corpus.py          # round-trip checker + ftfy-output refresher
```

## Regenerating / verifying

```bash
# (with a venv that has the pinned reference ftfy installed)
for b in generate_corpus build_famous build_curated build_issues build_ai_hallucinated; do
    python corpus/tools/$b.py
done
python corpus/tools/verify_corpus.py --write   # refresh verified/ftfy_* fields
```

`verify_corpus.py` (no `--write`) is what CI runs: it exits non-zero if any
entry's chain stops reproducing its mojibake, catching both accidental
corruption of the JSON and any sample that was mistakenly added without a
real chain.

## Provenance and confidence

Every sample carries either a source citation or a detailed justification
(usually both) in its `provenance` block:

* real-world entries cite a URL / issue / document, and were **re-derived**
  from their intended text and compared byte-for-byte against the cited
  string before inclusion — transcription noise and hallucinations are
  filtered out at build time, not trusted on faith;
* generated entries cite the *seed*'s origin (`provenance.seed.source`) and
  justify the corruption (`provenance.chain_rationale`), and record whether
  ftfy's codec set can even repair it (`provenance.ftfy_repair_supported`);
* ai-hallucinated entries are marked `provenance.real_encoding_error:
  false`.

The research that produced the real-world lists (including which strings
were seen verbatim vs reconstructed during web research) is recorded in
`sources/`. A quick coverage check:

```bash
python - <<'PY'
import json, glob
for f in sorted(glob.glob("corpus/entries/*.json")):
    for e in json.load(open(f)):
        p = e["provenance"]
        assert p.get("source"), e["id"]          # every entry cites a source
PY
```
