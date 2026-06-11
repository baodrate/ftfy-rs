use gungraun::prelude::*;
use gungraun::{Callgrind, EventKind, FlamegraphConfig};
use plsfix::fix_text;
use std::hint::black_box;

// ~60-byte sample blocks. Each exercises a different mix of the plsfix pipeline:
// CLEAN should hit the all-ASCII / `is_bad` fast paths, MOJIBAKE drives the
// encoding-repair loop, and MIXED also fires HTML-entity unescaping, curly-quote
// fixing, and the CRLF line-break normaliser.
const CLEAN: &str = "Hello, world! The quick brown fox jumps over the lazy dog.\n";
const MOJIBAKE: &str = "voilÃ  le travail. Ø±Ø³Ø§Ù„Ø© rsÃ¡lka.\n";
const MIXED: &str = "voilÃ  &amp; \u{201C}friends\u{201D}\u{2014}see &lt;a&gt;.\r\n";

fn make_clean(repeats: usize) -> String {
    CLEAN.repeat(repeats)
}

fn make_mojibake(repeats: usize) -> String {
    MOJIBAKE.repeat(repeats)
}

fn make_mixed(repeats: usize) -> String {
    MIXED.repeat(repeats)
}

// Cold start: a single small input. Per-call overhead (regex priming, pipeline
// setup, segmentation) dominates, since each gungraun run starts a fresh process.
#[library_benchmark]
#[bench::clean(args = [1], setup = make_clean)]
#[bench::mojibake(args = [1], setup = make_mojibake)]
#[bench::mixed(args = [1], setup = make_mixed)]
fn cold(input: String) -> String {
    black_box(fix_text(black_box(&input), None))
}

// Warm start: a ~60 kB input. Steady-state per-byte processing dominates,
// amortising the one-shot startup cost so we can see throughput regressions.
#[library_benchmark]
#[bench::clean(args = [1_000], setup = make_clean)]
#[bench::mojibake(args = [1_000], setup = make_mojibake)]
#[bench::mixed(args = [1_000], setup = make_mixed)]
fn warm(input: String) -> String {
    black_box(fix_text(black_box(&input), None))
}

library_benchmark_group!(name = run, benchmarks = [cold, warm]);

// Soft limits make `cargo bench` exit non-zero when a benchmark regresses past
// the threshold. CI overrides these via `GUNGRAUN_CALLGRIND_LIMITS` so the
// threshold knobs live in `.github/workflows/benchmark.yml`; the values below
// are the local-run default. Measured locally against the base ref:
//
//   Ir on identical code         ≤ 0.0005%
//   Ir from an inert black_box   ≤ 0.0003%
//   Ir from one per-byte fold    0.06 – 0.23%
//   Ir from 50× per-byte fold    2.96 – 11.46%  (regressed, as intended)
//
// Cycles fold in cache modelling and shift ±0.2% from layout alone, so the
// Cycles limit is loosened to 2× Ir — still far below any real regression
// (the 50× fold above pushed Cycles to +9.7%).
//
// FlamegraphConfig::default() also emits a regular and a differential
// flamegraph per bench under target/gungraun/**/*.svg, which CI uploads as
// an artefact.
main!(
    config = LibraryBenchmarkConfig::default().tool(
        Callgrind::default()
            .soft_limits([(EventKind::Ir, 1.0), (EventKind::EstimatedCycles, 2.0)])
            .flamegraph(FlamegraphConfig::default())
    ),
    library_benchmark_groups = [run]
);
