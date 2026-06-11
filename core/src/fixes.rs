use crate::{
    badness::is_bad,
    chardata::{
        is_control_char, ALTERED_UTF8_RE, C1_CONTROL_RE, DOUBLE_QUOTE_RE, HTML_ENTITY_RE,
        LIGATURES, LOSSY_UTF8_RE, SINGLE_QUOTE_RE, UTF8_CONTINUATION_STRICT_SET, UTF8_DETECTOR_RE,
        WIDTH_MAP,
    },
    codecs::sloppy::{Codec, LATIN_1, SLOPPY_WINDOWS_1252},
    fix_encoding_and_explain,
    html_entities::lookup_upper_alias,
};
use regex::{Regex, Replacer};
use std::borrow::Cow;

fn _unescape_fixup(capture: &regex::Captures) -> String {
    /*
    Replace one matched HTML entity with the character it represents,
    if possible.
    */
    let text = capture.get(0).map_or("", |m| m.as_str());
    // Check the compile-time ALL-CAPS overlay first. Falls through to
    // `htmlize` (WHATWG named entities + § 13.2.5.80 numeric refs).
    if let Some(val) = lookup_upper_alias(text) {
        return val.to_string();
    }
    let unescaped = htmlize::unescape(text);
    if unescaped.as_ref() != text {
        if unescaped.contains(';') {
            return text.to_string();
        }
        return unescaped.into_owned();
    }
    text.to_string()
}

pub fn unescape_html(text: &str) -> Cow<str> {
    /*
    Decode HTML entities and character references, including some nonstandard
    ones written in all-caps.

    In this function, we decode the escape sequences that appear in the
    `html.entities.html5` dictionary from Python, as long as they are the unambiguous ones
    that end in semicolons.

    We also decode all-caps versions of Latin letters and common symbols.
    If a database contains the name 'P&EACUTE;REZ', we can read that and intuit
    that it was supposed to say 'PÉREZ'. This is limited to a smaller set of
    entities, because there are many instances where entity names are
    case-sensitive in complicated ways.

    */
    HTML_ENTITY_RE.replace_all(&text, |caps: &regex::Captures| _unescape_fixup(caps))
}

lazy_static! {
    static ref ANSI_RE: regex::Regex = regex::Regex::new("\x1b\\[((?:\\d|;)*)([a-zA-Z])").unwrap();
}

/// Extension methods for `Regex` that operate on `Cow<str>` instead of `&str`.
pub trait RegexCowExt {
    /// [`Regex::replace`], but taking text as `Cow<str>` instead of `&str`.
    fn replace_cow<'t, R: Replacer>(&self, text: Cow<'t, str>, rep: R) -> Cow<'t, str>;

    /// [`Regex::replace_all`], but taking text as `Cow<str>` instead of `&str`.
    fn replace_all_cow<'t, R: Replacer>(&self, text: Cow<'t, str>, rep: R) -> Cow<'t, str>;

    /// [`Regex::replacen`], but taking text as `Cow<str>` instead of `&str`.
    fn replacen_cow<'t, R: Replacer>(
        &self,
        text: Cow<'t, str>,
        limit: usize,
        rep: R,
    ) -> Cow<'t, str>;
}

impl RegexCowExt for Regex {
    fn replace_cow<'t, R: Replacer>(&self, text: Cow<'t, str>, rep: R) -> Cow<'t, str> {
        match self.replace(&text, rep) {
            Cow::Owned(result) => Cow::Owned(result),
            Cow::Borrowed(_) => text,
        }
    }

    fn replace_all_cow<'t, R: Replacer>(&self, text: Cow<'t, str>, rep: R) -> Cow<'t, str> {
        match self.replace_all(&text, rep) {
            Cow::Owned(result) => Cow::Owned(result),
            Cow::Borrowed(_) => text,
        }
    }

    fn replacen_cow<'t, R: Replacer>(
        &self,
        text: Cow<'t, str>,
        limit: usize,
        rep: R,
    ) -> Cow<'t, str> {
        match self.replacen(&text, limit, rep) {
            Cow::Owned(result) => Cow::Owned(result),
            Cow::Borrowed(_) => text,
        }
    }
}

pub fn remove_terminal_escapes(text: &str) -> Cow<str> {
    /*
    Strip out "ANSI" terminal escape sequences, such as those that produce
    colored text on Unix.
    */
    ANSI_RE.replace_all(&text, "")
}

pub fn uncurl_quotes(text: &str) -> Cow<str> {
    /*
    Replace curly quotation marks with straight equivalents.
    */
    SINGLE_QUOTE_RE
        .replace_all_cow(DOUBLE_QUOTE_RE.replace_all(&text, "\""), "'")
        .into()
}

