# Fuzzing plsfix

Coverage-guided fuzzing of the `plsfix` core crate with
[`cargo-fuzz`](https://rust-fuzz.github.io/book/) (libFuzzer). These
targets hunt for panics and for violations of plsfix's own invariants;
the cross-implementation comparison against reference ftfy lives on the
Python side in [`../harness`](../harness).

## Targets

| Target | Property |
|---|---|
| `fix_text_no_panic` | `fix_text` never panics on any valid UTF-8 input. |
| `fix_and_explain_no_panic` | `fix_and_explain` never panics; for **single-segment** (line-break-free) input its `.text` equals `fix_text`. |
| `idempotent` | repeatedly applying `fix_text` **converges** to a fixed point within 16 passes (no oscillation/divergence). |
| `roundtrip_mojibake` | genuine mojibake (a seed's UTF-8 bytes reinterpreted as Latin-1) survives `fix_text` and converges. |
| `structured_config` | for an `arbitrary`-derived config (every toggle, normalization, decode length) `fix_text` never panics and converges. |

## Running

```bash
rustup toolchain install nightly        # cargo-fuzz needs nightly
cargo install cargo-fuzz

# seed the corpora from the mojibake sample set (recommended)
python ../corpus/tools/seed_fuzz.py     # or see "Seeding" below

cargo +nightly fuzz run fix_text_no_panic -- -max_total_time=60
cargo +nightly fuzz run idempotent -- -max_total_time=60
# ... etc

# minimize a crash
cargo +nightly fuzz tmin <target> fuzz/artifacts/<target>/crash-XXXX
```

The corpora under `fuzz/corpus/<target>/` are seeded from the verified
mojibake corpus so the fuzzer starts from realistic encoding-corruption
shapes rather than random bytes.

## Why "converge", not "idempotent"

The obvious invariant — `fix_text(fix_text(x)) == fix_text(x)` — is
**false**, and importantly it is false for reference ftfy too. When a line
break sits next to mojibake (e.g. `"S√µ\ré"`), the first pass normalizes
the line break and only the second pass repairs the now-adjacent MacRoman
mojibake, giving `"S√µ\né"` then `"Sõ\né"`. plsfix reproduces ftfy's output
at every pass. So the targets assert the weaker, *true* invariant:
iterating `fix_text` reaches a fixed point quickly (empirically ≤2 passes;
the targets allow 16, matching the internal `MAX_ATTEMPTS` bound) and never
oscillates.

Likewise, `fix_text` and `fix_and_explain` legitimately differ on
multi-line input: `fix_and_explain` fixes the whole string as one segment
while `fix_text` splits on lines and judges each line's encoding-badness in
isolation. ftfy does the same. The `fix_and_explain_no_panic` target
therefore only asserts equality for line-break-free input.

## Findings log

The first run of these targets (with the naive single-pass idempotence and
unconditional `fix_text == fix_and_explain` invariants) surfaced two real
behaviors, both confirmed to be **shared with ftfy exactly** rather than
plsfix regressions:

1. **Multi-pass convergence.** `fix_text` is not single-pass idempotent
   when a line break is adjacent to mojibake; a second pass is needed. ftfy
   behaves identically at every pass. The targets were corrected to assert
   bounded convergence.
2. **`fix_text` vs `fix_and_explain` on multi-line input.** They diverge
   because of segment-vs-line handling; ftfy diverges the same way. The
   target was corrected to assert equality only for single-segment input.

That plsfix matches ftfy pass-for-pass on these subtle, fuzzer-discovered
inputs is itself a strong signal of faithful replication.
