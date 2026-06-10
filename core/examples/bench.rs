use plsfix::fix_text;
use std::time::Instant;

fn corpus() -> Vec<&'static str> {
    vec![
        // ASCII / clean — these hit the fast path
        "Hello, world!",
        "The quick brown fox jumps over the lazy dog.",
        "abcdefghijklmnopqrstuvwxyz0123456789",
        "Just plain ASCII text without anything weird.",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
        "Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
        "Ut enim ad minim veniam, quis nostrud exercitation ullamco.",
        "Duis aute irure dolor in reprehenderit in voluptate velit esse.",
        "Excepteur sint occaecat cupidatat non proident, sunt in culpa.",
        "Programming languages: Rust, Python, Go, JavaScript, TypeScript.",
        "Numbers: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 100, 1000.",
        "Punctuation, semicolons; colons: and dashes - are fine.",
        "Question marks? Exclamation points! Periods. Commas, too.",
        "Single quotes 'simple' and double quotes \"simple\" are ASCII.",
        "Math: 2 + 2 = 4, 3 * 7 = 21, 10 / 2 = 5.",
        "URLs like https://example.com/path?q=1&r=2 are ASCII.",
        "Emails: user@example.com, foo.bar+baz@sub.example.org.",
        "File paths: /usr/local/bin/program, C:\\Users\\name.",
        "Code snippets: fn main() { println!(\"hi\"); }",
        "JSON: {\"key\": \"value\", \"number\": 42, \"bool\": true}",
        "HTML-ish content without entities: <b>bold</b> <i>italic</i>",
        "Logs: 2024-01-01 12:34:56 INFO Starting service",
        "Markdown: # Header\n\nParagraph with **bold** and _italic_.",
        "Plain English sentences with normal punctuation throughout.",
        "Another clean ASCII string of moderate length here.",
        // Mojibake — these go through the slow path
        "ÄŒeÅ¡tina",
        "GÃ idhlig",
        "LietuviÅ³",
        "cafÃ©",
        "naÃ¯ve",
        "FranÃ§ais",
        "EspaÃ±ol",
        "PortuguÃªs",
        "Â£100",
        "Â¡Hola!",
        "rÃ©sumÃ©",
        "Ã©lÃ¨ve",
        "BjÃ¶rk",
        "Ã¼ber",
        "schÃ¶n",
        "MÃ¼nchen",
        "ZÃ¼rich",
        "DÃ¼sseldorf",
        "KÃ¶ln",
        "â€œquotedâ€",
        "â€“ dash â€”",
        "â€¦ ellipsis",
        "donâ€™t",
        "â€˜singleâ€™",
        "weâ€™re",
    ]
}

fn time_once(corpus: &[&str]) -> u128 {
    let start = Instant::now();
    for s in corpus {
        let out = fix_text(s, None);
        std::hint::black_box(out);
    }
    start.elapsed().as_nanos()
}

fn main() {
    let corpus = corpus();
    let n_strings = corpus.len();

    // Cold: first call
    let cold = time_once(&corpus);
    println!(
        "cold: {} ns total, {} ns/string",
        cold,
        cold / n_strings as u128
    );

    // Warm
    let iterations = 10_000usize;
    let start = Instant::now();
    for _ in 0..iterations {
        for s in &corpus {
            let out = fix_text(s, None);
            std::hint::black_box(out);
        }
    }
    let elapsed = start.elapsed().as_nanos();
    let per_call = elapsed / (iterations as u128 * n_strings as u128);
    println!(
        "warm: {} ns total, {} ns/call ({} strings x {} iterations)",
        elapsed, per_call, n_strings, iterations
    );
}