pub fn fix_latin_ligatures(text: &str) -> Cow<str> {
    /*
    Replace single-character ligatures of Latin letters, such as 'ﬁ', with the
    characters that they contain, as in 'fi'. Latin ligatures are usually not
    intended in text strings (though they're lovely in *rendered* text).  If
    you have such a ligature in your string, it is probably a result of a
    copy-and-paste glitch.

    We leave ligatures in other scripts alone to be safe. They may be intended,
    and removing them may lose information. If you want to take apart nearly
    all ligatures, use NFKC normalization.
    */
    if text.chars().any(|ch| LIGATURES.get(&(ch as u32)).is_some()) {
        let mut result = String::new();

        for ch in text.chars() {
            match LIGATURES.get(&(ch as u32)) {
                Some(replacement) => result.push_str(replacement),
                None => result.push(ch),
            }
        }

        Cow::Owned(result)
    } else {
        Cow::Borrowed(text)
    }
}

pub fn fix_character_width(text: &str) -> Cow<str> {
    /*
    The ASCII characters, katakana, and Hangul characters have alternate
    "halfwidth" or "fullwidth" forms that help text line up in a grid.

    If you don't need these width properties, you probably want to replace
    these characters with their standard form, which is what this function
    does.

    Note that this replaces the ideographic space, U+3000, with the ASCII
    space, U+20.
    */
    if !text.chars().any(|ch| WIDTH_MAP.contains_key(&(ch as u32))) {
        return Cow::Borrowed(text);
    }

    let mut result = String::new();

    for ch in text.chars() {
        match WIDTH_MAP.get(&(ch as u32)) {
            Some(replacement) => result.push(*replacement),
            None => result.push(ch),
        }
    }

    Cow::Owned(result)
}

lazy_static! {
    static ref LINE_BREAK_RE: Regex = Regex::new(r"\r\n|\r|\u{2028}|\u{2029}|\u{0085}").unwrap();
}

pub fn fix_line_breaks(text: &str) -> Cow<str> {
    /*
    Convert all line breaks to Unix style.

    This will convert the following sequences into the standard \\n
    line break:

    - CRLF (\\r\\n), used on Windows and in some communication protocols
    - CR (\\r), once used on Mac OS Classic, and now kept alive by misguided
      software such as Microsoft Office for Mac
    - LINE SEPARATOR (\\u2028) and PARAGRAPH SEPARATOR (\\u2029), defined by
      Unicode and used to sow confusion and discord
    - NEXT LINE (\\x85), a C1 control character that is certainly not what you
      meant

    The NEXT LINE character is a bit of an odd case, because it
    usually won't show up if `fix_encoding` is also being run.
    \\x85 is very common mojibake for \\u2026, HORIZONTAL ELLIPSIS.
    */
    LINE_BREAK_RE.replace_all(text, "\n")
}

pub fn remove_control_chars(text: &str) -> Cow<str> {
    /*
    Remove various control characters that you probably didn't intend to be in
    your text. Many of these characters appear in the table of "Characters not
    suitable for use with markup" at
    http://www.unicode.org/reports/tr20/tr20-9.html.

    This includes:

    - ASCII control characters, except for the important whitespace characters
      (U+00 to U+08, U+0B, U+0E to U+1F, U+7F)
    - Deprecated Arabic control characters (U+206A to U+206F)
    - Interlinear annotation characters (U+FFF9 to U+FFFB)
    - The Object Replacement Character (U+FFFC)
    - The byte order mark (U+FEFF)

    However, these similar characters are left alone:

    - Control characters that produce whitespace (U+09, U+0A, U+0C, U+0D,
      U+2028, and U+2029)
    - C1 control characters (U+80 to U+9F) -- even though they are basically
      never used intentionally, they are important clues about what mojibake
      has happened
    - Control characters that affect glyph rendering, such as joiners and
      right-to-left marks (U+200C to U+200F, U+202A to U+202E)
    - Musical notation control characters (U+1D173 to U+1D17A) because wow if
      you're using those you probably have a good reason
    - Tag characters, because they are now used in emoji sequences such as
      "Flag of Wales"
      */
    if let Some((idx, _)) = text.char_indices().find(|(_, ch)| is_control_char(*ch)) {
        let mut result = String::with_capacity(text.len());
        result.push_str(&text[..idx]);
        result.extend(text[idx..].chars().filter(|ch| !is_control_char(*ch)));
        Cow::Owned(result)
    } else {
        Cow::Borrowed(text)
    }
}

