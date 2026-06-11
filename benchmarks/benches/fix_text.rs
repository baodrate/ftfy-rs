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

// CI overrides these soft limits via `GUNGRAUN_CALLGRIND_LIMITS` —
// the values below are the local-run default.
main!(
    config = LibraryBenchmarkConfig::default().tool(
        Callgrind::default()
            .soft_limits([(EventKind::Ir, 1.0), (EventKind::EstimatedCycles, 2.0)])
            .flamegraph(FlamegraphConfig::default())
    ),
    library_benchmark_groups = [run]
);
