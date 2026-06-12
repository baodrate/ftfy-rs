// Benchmark for fix_text, weighted toward mojibake that exercises
// restore_byte_a0 / replace_lossy_sequences in the encoding repair loop.
use std::time::Instant;

use plsfix::fix_text;

fn corpus() -> Vec<&'static str> {
    // ~50 strings, heavily weighted toward mojibake.
    let mojibake: &[&str] = &[
        "ÄŒeÅ¡tina",
        "GÃ idhlig",
        "LietuviÅ³",
        "SlovenÄ�ina",
        "Tiáº¿ng Viá»‡t",
        "Î•Î»Î»Î·Î½Î¹ÎºÎ¬",
        "Ð±ÑŠÐ»Ð³Ð°Ñ€Ñ�ÐºÐ¸ ÐµÐ·Ð¸Ðº",
        "Ð ÑƒÑ�Ñ�ÐºÐ¸Ð¹",
        "CÑ€Ð¿Ñ�ÐºÐ¸ [Ñ›Ð¸Ñ€Ð¸Ð»Ð¸Ñ†Ð¾Ð¼]",
        "×¢×‘×¨×™×ª",
        "à¤¹à¤¿à¤¨à¥�à¤¦à¥€",
        "à®¤à®®à®¿à®´à¯�",
        "à¸ à¸²à¸©à¸²à¹„à¸—à¸¢",
        "ç®€ä½“ä¸­æ–‡",
        "æ­£é«”ä¸­æ–‡",
        "æ—¥æœ¬èªž",
        "í•œêµ­ì–´",
        "He's Justinâ¤",
        "Le Schtroumpf Docteur conseille g√¢teaux et baies schtroumpfantes pour un r√©gime √©quilibr√©.",
        "âœ” No problems",
        "РґРѕСЂРѕРіРµ РР·-РїРѕРґ #С„СѓС‚Р±РѕР»",
        "Handwerk bringt dich überall hin: Von der YOU bis nach Monaco",
        "schÃ¶n",
        "FÃ©lix",
        "naÃ¯ve",
        "rÃ©sumÃ©",
        "ãƒ†ã‚¹ãƒˆ",
        "â€œhello â€� world",
        "donâ€™t",
        "â€“ dash",
        "Â£100",
        "Â© 2025",
        "Atenciá»‡n",
        "EspaÃ±ol",
        "PortuguÃªs",
        "FranÃ§ais",
        "â€¦ ellipsis",
        "Mëtàl Ümlauts already correct",
        "ÅšlÄ…sk",
        "MalmÃ¶",
        "ZÃ¼rich",
        // a few clean strings so the bench reflects mixed input
        "this is a perfectly fine ASCII string",
        "déjà vu — already correct UTF-8",
        "正體中文 already correct",
        "Здравствуйте already correct",
        "<p>hello &amp; world</p>",
        "smart quote: \u{201C}hi\u{201D}",
        "ligature: ﬁnish",
        "control\u{0007}bell",
    ];
    mojibake.to_vec()
}

fn run_once(corpus: &[&str]) -> usize {
    // Return total fixed-bytes so the optimizer can't elide the call.
    corpus
        .iter()
        .map(|s| fix_text(s, None).len())
        .fold(0usize, usize::wrapping_add)
}

fn main() {
    let corpus = corpus();
    println!("corpus size: {} strings", corpus.len());

    // Cold: time the very first invocation across the corpus.
    let t0 = Instant::now();
    let cold_acc = run_once(&corpus);
    let cold = t0.elapsed();
    println!("cold:  {:>10} ns total, acc={}", cold.as_nanos(), cold_acc);

    // Warm: 10_000 iterations.
    let iters: u64 = 10_000;
    // Warmup
    let warmup_acc = (0..1_000)
        .map(|_| run_once(&corpus))
        .fold(0usize, usize::wrapping_add);
    let t0 = Instant::now();
    let acc = (0..iters)
        .map(|_| run_once(&corpus))
        .fold(warmup_acc, usize::wrapping_add);
    let warm = t0.elapsed();
    let ns_per_iter = warm.as_nanos() as f64 / iters as f64;
    let ns_per_string = ns_per_iter / corpus.len() as f64;
    println!(
        "warm:  {:>10} ns/iter ({:.1} ns/string over {} strings), acc={}",
        ns_per_iter as u64,
        ns_per_string,
        corpus.len(),
        acc
    );
}