lazy_static! {
    /*
    This regex implements an exception to restore_byte_a0, so we can decode the
    very common mojibake of (for example) "Ã la mode" as "à la mode", not "àla
    mode".

    If byte C3 appears with a single space after it -- most commonly this shows
    up as " Ã " appearing as an entire word -- we'll insert \xa0 while keeping
    the space. Without this change, we would decode "à" as the start of the next
    word, such as "àla". It's almost always intended to be a separate word, as in
    "à la", but when mojibake turns this into "Ã\xa0 la", the two kinds of spaces
    get coalesced into "Ã la".

    We make exceptions for the Portuguese words "às", "àquele", "àquela",
    "àquilo" and their plurals -- these are contractions of, for example, "a
    aquele" and are very common. Note that the final letter is important to
    distinguish this case from French "à quel point".

    Other instances in Portuguese, such as "àfrica", seem to be typos (intended
    to be "África" with the accent in the other direction).

    Unfortunately, "à" is a common letter in Catalan, and mojibake of words that
    contain it will end up with inserted spaces. We can't do the right thing with
    every word. The cost is that the mojibake text "fÃ cil" will be interpreted as
    "fà cil", not "fàcil".
    */
    static ref A_GRAVE_WORD_RE: regex::bytes::Regex =
        regex::bytes::Regex::new(r"(?-u:\xc3 [^ ]* ?)").unwrap();

    static ref A_GRAVE_NEGATIVE_RE: regex::bytes::Regex =
        regex::bytes::Regex::new(r"^(?-u:\xc3 ( |quele|quela|quilo|s ))").unwrap();
}

pub fn restore_byte_a0(byts: &[u8]) -> Vec<u8> {
    /*
    Some mojibake has been additionally altered by a process that said "hmm,
    byte A0, that's basically a space!" and replaced it with an ASCII space.
    When the A0 is part of a sequence that we intend to decode as UTF-8,
    changing byte A0 to 20 would make it fail to decode.

    This process finds sequences that would convincingly decode as UTF-8 if
    byte 20 were changed to A0, and puts back the A0. For the purpose of
    deciding whether this is a good idea, this step gets a cost of twice
    the number of bytes that are changed.

    This is used as a step within `fix_encoding`.
    */
    let byts = A_GRAVE_WORD_RE.replace_all(&byts, |captures: &regex::bytes::Captures| {
        let mut result = captures[0].to_owned(); // Clone the captured bytes

        if A_GRAVE_NEGATIVE_RE.is_match(&result) {
            return result;
        }

        result[0] = b'\xc3';
        result[1] = b'\xa0';
        result.insert(2, b' ');

        result
    });

    let byts = ALTERED_UTF8_RE.replace_all(&byts, |captures: &regex::bytes::Captures| {
        let mut result = captures[0].to_owned(); // Clone the captured bytes
        for byte in &mut result {
            if *byte == b'\x20' {
                *byte = b'\xa0';
            }
        }
        result
    });

    byts.to_vec()
}

pub fn replace_lossy_sequences(byts: &Vec<u8>) -> Vec<u8> {
    /*
    This function identifies sequences where information has been lost in
    a "sloppy" codec, indicated by byte 0x1A, and if they would otherwise look
    like a UTF-8 sequence, it replaces them with the UTF-8 sequence for U+FFFD.

    A further explanation:

    plsfix can now fix text in a few cases that it would previously fix
    incompletely, because of the fact that it can't successfully apply the fix
    to the entire string. A very common case of this is when characters have
    been erroneously decoded as windows-1252, but instead of the "sloppy"
    windows-1252 that passes through unassigned bytes, the unassigned bytes get
    turned into U+FFFD (�), so we can't tell what they were.

    This most commonly happens with curly quotation marks that appear
    ``â€œ like this â€�``.

    We can do better by building on plsfix's "sloppy codecs" to let them handle
    less-sloppy but more-lossy text. When they encounter the character ``�``,
    instead of refusing to encode it, they encode it as byte 1A -- an
    ASCII control code called SUBSTITUTE that once was meant for about the same
    purpose. We can then apply a fixer that looks for UTF-8 sequences where
    some continuation bytes have been replaced by byte 0x1A, and decode the whole
    sequence as �; if that doesn't work, it'll just turn the byte back into �
    itself.

    As a result, the above text ``â€œ like this â€�`` will decode as
    ``“ like this �``.

    If U+1A was actually in the original string, then the sloppy codecs will
    not be used, and this function will not be run, so your weird control
    character will be left alone but wacky fixes like this won't be possible.

    This is used as a transcoder within `fix_encoding`.
    */
    let replace_content = "\u{FFFD}".as_bytes().to_owned();
    LOSSY_UTF8_RE
        .replace_all(&byts[..], replace_content)
        .to_vec()
}

pub fn decode_inconsistent_utf8(text: &str) -> Cow<str> {
    /*
    Sometimes, text from one encoding ends up embedded within text from a
    different one. This is common enough that we need to be able to fix it.

    This is used as a transcoder within `fix_encoding`.
    */
    let mut matches = accepted_utf8_matches(text).peekable();
    if matches.peek().is_none() {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut last_end = 0;
    for mat in matches {
        let substr = mat.as_str();
        out.push_str(&text[last_end..mat.start()]);
        if substr.len() < text.len() && is_bad(substr) {
            out.push_str(&fix_encoding_and_explain(substr, false, None).text);
        } else {
            out.push_str(substr);
        }
        last_end = mat.end();
    }
    out.push_str(&text[last_end..]);
    Cow::Owned(out)
}

/// Iterator over `UTF8_DETECTOR_RE` matches in `text` with the `(?<![strict])`
/// lookbehind reconstructed in user code (the `regex` crate doesn't support
/// lookarounds). On rejection the search advances one char past `start` —
/// not to `end` — so that fancy_regex / Python `re`'s failing-lookbehind
/// retry semantics are preserved and an inner valid match nested in a
/// rejected one isn't skipped.
fn accepted_utf8_matches(text: &str) -> impl Iterator<Item = regex::Match<'_>> {
    let mut search_from = 0usize;
    std::iter::from_fn(move || loop {
        let mat = UTF8_DETECTOR_RE.find_at(text, search_from)?;
        let preceding_is_strict = text[..mat.start()]
            .chars()
            .next_back()
            .is_some_and(|c| UTF8_CONTINUATION_STRICT_SET.contains(&c));
        if preceding_is_strict {
            search_from = text.ceil_char_boundary(mat.start() + 1);
            continue;
        }
        search_from = mat.end();
        return Some(mat);
    })
}

fn _c1_fixer(mat: &regex::Captures) -> String {
    let mat = mat.get(0).unwrap().as_str().to_string();

    let encoded = LATIN_1.encode(&mat);

    match encoded {
        Ok(byts) => SLOPPY_WINDOWS_1252.decode(&byts),
        Err(_) => mat,
    }
}

pub fn fix_c1_controls(text: &str) -> Cow<str> {
    /*
    If text still contains C1 control characters, treat them as their
    Windows-1252 equivalents. This matches what Web browsers do.
    */
    C1_CONTROL_RE.replace_all(text, |caps: &regex::Captures| _c1_fixer(caps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_uncode_html_tag() {
        assert_eq!(unescape_html("&lt;tag&gt;"), "<tag>");
    }

    #[test]
    fn test_uncode_html_special_characters() {
        assert_eq!(
            unescape_html("&Jscr;ohn &HilbertSpace;ancock"),
            "𝒥ohn ℋancock"
        );
    }

    #[test]
    fn test_uncode_html_checkmark() {
        assert_eq!(unescape_html("&checkmark;"), "✓");
    }

    #[test]
    fn test_uncode_html_accented_letter_past_tense() {
        assert_eq!(unescape_html("P&eacute;rez"), "Pérez");
    }

    #[test]
    fn test_uncode_html_all_caps() {
        assert_eq!(unescape_html("P&EACUTE;REZ"), "PÉREZ");
    }

    #[test]
    fn test_uncode_html_german_character() {
        assert_eq!(unescape_html("BUNDESSTRA&SZLIG;E"), "BUNDESSTRASSE");
    }

    #[test]
    fn test_uncode_html_variations_of_ntilde() {
        assert_eq!(
            unescape_html("&ntilde; &Ntilde; &NTILDE; &nTILDE;"),
            "ñ Ñ Ñ &nTILDE;"
        );
    }

    #[test]
    fn test_uncode_html_ampersand() {
        assert_eq!(unescape_html("&amp;"), "&");
    }

    #[test]
    fn test_uncode_html_hash() {
        assert_eq!(unescape_html("&#35;"), "#");
    }

    #[test]
    fn test_uncode_html_non_html_string() {
        assert_eq!(unescape_html("non_html_string"), "non_html_string");
    }

    /// Regression: an uppercased entity alias for a lower-case name must not
    /// clobber a real HTML5 entity. e.g.:
    ///     '&dd;' -> 'ⅆ' (U+2146): Double-Struck Italic Small D
    ///     '&DD;' -> 'ⅅ' (U+2145): Double-Struck Italic Capital D
    /// but 'ⅆ'.to_uppercase() != 'ⅅ', so take care to not clobber the real entity
    #[test]
    fn test_html_entity_uppercase_alias_clobber() {
        assert_eq!(unescape_html("x &dd; y"), "x \u{2146} y");
        assert_eq!(unescape_html("x &DD; y"), "x \u{2145} y");
    }

    #[test]
    fn test_remove_escapes_color_text() {
        assert_eq!(
            remove_terminal_escapes("\x1b[36;44mI'm blue da ba dee da ba doo...\x1b[0m"),
            "I'm blue da ba dee da ba doo..."
        );
    }

    #[test]
    fn test_remove_escapes_empty_string() {
        assert_eq!(remove_terminal_escapes(""), "");
    }

    #[test]
    fn test_remove_escapes_no_escapes() {
        assert_eq!(
            remove_terminal_escapes("No escapes here."),
            "No escapes here."
        );
    }

    #[test]
    fn test_remove_escapes_mono_color() {
        assert_eq!(
            remove_terminal_escapes("\x1b[31mRed Text\x1b[0m"),
            "Red Text"
        );
    }

    #[test]
    fn test_remove_escapes_multiple_colors() {
        assert_eq!(
            remove_terminal_escapes("\x1b[31mRed\x1b[32mGreen\x1b[34mBlue\x1b[0m"),
            "RedGreenBlue"
        );
    }

    #[test]
    fn test_remove_escapes_background_color() {
        assert_eq!(
            remove_terminal_escapes("\x1b[41mRed background\x1b[0m"),
            "Red background"
        );
    }

    #[test]
    fn test_remove_escapes_formatting() {
        assert_eq!(
            remove_terminal_escapes("\x1b[1mBold\x1b[22mNormal\x1b[0m"),
            "BoldNormal"
        );
    }

    #[test]
    fn test_remove_escapes_cursor_movement() {
        assert_eq!(
            remove_terminal_escapes("\x1b[5ACursor moved\x1b[0m"),
            "Cursor moved"
        );
    }

    #[test]
    fn test_remove_escapes_multiple_sequences() {
        assert_eq!(
            remove_terminal_escapes("\x1b[31mRed\x1b[0m and \x1b[34mBlue\x1b[0m"),
            "Red and Blue"
        );
    }

    #[test]
    fn test_remove_escapes_multiple_lines() {
        assert_eq!(
            remove_terminal_escapes("\x1b[1mFirst Line\x1b[0m\n\x1b[4mSecond Line\x1b[0m"),
            "First Line\nSecond Line"
        );
    }

    #[test]
    fn test_uncurl_single_character() {
        let input = "‘".to_string();
        assert_eq!(uncurl_quotes(&input), "'");
    }

    #[test]
    fn test_uncurl_multiple_characters() {
        let input = "‘some‘".to_string();
        assert_eq!(uncurl_quotes(&input), "'some'");
    }

    #[test]
    fn test_uncurl_single_quotes() {
        let input = "‘here’s a test’".to_string();
        assert_eq!(uncurl_quotes(&input), "'here's a test'");
    }

    #[test]
    fn test_uncurl_double_quotes() {
        let input = "“here’s a test”".to_string();
        assert_eq!(uncurl_quotes(&input), "\"here's a test\"");
    }

    #[test]
    fn test_uncurl_mixed_quotes() {
        let input = "“here’s a test” and ‘another test’".to_string();
        assert_eq!(
            uncurl_quotes(&input),
            "\"here's a test\" and 'another test'"
        );
    }

    #[test]
    fn test_uncurl_no_quotes() {
        let input = "There are no quotes in this string.".to_string();
        assert_eq!(uncurl_quotes(&input), "There are no quotes in this string.");
    }

    #[test]
    fn test_uncurl_multiple_same_quotes() {
        let input = "‘‘double quotes’’".to_string();
        assert_eq!(uncurl_quotes(&input), "''double quotes''");
    }

    #[test]
    fn test_uncurl_mixed_same_quotes() {
        let input = "“‘mixed quotes’”".to_string();
        assert_eq!(uncurl_quotes(&input), "\"'mixed quotes'\"");
    }

    #[test]
    fn test_uncurl_quotes_at_ends() {
        let input = "‘The quotes are on the ends‘".to_string();
        assert_eq!(uncurl_quotes(&input), "'The quotes are on the ends'");
    }

    #[test]
    fn test_uncurl_empty_string() {
        let input = "".to_string();
        assert_eq!(uncurl_quotes(&input), "");
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(fix_latin_ligatures(""), "");
    }

    #[test]
    fn test_single_ligature_01() {
        assert_eq!(fix_latin_ligatures("ﬁ"), "fi");
    }

    #[test]
    fn test_single_ligature_02() {
        assert_eq!(fix_latin_ligatures("ﬂ"), "fl");
    }

    #[test]
    fn test_multiple_same_ligatures() {
        assert_eq!(fix_latin_ligatures("ﬁﬁﬁ"), "fififi");
    }

    #[test]
    fn test_multiple_different_ligatures() {
        assert_eq!(fix_latin_ligatures("ﬁﬂ"), "fifl");
    }

    #[test]
    fn test_ligature_in_word() {
        assert_eq!(fix_latin_ligatures("afﬁnity"), "affinity");
    }

    #[test]
    fn test_normal_string() {
        assert_eq!(fix_latin_ligatures("normal"), "normal");
    }

    #[test]
    fn test_string_with_ffl() {
        assert_eq!(fix_latin_ligatures("weaponﬄ"), "weaponffl");
    }

    #[test]
    fn test_string_with_non_latin_ligature() {
        // 䜷 is a Chinese ligature
        assert_eq!(fix_latin_ligatures("䜷"), "䜷");
    }

    #[test]
    fn test_string_with_multiple_words() {
        assert_eq!(fix_latin_ligatures("ﬂuﬃeﬆ air"), "fluffiest air");
    }

    #[test]
    fn test_empty_string_character_width() {
        assert_eq!(fix_character_width(""), "");
    }

    #[test]
    fn test_single_fullwidth_char() {
        assert_eq!(fix_character_width("Ａ"), "A");
    }

    #[test]
    fn test_single_halfwidth_katakana() {
        assert_eq!(fix_character_width("ｱ"), "ア");
    }

    #[test]
    fn test_multiple_fullwidth_chars() {
        assert_eq!(fix_character_width("ＬＯＵＤ"), "LOUD");
    }

    #[test]
    fn test_fullwidth_space() {
        assert_eq!(fix_character_width("ＬＯＵＤ　ＮＯＩＳＥＳ"), "LOUD NOISES");
    }

    #[test]
    fn test_combination_half_fullwidth() {
        assert_eq!(fix_character_width("Ｕﾀｰﾝ"), "Uターン");
    }

    #[test]
    fn test_no_change_needed() {
        assert_eq!(fix_character_width("Hello World"), "Hello World");
    }

    #[test]
    fn test_mixed_string() {
        assert_eq!(
            fix_character_width("Ｔhis Ｉs a Mｉｘed Ｗord"),
            "This Is a Mixed Word"
        );
    }

    #[test]
    fn test_halfwidth_hangul() {
        assert_eq!(fix_character_width("ﾖﾝﾍ"), "ヨンヘ");
    }

    #[test]
    fn test_fullwidth_numbers() {
        assert_eq!(fix_character_width("１２３４５"), "12345");
    }

    #[test]
    fn test_empty_string_fix_line_breaks() {
        assert_eq!(fix_line_breaks(""), "");
    }

    #[test]
    fn test_crlf() {
        assert_eq!(fix_line_breaks("Hello\r\nWorld"), "Hello\nWorld");
    }

    #[test]
    fn test_cr() {
        assert_eq!(fix_line_breaks("Hello\rWorld"), "Hello\nWorld");
    }

    #[test]
    fn test_line_separator() {
        assert_eq!(fix_line_breaks("Hello\u{2028}World"), "Hello\nWorld");
    }

    #[test]
    fn test_paragraph_separator() {
        assert_eq!(fix_line_breaks("Hello\u{2029}World"), "Hello\nWorld");
    }

    #[test]
    fn test_next_line() {
        assert_eq!(fix_line_breaks("Hello\u{0085}World"), "Hello\nWorld");
    }

    #[test]
    fn test_mix_breaks() {
        assert_eq!(
            fix_line_breaks("Hello\r\nWorld\u{2028}This\u{2029}Is\u{0085}Line\rBreak"),
            "Hello\nWorld\nThis\nIs\nLine\nBreak"
        );
    }

    #[test]
    fn test_no_change_needed_fix_line_breaks() {
        assert_eq!(fix_line_breaks("Hello\nWorld"), "Hello\nWorld");
    }

    #[test]
    fn test_multiple_same_breaks() {
        assert_eq!(fix_line_breaks("Hello\r\rWorld"), "Hello\n\nWorld");
    }

    #[test]
    fn test_string_with_no_breaks() {
        assert_eq!(fix_line_breaks("HelloWorld"), "HelloWorld");
    }

    #[test]
    fn test_all_valid_chars() {
        let input = "Hello, World!";
        let output = remove_control_chars(input);
        assert_eq!(output, "Hello, World!");
    }

    #[test]
    fn test_mix_valid_invalid_chars() {
        let input = "Hello,\r\n World!";
        let output = remove_control_chars(input);
        assert_eq!(output, "Hello,\r\n World!");
    }

    #[test]
    fn test_all_invalid_chars() {
        let input = "\x07\x08\x0B\x0E\x0F";
        let output = remove_control_chars(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_deprecated_arabic_chars() {
        let input = "\u{206A}\u{206B}";
        let output = remove_control_chars(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_both_whitespace_non_whitespace_control_chars() {
        let input = "Hello\x0A\x08World";
        let output = remove_control_chars(input);
        assert_eq!(output, "Hello\nWorld");
    }

    #[test]
    fn test_whitespaces_control_chars() {
        let input = "\t\n\r";
        let output = remove_control_chars(input);
        assert_eq!(output, "\t\n\r");
    }

    #[test]
    fn test_arabic_control_chars() {
        let input = "Hello\u{206A}World";
        let output = remove_control_chars(input);
        assert_eq!(output, "HelloWorld");
    }

    #[test]
    fn test_interlinear_annotation_chars() {
        let input = "Hello\u{FFF9}World";
        let output = remove_control_chars(input);
        assert_eq!(output, "HelloWorld");
    }

    #[test]
    fn test_with_byte_order_mark() {
        let input = "\u{FEFF}Hello World";
        let output = remove_control_chars(input);
        assert_eq!(output, "Hello World");
    }

    #[test]
    fn test_empty_string_remove_control_chars() {
        let input = "";
        let output = remove_control_chars(input);
        assert_eq!(output, "");
    }

    #[test]
    fn it_can_handle_empty_strings() {
        assert_eq!(restore_byte_a0("".as_bytes()), "".as_bytes());
    }

    #[test]
    fn it_does_not_restore_when_bytes_already_in_utf8() {
        assert_eq!(
            restore_byte_a0("normal text".as_bytes()),
            "normal text".as_bytes()
        );
    }

    #[test]
    fn it_handles_escapes_correctly() {
        assert_eq!(
            restore_byte_a0("\\\\U12345678".as_bytes()),
            "\\\\U12345678".as_bytes()
        );
    }
}

/// Ported from ftfy's `tests/test_characters.py`
#[cfg(test)]
mod ftfy_test_characters {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_control_chars() {
        let text = "\u{feff}Sometimes, \u{fffc}bad ideas \u{7f}\u{fffa}like these characters\u{fffb} \u{206a}get standardized.\r\n";
        let fixed = "Sometimes, bad ideas like these characters get standardized.\r\n";
        assert_eq!(remove_control_chars(text), fixed);
    }

    // ftfy used to remove "tag characters", but they have been repurposed in the
    // "Flag of England", "Flag of Scotland", and "Flag of Wales" emoji sequences.
    #[test]
    fn test_welsh_flag() {
        let codepoints = "\u{1f3f4}\u{e0067}\u{e0062}\u{e0077}\u{e006c}\u{e0073}\u{e007f}";
        let text = "This flag has a dragon on it 🏴󠁧󠁢󠁷󠁬󠁳󠁿";
        assert_eq!(text, format!("This flag has a dragon on it {codepoints}"));
        assert_eq!(remove_control_chars(text), text);
    }
}

/// Ported from ftfy's `tests/test_entities.py`
#[cfg(test)]
mod ftfy_test_entities {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_unescape_html_euro_numeric() {
        assert_eq!(unescape_html("euro &#x80;"), "euro €");
    }

    #[test]
    fn test_unescape_html_euro_named_all_caps() {
        assert_eq!(unescape_html("EURO &EURO;"), "EURO €");
    }

    #[test]
    fn test_unescape_html_not_an_entity() {
        assert_eq!(
            unescape_html("not an entity &#20x6;"),
            "not an entity &#20x6;"
        );
    }

    #[test]
    fn test_unescape_html_sacute_all_caps() {
        assert_eq!(unescape_html("JEDNOCZE&SACUTE;NIE"), "JEDNOCZEŚNIE");
    }

    #[test]
    fn test_unescape_html_scaron_all_caps() {
        assert_eq!(unescape_html("V&SCARON;ICHNI"), "VŠICHNI");
    }

    // WHATWG § 13.2.5.80: noncharacters are emitted as their codepoint
    // ("parse error, but the character is emitted"). Python's
    // `html.unescape` deletes them; plsfix follows WHATWG.
    #[test]
    fn test_unescape_html_noncharacter() {
        assert_eq!(unescape_html("&#xffff;"), "\u{ffff}");
    }

    // > 0x10FFFF → U+FFFD per WHATWG § 13.2.5.80.
    #[test]
    fn test_unescape_html_out_of_range() {
        assert_eq!(unescape_html("&#xffffffff;"), "\u{fffd}");
    }

    // other html tests

    #[test]
    fn test_unescape_html_above_unicode_max() {
        assert_eq!(unescape_html("&#x110000;"), "\u{fffd}");
    }

    // NUL → U+FFFD per WHATWG § 13.2.5.80.
    #[test]
    fn test_unescape_html_nul() {
        assert_eq!(unescape_html("&#0;"), "\u{fffd}");
    }

    // 0x80..=0x9F C1 remap per WHATWG § 13.2.5.80.
    #[test]
    fn test_unescape_html_c1_range_decimal() {
        assert_eq!(unescape_html("&#128;"), "€");
    }

    #[test]
    fn test_unescape_html_c1_range_dash() {
        assert_eq!(unescape_html("en &#x96; dash"), "en \u{2013} dash");
    }

    #[test]
    fn test_unescape_html_c1_self_mapped() {
        // 0x9D has no HTML5 replacement → U+009D.
        assert_eq!(unescape_html("&#x9d;"), "\u{9d}");
    }

    #[test]
    fn test_strict_continuation_set_includes_byte_range() {
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'\u{80}'));
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'\u{a0}'));
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'\u{b7}'));
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'\u{bf}'));
    }

    #[test]
    fn test_strict_continuation_set_includes_explicit_chars() {
        // Sentinels from each region of the clue's explicit-char tail; if the
        // skip-the-`\x80-\xbf`-literal arithmetic miscounts, one flips.
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'Ą'));
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'Œ'));
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'Ώ'));
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'Ґ'));
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'€'));
        assert!(UTF8_CONTINUATION_STRICT_SET.contains(&'™'));
    }

    #[test]
    fn test_strict_continuation_set_excludes_nonstrict_continuation_chars() {
        // In `utf8_continuation` but not `_strict` — must stay out so they
        // don't block mojibake matches.
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&' '));
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'—'));
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'‘'));
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'”'));
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'•'));
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'…'));
    }

    #[test]
    fn test_strict_continuation_set_excludes_ascii_and_unrelated_chars() {
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'a'));
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'0'));
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'\u{7f}'));
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'\u{c0}'));
        // first_of_2 chars must never be strict, else two valid groups self-block.
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'Ã'));
        assert!(!UTF8_CONTINUATION_STRICT_SET.contains(&'Â'));
    }

    #[test]
    fn test_utf8_detector_re_finds_basic_mojibake() {
        let m = UTF8_DETECTOR_RE.find("Ã©").expect("should match");
        assert_eq!(m.as_str(), "Ã©");
        assert_eq!(m.start(), 0);
    }

    #[test]
    fn test_utf8_detector_re_greedy_plus_coalesces_adjacent_groups() {
        let m = UTF8_DETECTOR_RE.find("Ã©Ã¨").expect("should match");
        assert_eq!(m.as_str(), "Ã©Ã¨");
    }

    #[test]
    fn test_utf8_detector_re_ignores_plain_ascii() {
        assert!(UTF8_DETECTOR_RE.find("plain ascii text").is_none());
    }

    #[test]
    fn test_decode_inconsistent_utf8_passthrough_empty() {
        assert_eq!(decode_inconsistent_utf8(""), "");
    }

    #[test]
    fn test_decode_inconsistent_utf8_passthrough_ascii() {
        let s = "no mojibake here";
        let out = decode_inconsistent_utf8(s);
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, s);
    }

    #[test]
    fn test_decode_inconsistent_utf8_match_at_start_of_string() {
        assert_eq!(decode_inconsistent_utf8("Ã©foo"), "éfoo");
    }

    #[test]
    fn test_decode_inconsistent_utf8_preceded_by_ascii_accepts() {
        assert_eq!(decode_inconsistent_utf8("foo Ã©"), "foo é");
    }

    #[test]
    fn test_decode_inconsistent_utf8_preceded_by_strict_byte_range_rejects() {
        let input = "\u{a0}Ã©";
        assert_eq!(decode_inconsistent_utf8(input), input);
    }

    #[test]
    fn test_decode_inconsistent_utf8_preceded_by_strict_multibyte_rejects() {
        // Ą is 2 bytes — exercises chars().next_back() with non-ASCII.
        let input = "ĄÃ©";
        assert_eq!(decode_inconsistent_utf8(input), input);
    }

    #[test]
    fn test_decode_inconsistent_utf8_preceded_by_nonstrict_continuation_accepts() {
        assert_eq!(decode_inconsistent_utf8("—Ã©"), "—é");
        assert_eq!(decode_inconsistent_utf8("\u{2018}Ã©"), "\u{2018}é");
    }

    #[test]
    fn test_decode_inconsistent_utf8_retry_finds_inner_match_after_rejection() {
        // Outer greedy match "Â—Ã©" is rejected (preceded by ·). A naive
        // `search_from = end` would miss the inner "Ã©"; the one-char retry
        // finds it, preceded by — (continuation but not strict).
        assert_eq!(decode_inconsistent_utf8("·Â—Ã©"), "·Â—é");
    }

    #[test]
    fn test_decode_inconsistent_utf8_multiple_spans_mixed_acceptance() {
        let input = "\u{a0}Ã© foo Ã©bar";
        assert_eq!(decode_inconsistent_utf8(input), "\u{a0}Ã© foo ébar");
    }

    #[test]
    fn test_decode_inconsistent_utf8_match_at_end_of_string() {
        assert_eq!(decode_inconsistent_utf8("foo Ã©"), "foo é");
    }

    #[test]
    fn test_decode_inconsistent_utf8_rejection_then_no_more_matches() {
        let input = "\u{a0}Ã©";
        let out = decode_inconsistent_utf8(input);
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, input);
    }

    #[test]
    fn test_fix_c1_controls_remaps_euro() {
        assert_eq!(fix_c1_controls("price: \u{80}9.99"), "price: €9.99");
    }

    #[test]
    fn test_fix_c1_controls_remaps_curly_quotes() {
        assert_eq!(
            fix_c1_controls("\u{93}hello\u{94}"),
            "\u{201c}hello\u{201d}"
        );
    }

    #[test]
    fn test_fix_c1_controls_passes_through_non_c1() {
        // \u{a0} is just above the C1 range.
        let s = "ascii plus \u{a0} and \u{ff}";
        assert_eq!(fix_c1_controls(s), s);
    }

    #[test]
    fn test_fix_c1_controls_passes_through_pure_ascii() {
        let s = "nothing to fix here";
        let out = fix_c1_controls(s);
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, s);
    }
}
