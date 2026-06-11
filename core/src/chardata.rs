use rustc_hash::FxHashMap;
use unicode_normalization::UnicodeNormalization;

use regex::Regex;

pub use crate::utf8::UTF8_CONTINUATION_STRICT_SET;

use crate::codecs::sloppy::{
    Codec, CodecType, CP437, ISO_8859_2, LATIN_1, MACROMAN, SLOPPY_WINDOWS_1250,
    SLOPPY_WINDOWS_1251, SLOPPY_WINDOWS_1252, SLOPPY_WINDOWS_1253, SLOPPY_WINDOWS_1254,
    SLOPPY_WINDOWS_1257,
};

pub fn possible_encoding(text: &str, encoding: CodecType) -> bool {
    /*
    Given text and a single-byte encoding, check whether that text could have
    been decoded from that single-byte encoding.

    In other words, check whether it can be encoded in that encoding, possibly
    sloppily.
    */
    ENCODING_REGEXES[&encoding].is_match(text.as_bytes())
}

lazy_static! {
    pub static ref CHARMAP_ENCODINGS: Vec<(CodecType, &'static dyn Codec)> = {
        let mut codecs: Vec<(CodecType, &dyn Codec)> = Vec::new();

        codecs.push((CodecType::Latin1, &*LATIN_1));
        codecs.push((CodecType::SloppyWindows1252, &*SLOPPY_WINDOWS_1252));
        codecs.push((CodecType::SloppyWindows1251, &*SLOPPY_WINDOWS_1251));
        codecs.push((CodecType::SloppyWindows1250, &*SLOPPY_WINDOWS_1250));
        codecs.push((CodecType::SloppyWindows1253, &*SLOPPY_WINDOWS_1253));
        codecs.push((CodecType::SloppyWindows1254, &*SLOPPY_WINDOWS_1254));
        codecs.push((CodecType::SloppyWindows1257, &*SLOPPY_WINDOWS_1257));
        codecs.push((CodecType::Iso88592, &*ISO_8859_2));
        codecs.push((CodecType::MacRoman, &*MACROMAN));
        codecs.push((CodecType::Cp437, &*CP437));

        codecs
    };

    pub static ref SINGLE_QUOTE_RE: Regex =
        Regex::new("\u{02bc}|\u{2018}|\u{2019}|\u{201a}|\u{201b}").unwrap();
    pub static ref DOUBLE_QUOTE_RE: Regex =
        Regex::new("\u{201c}|\u{201d}|\u{201e}|\u{201f}").unwrap();

    /*
    ENCODING_REGEXES contain reasonably fast ways to detect if we
    could represent a given string in a given encoding. The simplest one is
    the 'ascii' detector, which of course just determines if all characters
    are between U+0000 and U+007F.
    */
    pub static ref ENCODING_REGEXES: FxHashMap<CodecType, regex::bytes::Regex> = {
        let mut encoding_regexes: FxHashMap<CodecType, regex::bytes::Regex> = FxHashMap::default();

        encoding_regexes.insert(
            CodecType::Ascii,
            regex::bytes::Regex::new(r"(?-u:^[\x00-\x7f]*$)").unwrap(),
        );

        /*
        Make a sequence of characters that bytes \x80 to \xFF decode to
        in each encoding, as well as byte \x1A, which is used to represent
        the replacement character � in the sloppy-* encodings.
        */
        let byte_range: Vec<u8> = (0x80..=0xFF).chain(std::iter::once(0x1A)).collect();

        for (codec_type, codec) in CHARMAP_ENCODINGS.iter() {
            let charlist = codec.decode(&byte_range);
            // for each character, encode back to utf-8
            let bytes = charlist
                .chars()
                .map(|c| c.encode_utf8(&mut [0; 4]).as_bytes().to_vec());

            // convert bytes to a regex like \x80\x81\x82\x83|\x84\x85\x86\x87|...
            let charlist = bytes
                .map(|b| {
                    b.iter()
                        .map(|b| format!(r"\x{:02x}", b))
                        .collect::<Vec<String>>()
                        .join("")
                })
                .collect::<Vec<String>>()
                .join("|");

            /*
            The rest of the ASCII bytes -- bytes \x00 to \x19 and \x1B
            to \x7F -- will decode as those ASCII characters in any encoding we
            support, so we can just include them as ranges. This also lets us
            not worry about escaping regex special characters, because all of
            them are in the \x1B to \x7F range.
            */
            let regex = format!(
                r"^(?-u:[\x00-\x19\x1b-\x7f]|{charlist})*$",
                charlist = charlist
            );

            encoding_regexes.insert(
                codec_type.clone(),
                regex::bytes::Regex::new(&regex).unwrap(),
            );
        }

        return encoding_regexes;
    };

    pub static ref HTML_ENTITY_RE: Regex = Regex::new(r"&#?[0-9A-Za-z]{1,24};").unwrap();

    /*
    Recognize UTF-8 sequences that would be valid if it weren't for a b'\xa0'
    that some Windows-1252 program converted to a plain space.

    The smaller values are included on a case-by-case basis, because we don't want
    to decode likely input sequences to unlikely characters. These are the ones
    that *do* form likely characters before 0xa0:

    0xc2 -> U+A0 NO-BREAK SPACE
    0xc3 -> U+E0 LATIN SMALL LETTER A WITH GRAVE
    0xc5 -> U+160 LATIN CAPITAL LETTER S WITH CARON
    0xce -> U+3A0 GREEK CAPITAL LETTER PI
    0xd0 -> U+420 CYRILLIC CAPITAL LETTER ER
    0xd9 -> U+660 ARABIC-INDIC DIGIT ZERO

    In three-character sequences, we exclude some lead bytes in some cases.

    When the lead byte is immediately followed by 0xA0, we shouldn't accept
    a space there, because it leads to some less-likely character ranges:

    0xe0 -> Samaritan script
    0xe1 -> Mongolian script (corresponds to Latin-1 'á' which is too common)

    We accept 0xe2 and 0xe3, which cover many scripts. Bytes 0xe4 and
    higher point mostly to CJK characters, which we generally don't want to
    decode near Latin lowercase letters.

    In four-character sequences, the lead byte must be F0, because that accounts
    for almost all of the usage of high-numbered codepoints (tag characters whose
    UTF-8 starts with the byte F3 are only used in some rare new emoji sequences).

    This is meant to be applied to encodings of text that tests true for `is_bad`.
    Any of these could represent characters that legitimately appear surrounded by
    spaces, particularly U+C5 (Å), which is a word in multiple languages!

    We should consider checking for b'\x85' being converted to ... in the future.
    I've seen it once, but the text still wasn't recoverable.
    */
    pub static ref ALTERED_UTF8_RE: regex::bytes::Regex = regex::bytes::Regex::new(
        &(r"(?-u:".to_string() + r"[\xc2\xc3\xc5\xce\xd0\xd9][ ]"
            + r"|[\xe2\xe3][ ][\x80-\x84\x86-\x9f\xa1-\xbf]"
            + r"|[\xe0-\xe3][\x80-\x84\x86-\x9f\xa1-\xbf][ ]"
            + r"|[\xf0][ ][\x80-\xbf][\x80-\xbf]"
            + r"|[\xf0][\x80-\xbf][ ][\x80-\xbf]"
            + r"|[\xf0][\x80-\xbf][\x80-\xbf][ ]"
            + r")"
        )
    )
    .unwrap();

    /*
    This expression matches UTF-8 and CESU-8 sequences where some of the
    continuation bytes have been lost. The byte 0x1a (sometimes written as ^Z) is
    used within plsfix to represent a byte that produced the replacement character
    \ufffd. We don't know which byte it was, but we can at least decode the UTF-8
    sequence as \ufffd instead of failing to re-decode it at all.

    In some cases, we allow the ASCII '?' in place of \ufffd, but at most once per
    sequence.
    */
    pub static ref LOSSY_UTF8_RE: regex::bytes::Regex = regex::bytes::Regex::new(

        &(r"(?-u:".to_string()
            + r"[\xc2-\xdf][\x1a]"
            + r"|[\xc2-\xc3][?]"
            + r"|\xed[\xa0-\xaf][\x1a?]\xed[\xb0-\xbf][\x1a?\x80-\xbf]"
            + r"|\xed[\xa0-\xaf][\x1a?\x80-\xbf]\xed[\xb0-\xbf][\x1a?]"
            + r"|[\xe0-\xef][\x1a?][\x1a\x80-\xbf]"
            + r"|[\xe0-\xef][\x1a\x80-\xbf][\x1a?]"
            + r"|[\xf0-\xf4][\x1a?][\x1a\x80-\xbf][\x1a\x80-\xbf]"
            + r"|[\xf0-\xf4][\x1a\x80-\xbf][\x1a?][\x1a\x80-\xbf]"
            + r"|[\xf0-\xf4][\x1a\x80-\xbf][\x1a\x80-\xbf][\x1a?]"
            + r"|\x1a"
            + r")"
        )
    )
    .unwrap();

    /*
    This regex matches C1 control characters, which occupy some of the positions
    in the Latin-1 character map that Windows assigns to other characters instead.
    */
    pub static ref C1_CONTROL_RE: regex::Regex =
        regex::Regex::new(r"[\x80-\x9f]").unwrap();

    /*
    A translate mapping that breaks ligatures made of Latin letters. While
    ligatures may be important to the representation of other languages, in Latin
    letters they tend to represent a copy/paste error. It omits ligatures such
    as æ that are frequently used intentionally.

    This list additionally includes some Latin digraphs that represent two
    characters for legacy encoding reasons, not for typographical reasons.

    Ligatures and digraphs may also be separated by NFKC normalization, but that
    is sometimes more normalization than you want.
    */
    pub static ref LIGATURES: FxHashMap<u32, &'static str> = {
        let mut ligatures: FxHashMap<u32, &str> = FxHashMap::default();

        ligatures.insert('Ĳ' as u32, "IJ"); // Dutch ligatures
        ligatures.insert('ĳ' as u32, "ij");
        ligatures.insert('ŉ' as u32, "ʼn"); // Afrikaans digraph meant to avoid auto-curled quote
        ligatures.insert('Ǳ' as u32, "DZ"); // Serbian/Croatian digraphs for Cyrillic conversion
        ligatures.insert('ǲ' as u32, "Dz");
        ligatures.insert('ǳ' as u32, "dz");
        ligatures.insert('Ǆ' as u32, "DŽ");
        ligatures.insert('ǅ' as u32, "Dž");
        ligatures.insert('ǆ' as u32, "dž");
        ligatures.insert('Ǉ' as u32, "LJ");
        ligatures.insert('ǈ' as u32, "Lj");
        ligatures.insert('ǉ' as u32, "lj");
        ligatures.insert('Ǌ' as u32, "NJ");
        ligatures.insert('ǋ' as u32, "Nj");
        ligatures.insert('ǌ' as u32, "nj");
        ligatures.insert('ﬀ' as u32, "ff"); // Latin typographical ligatures
        ligatures.insert('ﬁ' as u32, "fi");
        ligatures.insert('ﬂ' as u32, "fl");
        ligatures.insert('ﬃ' as u32, "ffi");
        ligatures.insert('ﬄ' as u32, "ffl");
        ligatures.insert('ﬅ' as u32, "ſt");
        ligatures.insert('ﬆ' as u32, "st");

        ligatures
    };

    pub static ref WIDTH_MAP: FxHashMap<u32, char> = {
        let mut width_map: FxHashMap<u32, char> = FxHashMap::default();
        // Though it's not listed as a fullwidth character, we'll want to convert
        // U+3000 IDEOGRAPHIC SPACE to U+20 SPACE on the same principle, so start
        // with that in the dictionary.
        width_map.insert(0x3000, ' ');

        for i in 0xFF01..0xFFF0 {
            if let Some(ci) = std::char::from_u32(i) {
                let alternate = ci.nfkc().next();
                if let Some(c) = alternate {
                    if c != ci {
                        width_map.insert(i, c);
                    }
                }
            }
        }
        width_map
    };


    /*
    The character classes that UTF8_DETECTOR_RE is built from, keyed the same way
    as ftfy's UTF8_CLUES dict. The per-character `encoding:byte` annotations that
    document where each character comes from live in the test that pins this map to
    ftfy (see test_utf8_clues_match_ftfy).
    */
    static ref UTF8_CLUES: FxHashMap<&'static str, &'static str> = {
        let mut m = FxHashMap::default();
        // Letters that decode to 0xC2 - 0xDF in a Latin-1-like encoding
        m.insert("utf8_first_of_2", "ĂÂÄĀÅÃÆĆČÇĎĐÉĚÊËĖÈĒĘÐĞĢÍÎÏİÌĪĶĹĻŁŃŇŅÑÓÔÖŐÒŌØÕŘŚŠŞŢÞÚÛÜŰÙŪŲŮÝŹŽŻß×ΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩΪΫάέήίВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ");
        // Letters that decode to 0xE0 - 0xEF in a Latin-1-like encoding
        m.insert("utf8_first_of_3", "áăâäàāąåãæćčçďéěêëėèēęęģíîïìīįķĺļŕźΰαβγδεζηθικλμνξοабвгдежзийклмноп");
        // Letters that decode to 0xF0 or 0xF3 in a Latin-1-like encoding.
        // (Other leading bytes correspond only to unassigned codepoints)
        m.insert("utf8_first_of_4", "đðğóšπσру");
        // Letters that decode to 0x80 - 0xBF in a Latin-1-like encoding,
        // including a space (`\u{20}`) standing in for 0xA0
        m.insert("utf8_continuation", r"\x80-\xbf\u{20}ĄÆĽŁØŖŚŠŞŤŸŹŽŻŒąæƒľłøŗśšşťźžżœˆˇ˘˛˜˝΄΅ΆΈΉΊΌΎΏЁЂЃЄЅІЇЈЉЊЋЌЎЏёђѓєѕіїјљњћќўџҐґ–—―‘’‚“”„†‡•…‰‹›€№™");
        // Letters that decode to 0x80 - 0xBF in a Latin-1-like encoding,
        // and don't usually stand for themselves when adjacent to mojibake.
        // This excludes spaces, dashes, 'bullet', quotation marks, and ellipses.
        m.insert("utf8_continuation_strict", r"\x80-\xbfĄÆĽŁØŖŚŠŞŤŸŹŽŻŒąæƒľłøŗśšşťźžżœˆˇ˘˛˜˝΄΅ΆΈΉΊΌΎΏЁЂЃЄЅІЇЈЉЊЋЌЎЏёђѓєѕіїјљњћќўџҐґ†‡‰‹›€№™");
        m
    };

    /*
    This regex uses UTF8_CLUES to find sequences of likely mojibake.
    It matches them with + so that several adjacent UTF-8-looking sequences
    get coalesced into one, allowing them to be fixed more efficiently
    and not requiring every individual subsequence to be detected as 'badness'.

    We accept spaces in place of "utf8_continuation", because spaces might have
    been intended to be U+A0 NO-BREAK SPACE.

    We do a lookbehind to make sure the previous character isn't a
    "utf8_continuation_strict" character, so that we don't fix just a few
    characters in a huge garble and make the situation worse.

    Unfortunately, the matches to this regular expression won't show their
    surrounding context, and including context would make the expression much
    less efficient. The 'badness' rules that require context, such as a preceding
    lowercase letter, will prevent some cases of inconsistent UTF-8 from being
    fixed when they don't see it.
    */
    // Lookbehind `(?<![strict])` lives in `decode_inconsistent_utf8` instead —
    // the `regex` crate doesn't support lookarounds.
    pub static ref UTF8_DETECTOR_RE: regex::Regex = {
        regex::Regex::new(
        &format!(
            r"(?x)
            (
                [{utf8_first_of_2}] [{utf8_continuation}]
                |
                [{utf8_first_of_3}] [{utf8_continuation}]{{2}}
                |
                [{utf8_first_of_4}] [{utf8_continuation}]{{3}}
            )+",
            utf8_first_of_2 = UTF8_CLUES["utf8_first_of_2"],
            utf8_first_of_3 = UTF8_CLUES["utf8_first_of_3"],
            utf8_first_of_4 = UTF8_CLUES["utf8_first_of_4"],
            utf8_continuation = UTF8_CLUES["utf8_continuation"],
        ),
    )
    .expect("Failed to compile the regex")
    };
}

/// Likely-unintended control characters to be stripped.
///
/// See [`crate::fixes::remove_control_chars`] for a description of these codepoint ranges and why
/// they should be removed.
#[inline]
pub const fn is_control_char(c: char) -> bool {
    matches!(
        c,
        '\u{00}'..='\u{08}'
            | '\u{0B}'
            | '\u{0E}'..='\u{1F}'
            | '\u{7F}'
            | '\u{206A}'..='\u{206F}'
            | '\u{FFF9}'..='\u{FFFC}'
            | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::UTF8_CLUES;

    /// Look up a character by its Unicode name (how ftfy spells its clues),
    /// mirroring python's `\N{...}` escape.
    fn get_char(name: &str) -> char {
        unicode_names2::character(name).unwrap_or_else(|| panic!("unknown Unicode name {name:?}"))
    }
    fn c(name: &str) -> String {
        get_char(name).into()
    }
    fn s(name: &str) -> String {
        let c = get_char(name);
        assert!(c.is_whitespace());
        c.escape_unicode().collect()
    }

    /// Verify our UTF8_CLUES matches ftfy's exactly, character for character.
    /// Mirrors test_mojibake_categories_match_ftfy in badness.rs; the per-character
    /// `encoding:byte` comments are copied verbatim from ftfy's chardata.py.
    #[test]
    fn test_utf8_clues_match_ftfy() {
        let expected: std::collections::BTreeMap<&str, String> = [
            // Letters that decode to 0xC2 - 0xDF in a Latin-1-like encoding
            (
                "utf8_first_of_2",
                vec![
                    c("LATIN CAPITAL LETTER A WITH BREVE"),      // windows-1250:C3
                    c("LATIN CAPITAL LETTER A WITH CIRCUMFLEX"), // latin-1:C2
                    c("LATIN CAPITAL LETTER A WITH DIAERESIS"),  // latin-1:C4
                    c("LATIN CAPITAL LETTER A WITH MACRON"),     // windows-1257:C2
                    c("LATIN CAPITAL LETTER A WITH RING ABOVE"), // latin-1:C5
                    c("LATIN CAPITAL LETTER A WITH TILDE"),      // latin-1:C3
                    c("LATIN CAPITAL LETTER AE"),                // latin-1:C6
                    c("LATIN CAPITAL LETTER C WITH ACUTE"),      // windows-1250:C6
                    c("LATIN CAPITAL LETTER C WITH CARON"),      // windows-1250:C8
                    c("LATIN CAPITAL LETTER C WITH CEDILLA"),    // latin-1:C7
                    c("LATIN CAPITAL LETTER D WITH CARON"),      // windows-1250:CF
                    c("LATIN CAPITAL LETTER D WITH STROKE"),     // windows-1250:D0
                    c("LATIN CAPITAL LETTER E WITH ACUTE"),      // latin-1:C9
                    c("LATIN CAPITAL LETTER E WITH CARON"),      // windows-1250:CC
                    c("LATIN CAPITAL LETTER E WITH CIRCUMFLEX"), // latin-1:CA
                    c("LATIN CAPITAL LETTER E WITH DIAERESIS"),  // latin-1:CB
                    c("LATIN CAPITAL LETTER E WITH DOT ABOVE"),  // windows-1257:CB
                    c("LATIN CAPITAL LETTER E WITH GRAVE"),      // latin-1:C8
                    c("LATIN CAPITAL LETTER E WITH MACRON"),     // windows-1257:C7
                    c("LATIN CAPITAL LETTER E WITH OGONEK"),     // windows-1250:CA
                    c("LATIN CAPITAL LETTER ETH"),               // latin-1:D0
                    c("LATIN CAPITAL LETTER G WITH BREVE"),      // windows-1254:D0
                    c("LATIN CAPITAL LETTER G WITH CEDILLA"),    // windows-1257:CC
                    c("LATIN CAPITAL LETTER I WITH ACUTE"),      // latin-1:CD
                    c("LATIN CAPITAL LETTER I WITH CIRCUMFLEX"), // latin-1:CE
                    c("LATIN CAPITAL LETTER I WITH DIAERESIS"),  // latin-1:CF
                    c("LATIN CAPITAL LETTER I WITH DOT ABOVE"),  // windows-1254:DD
                    c("LATIN CAPITAL LETTER I WITH GRAVE"),      // latin-1:CC
                    c("LATIN CAPITAL LETTER I WITH MACRON"),     // windows-1257:CE
                    c("LATIN CAPITAL LETTER K WITH CEDILLA"),    // windows-1257:CD
                    c("LATIN CAPITAL LETTER L WITH ACUTE"),      // windows-1250:C5
                    c("LATIN CAPITAL LETTER L WITH CEDILLA"),    // windows-1257:CF
                    c("LATIN CAPITAL LETTER L WITH STROKE"),     // windows-1257:D9
                    c("LATIN CAPITAL LETTER N WITH ACUTE"),      // windows-1250:D1
                    c("LATIN CAPITAL LETTER N WITH CARON"),      // windows-1250:D2
                    c("LATIN CAPITAL LETTER N WITH CEDILLA"),    // windows-1257:D2
                    c("LATIN CAPITAL LETTER N WITH TILDE"),      // latin-1:D1
                    c("LATIN CAPITAL LETTER O WITH ACUTE"),      // latin-1:D3
                    c("LATIN CAPITAL LETTER O WITH CIRCUMFLEX"), // latin-1:D4
                    c("LATIN CAPITAL LETTER O WITH DIAERESIS"),  // latin-1:D6
                    c("LATIN CAPITAL LETTER O WITH DOUBLE ACUTE"), // windows-1250:D5
                    c("LATIN CAPITAL LETTER O WITH GRAVE"),      // latin-1:D2
                    c("LATIN CAPITAL LETTER O WITH MACRON"),     // windows-1257:D4
                    c("LATIN CAPITAL LETTER O WITH STROKE"),     // latin-1:D8
                    c("LATIN CAPITAL LETTER O WITH TILDE"),      // latin-1:D5
                    c("LATIN CAPITAL LETTER R WITH CARON"),      // windows-1250:D8
                    c("LATIN CAPITAL LETTER S WITH ACUTE"),      // windows-1257:DA
                    c("LATIN CAPITAL LETTER S WITH CARON"),      // windows-1257:D0
                    c("LATIN CAPITAL LETTER S WITH CEDILLA"),    // windows-1254:DE
                    c("LATIN CAPITAL LETTER T WITH CEDILLA"),    // windows-1250:DE
                    c("LATIN CAPITAL LETTER THORN"),             // latin-1:DE
                    c("LATIN CAPITAL LETTER U WITH ACUTE"),      // latin-1:DA
                    c("LATIN CAPITAL LETTER U WITH CIRCUMFLEX"), // latin-1:DB
                    c("LATIN CAPITAL LETTER U WITH DIAERESIS"),  // latin-1:DC
                    c("LATIN CAPITAL LETTER U WITH DOUBLE ACUTE"), // windows-1250:DB
                    c("LATIN CAPITAL LETTER U WITH GRAVE"),      // latin-1:D9
                    c("LATIN CAPITAL LETTER U WITH MACRON"),     // windows-1257:DB
                    c("LATIN CAPITAL LETTER U WITH OGONEK"),     // windows-1257:D8
                    c("LATIN CAPITAL LETTER U WITH RING ABOVE"), // windows-1250:D9
                    c("LATIN CAPITAL LETTER Y WITH ACUTE"),      // latin-1:DD
                    c("LATIN CAPITAL LETTER Z WITH ACUTE"),      // windows-1257:CA
                    c("LATIN CAPITAL LETTER Z WITH CARON"),      // windows-1257:DE
                    c("LATIN CAPITAL LETTER Z WITH DOT ABOVE"),  // windows-1257:DD
                    c("LATIN SMALL LETTER SHARP S"),             // latin-1:DF
                    c("MULTIPLICATION SIGN"),                    // latin-1:D7
                    c("GREEK CAPITAL LETTER BETA"),              // windows-1253:C2
                    c("GREEK CAPITAL LETTER GAMMA"),             // windows-1253:C3
                    c("GREEK CAPITAL LETTER DELTA"),             // windows-1253:C4
                    c("GREEK CAPITAL LETTER EPSILON"),           // windows-1253:C5
                    c("GREEK CAPITAL LETTER ZETA"),              // windows-1253:C6
                    c("GREEK CAPITAL LETTER ETA"),               // windows-1253:C7
                    c("GREEK CAPITAL LETTER THETA"),             // windows-1253:C8
                    c("GREEK CAPITAL LETTER IOTA"),              // windows-1253:C9
                    c("GREEK CAPITAL LETTER KAPPA"),             // windows-1253:CA
                    c("GREEK CAPITAL LETTER LAMDA"),             // windows-1253:CB
                    c("GREEK CAPITAL LETTER MU"),                // windows-1253:CC
                    c("GREEK CAPITAL LETTER NU"),                // windows-1253:CD
                    c("GREEK CAPITAL LETTER XI"),                // windows-1253:CE
                    c("GREEK CAPITAL LETTER OMICRON"),           // windows-1253:CF
                    c("GREEK CAPITAL LETTER PI"),                // windows-1253:D0
                    c("GREEK CAPITAL LETTER RHO"),               // windows-1253:D1
                    c("GREEK CAPITAL LETTER SIGMA"),             // windows-1253:D3
                    c("GREEK CAPITAL LETTER TAU"),               // windows-1253:D4
                    c("GREEK CAPITAL LETTER UPSILON"),           // windows-1253:D5
                    c("GREEK CAPITAL LETTER PHI"),               // windows-1253:D6
                    c("GREEK CAPITAL LETTER CHI"),               // windows-1253:D7
                    c("GREEK CAPITAL LETTER PSI"),               // windows-1253:D8
                    c("GREEK CAPITAL LETTER OMEGA"),             // windows-1253:D9
                    c("GREEK CAPITAL LETTER IOTA WITH DIALYTIKA"), // windows-1253:DA
                    c("GREEK CAPITAL LETTER UPSILON WITH DIALYTIKA"), // windows-1253:DB
                    c("GREEK SMALL LETTER ALPHA WITH TONOS"),    // windows-1253:DC
                    c("GREEK SMALL LETTER EPSILON WITH TONOS"),  // windows-1253:DD
                    c("GREEK SMALL LETTER ETA WITH TONOS"),      // windows-1253:DE
                    c("GREEK SMALL LETTER IOTA WITH TONOS"),     // windows-1253:DF
                    c("CYRILLIC CAPITAL LETTER VE"),             // windows-1251:C2
                    c("CYRILLIC CAPITAL LETTER GHE"),            // windows-1251:C3
                    c("CYRILLIC CAPITAL LETTER DE"),             // windows-1251:C4
                    c("CYRILLIC CAPITAL LETTER IE"),             // windows-1251:C5
                    c("CYRILLIC CAPITAL LETTER ZHE"),            // windows-1251:C6
                    c("CYRILLIC CAPITAL LETTER ZE"),             // windows-1251:C7
                    c("CYRILLIC CAPITAL LETTER I"),              // windows-1251:C8
                    c("CYRILLIC CAPITAL LETTER SHORT I"),        // windows-1251:C9
                    c("CYRILLIC CAPITAL LETTER KA"),             // windows-1251:CA
                    c("CYRILLIC CAPITAL LETTER EL"),             // windows-1251:CB
                    c("CYRILLIC CAPITAL LETTER EM"),             // windows-1251:CC
                    c("CYRILLIC CAPITAL LETTER EN"),             // windows-1251:CD
                    c("CYRILLIC CAPITAL LETTER O"),              // windows-1251:CE
                    c("CYRILLIC CAPITAL LETTER PE"),             // windows-1251:CF
                    c("CYRILLIC CAPITAL LETTER ER"),             // windows-1251:D0
                    c("CYRILLIC CAPITAL LETTER ES"),             // windows-1251:D1
                    c("CYRILLIC CAPITAL LETTER TE"),             // windows-1251:D2
                    c("CYRILLIC CAPITAL LETTER U"),              // windows-1251:D3
                    c("CYRILLIC CAPITAL LETTER EF"),             // windows-1251:D4
                    c("CYRILLIC CAPITAL LETTER HA"),             // windows-1251:D5
                    c("CYRILLIC CAPITAL LETTER TSE"),            // windows-1251:D6
                    c("CYRILLIC CAPITAL LETTER CHE"),            // windows-1251:D7
                    c("CYRILLIC CAPITAL LETTER SHA"),            // windows-1251:D8
                    c("CYRILLIC CAPITAL LETTER SHCHA"),          // windows-1251:D9
                    c("CYRILLIC CAPITAL LETTER HARD SIGN"),      // windows-1251:DA
                    c("CYRILLIC CAPITAL LETTER YERU"),           // windows-1251:DB
                    c("CYRILLIC CAPITAL LETTER SOFT SIGN"),      // windows-1251:DC
                    c("CYRILLIC CAPITAL LETTER E"),              // windows-1251:DD
                    c("CYRILLIC CAPITAL LETTER YU"),             // windows-1251:DE
                    c("CYRILLIC CAPITAL LETTER YA"),             // windows-1251:DF
                ]
                .join(""),
            ),
            // Letters that decode to 0xE0 - 0xEF in a Latin-1-like encoding
            (
                "utf8_first_of_3",
                vec![
                    c("LATIN SMALL LETTER A WITH ACUTE"),      // latin-1:E1
                    c("LATIN SMALL LETTER A WITH BREVE"),      // windows-1250:E3
                    c("LATIN SMALL LETTER A WITH CIRCUMFLEX"), // latin-1:E2
                    c("LATIN SMALL LETTER A WITH DIAERESIS"),  // latin-1:E4
                    c("LATIN SMALL LETTER A WITH GRAVE"),      // latin-1:E0
                    c("LATIN SMALL LETTER A WITH MACRON"),     // windows-1257:E2
                    c("LATIN SMALL LETTER A WITH OGONEK"),     // windows-1257:E0
                    c("LATIN SMALL LETTER A WITH RING ABOVE"), // latin-1:E5
                    c("LATIN SMALL LETTER A WITH TILDE"),      // latin-1:E3
                    c("LATIN SMALL LETTER AE"),                // latin-1:E6
                    c("LATIN SMALL LETTER C WITH ACUTE"),      // windows-1250:E6
                    c("LATIN SMALL LETTER C WITH CARON"),      // windows-1250:E8
                    c("LATIN SMALL LETTER C WITH CEDILLA"),    // latin-1:E7
                    c("LATIN SMALL LETTER D WITH CARON"),      // windows-1250:EF
                    c("LATIN SMALL LETTER E WITH ACUTE"),      // latin-1:E9
                    c("LATIN SMALL LETTER E WITH CARON"),      // windows-1250:EC
                    c("LATIN SMALL LETTER E WITH CIRCUMFLEX"), // latin-1:EA
                    c("LATIN SMALL LETTER E WITH DIAERESIS"),  // latin-1:EB
                    c("LATIN SMALL LETTER E WITH DOT ABOVE"),  // windows-1257:EB
                    c("LATIN SMALL LETTER E WITH GRAVE"),      // latin-1:E8
                    c("LATIN SMALL LETTER E WITH MACRON"),     // windows-1257:E7
                    c("LATIN SMALL LETTER E WITH OGONEK"),     // windows-1250:EA
                    c("LATIN SMALL LETTER E WITH OGONEK"),     // windows-1250:EA
                    c("LATIN SMALL LETTER G WITH CEDILLA"),    // windows-1257:EC
                    c("LATIN SMALL LETTER I WITH ACUTE"),      // latin-1:ED
                    c("LATIN SMALL LETTER I WITH CIRCUMFLEX"), // latin-1:EE
                    c("LATIN SMALL LETTER I WITH DIAERESIS"),  // latin-1:EF
                    c("LATIN SMALL LETTER I WITH GRAVE"),      // latin-1:EC
                    c("LATIN SMALL LETTER I WITH MACRON"),     // windows-1257:EE
                    c("LATIN SMALL LETTER I WITH OGONEK"),     // windows-1257:E1
                    c("LATIN SMALL LETTER K WITH CEDILLA"),    // windows-1257:ED
                    c("LATIN SMALL LETTER L WITH ACUTE"),      // windows-1250:E5
                    c("LATIN SMALL LETTER L WITH CEDILLA"),    // windows-1257:EF
                    c("LATIN SMALL LETTER R WITH ACUTE"),      // windows-1250:E0
                    c("LATIN SMALL LETTER Z WITH ACUTE"),      // windows-1257:EA
                    c("GREEK SMALL LETTER UPSILON WITH DIALYTIKA AND TONOS"), // windows-1253:E0
                    c("GREEK SMALL LETTER ALPHA"),             // windows-1253:E1
                    c("GREEK SMALL LETTER BETA"),              // windows-1253:E2
                    c("GREEK SMALL LETTER GAMMA"),             // windows-1253:E3
                    c("GREEK SMALL LETTER DELTA"),             // windows-1253:E4
                    c("GREEK SMALL LETTER EPSILON"),           // windows-1253:E5
                    c("GREEK SMALL LETTER ZETA"),              // windows-1253:E6
                    c("GREEK SMALL LETTER ETA"),               // windows-1253:E7
                    c("GREEK SMALL LETTER THETA"),             // windows-1253:E8
                    c("GREEK SMALL LETTER IOTA"),              // windows-1253:E9
                    c("GREEK SMALL LETTER KAPPA"),             // windows-1253:EA
                    c("GREEK SMALL LETTER LAMDA"),             // windows-1253:EB
                    c("GREEK SMALL LETTER MU"),                // windows-1253:EC
                    c("GREEK SMALL LETTER NU"),                // windows-1253:ED
                    c("GREEK SMALL LETTER XI"),                // windows-1253:EE
                    c("GREEK SMALL LETTER OMICRON"),           // windows-1253:EF
                    c("CYRILLIC SMALL LETTER A"),              // windows-1251:E0
                    c("CYRILLIC SMALL LETTER BE"),             // windows-1251:E1
                    c("CYRILLIC SMALL LETTER VE"),             // windows-1251:E2
                    c("CYRILLIC SMALL LETTER GHE"),            // windows-1251:E3
                    c("CYRILLIC SMALL LETTER DE"),             // windows-1251:E4
                    c("CYRILLIC SMALL LETTER IE"),             // windows-1251:E5
                    c("CYRILLIC SMALL LETTER ZHE"),            // windows-1251:E6
                    c("CYRILLIC SMALL LETTER ZE"),             // windows-1251:E7
                    c("CYRILLIC SMALL LETTER I"),              // windows-1251:E8
                    c("CYRILLIC SMALL LETTER SHORT I"),        // windows-1251:E9
                    c("CYRILLIC SMALL LETTER KA"),             // windows-1251:EA
                    c("CYRILLIC SMALL LETTER EL"),             // windows-1251:EB
                    c("CYRILLIC SMALL LETTER EM"),             // windows-1251:EC
                    c("CYRILLIC SMALL LETTER EN"),             // windows-1251:ED
                    c("CYRILLIC SMALL LETTER O"),              // windows-1251:EE
                    c("CYRILLIC SMALL LETTER PE"),             // windows-1251:EF
                ]
                .join(""),
            ),
            // Letters that decode to 0xF0 or 0xF3 in a Latin-1-like encoding.
            // (Other leading bytes correspond only to unassigned codepoints)
            (
                "utf8_first_of_4",
                vec![
                    c("LATIN SMALL LETTER D WITH STROKE"), // windows-1250:F0
                    c("LATIN SMALL LETTER ETH"),           // latin-1:F0
                    c("LATIN SMALL LETTER G WITH BREVE"),  // windows-1254:F0
                    c("LATIN SMALL LETTER O WITH ACUTE"),  // latin-1:F3
                    c("LATIN SMALL LETTER S WITH CARON"),  // windows-1257:F0
                    c("GREEK SMALL LETTER PI"),            // windows-1253:F0
                    c("GREEK SMALL LETTER SIGMA"),         // windows-1253:F3
                    c("CYRILLIC SMALL LETTER ER"),         // windows-1251:F0
                    c("CYRILLIC SMALL LETTER U"),          // windows-1251:F3
                ]
                .join(""),
            ),
            // Letters that decode to 0x80 - 0xBF in a Latin-1-like encoding,
            // including a space standing in for 0xA0
            (
                "utf8_continuation",
                vec![
                    r"\x80-\xbf".to_string(),
                    s("SPACE"), // modification of latin-1:A0, NO-BREAK SPACE
                    c("LATIN CAPITAL LETTER A WITH OGONEK"), // windows-1250:A5
                    c("LATIN CAPITAL LETTER AE"), // windows-1257:AF
                    c("LATIN CAPITAL LETTER L WITH CARON"), // windows-1250:BC
                    c("LATIN CAPITAL LETTER L WITH STROKE"), // windows-1250:A3
                    c("LATIN CAPITAL LETTER O WITH STROKE"), // windows-1257:A8
                    c("LATIN CAPITAL LETTER R WITH CEDILLA"), // windows-1257:AA
                    c("LATIN CAPITAL LETTER S WITH ACUTE"), // windows-1250:8C
                    c("LATIN CAPITAL LETTER S WITH CARON"), // windows-1252:8A
                    c("LATIN CAPITAL LETTER S WITH CEDILLA"), // windows-1250:AA
                    c("LATIN CAPITAL LETTER T WITH CARON"), // windows-1250:8D
                    c("LATIN CAPITAL LETTER Y WITH DIAERESIS"), // windows-1252:9F
                    c("LATIN CAPITAL LETTER Z WITH ACUTE"), // windows-1250:8F
                    c("LATIN CAPITAL LETTER Z WITH CARON"), // windows-1252:8E
                    c("LATIN CAPITAL LETTER Z WITH DOT ABOVE"), // windows-1250:AF
                    c("LATIN CAPITAL LIGATURE OE"), // windows-1252:8C
                    c("LATIN SMALL LETTER A WITH OGONEK"), // windows-1250:B9
                    c("LATIN SMALL LETTER AE"), // windows-1257:BF
                    c("LATIN SMALL LETTER F WITH HOOK"), // windows-1252:83
                    c("LATIN SMALL LETTER L WITH CARON"), // windows-1250:BE
                    c("LATIN SMALL LETTER L WITH STROKE"), // windows-1250:B3
                    c("LATIN SMALL LETTER O WITH STROKE"), // windows-1257:B8
                    c("LATIN SMALL LETTER R WITH CEDILLA"), // windows-1257:BA
                    c("LATIN SMALL LETTER S WITH ACUTE"), // windows-1250:9C
                    c("LATIN SMALL LETTER S WITH CARON"), // windows-1252:9A
                    c("LATIN SMALL LETTER S WITH CEDILLA"), // windows-1250:BA
                    c("LATIN SMALL LETTER T WITH CARON"), // windows-1250:9D
                    c("LATIN SMALL LETTER Z WITH ACUTE"), // windows-1250:9F
                    c("LATIN SMALL LETTER Z WITH CARON"), // windows-1252:9E
                    c("LATIN SMALL LETTER Z WITH DOT ABOVE"), // windows-1250:BF
                    c("LATIN SMALL LIGATURE OE"), // windows-1252:9C
                    c("MODIFIER LETTER CIRCUMFLEX ACCENT"), // windows-1252:88
                    c("CARON"), // windows-1250:A1
                    c("BREVE"), // windows-1250:A2
                    c("OGONEK"), // windows-1250:B2
                    c("SMALL TILDE"), // windows-1252:98
                    c("DOUBLE ACUTE ACCENT"), // windows-1250:BD
                    c("GREEK TONOS"), // windows-1253:B4
                    c("GREEK DIALYTIKA TONOS"), // windows-1253:A1
                    c("GREEK CAPITAL LETTER ALPHA WITH TONOS"), // windows-1253:A2
                    c("GREEK CAPITAL LETTER EPSILON WITH TONOS"), // windows-1253:B8
                    c("GREEK CAPITAL LETTER ETA WITH TONOS"), // windows-1253:B9
                    c("GREEK CAPITAL LETTER IOTA WITH TONOS"), // windows-1253:BA
                    c("GREEK CAPITAL LETTER OMICRON WITH TONOS"), // windows-1253:BC
                    c("GREEK CAPITAL LETTER UPSILON WITH TONOS"), // windows-1253:BE
                    c("GREEK CAPITAL LETTER OMEGA WITH TONOS"), // windows-1253:BF
                    c("CYRILLIC CAPITAL LETTER IO"), // windows-1251:A8
                    c("CYRILLIC CAPITAL LETTER DJE"), // windows-1251:80
                    c("CYRILLIC CAPITAL LETTER GJE"), // windows-1251:81
                    c("CYRILLIC CAPITAL LETTER UKRAINIAN IE"), // windows-1251:AA
                    c("CYRILLIC CAPITAL LETTER DZE"), // windows-1251:BD
                    c("CYRILLIC CAPITAL LETTER BYELORUSSIAN-UKRAINIAN I"), // windows-1251:B2
                    c("CYRILLIC CAPITAL LETTER YI"), // windows-1251:AF
                    c("CYRILLIC CAPITAL LETTER JE"), // windows-1251:A3
                    c("CYRILLIC CAPITAL LETTER LJE"), // windows-1251:8A
                    c("CYRILLIC CAPITAL LETTER NJE"), // windows-1251:8C
                    c("CYRILLIC CAPITAL LETTER TSHE"), // windows-1251:8E
                    c("CYRILLIC CAPITAL LETTER KJE"), // windows-1251:8D
                    c("CYRILLIC CAPITAL LETTER SHORT U"), // windows-1251:A1
                    c("CYRILLIC CAPITAL LETTER DZHE"), // windows-1251:8F
                    c("CYRILLIC SMALL LETTER IO"), // windows-1251:B8
                    c("CYRILLIC SMALL LETTER DJE"), // windows-1251:90
                    c("CYRILLIC SMALL LETTER GJE"), // windows-1251:83
                    c("CYRILLIC SMALL LETTER UKRAINIAN IE"), // windows-1251:BA
                    c("CYRILLIC SMALL LETTER DZE"), // windows-1251:BE
                    c("CYRILLIC SMALL LETTER BYELORUSSIAN-UKRAINIAN I"), // windows-1251:B3
                    c("CYRILLIC SMALL LETTER YI"), // windows-1251:BF
                    c("CYRILLIC SMALL LETTER JE"), // windows-1251:BC
                    c("CYRILLIC SMALL LETTER LJE"), // windows-1251:9A
                    c("CYRILLIC SMALL LETTER NJE"), // windows-1251:9C
                    c("CYRILLIC SMALL LETTER TSHE"), // windows-1251:9E
                    c("CYRILLIC SMALL LETTER KJE"), // windows-1251:9D
                    c("CYRILLIC SMALL LETTER SHORT U"), // windows-1251:A2
                    c("CYRILLIC SMALL LETTER DZHE"), // windows-1251:9F
                    c("CYRILLIC CAPITAL LETTER GHE WITH UPTURN"), // windows-1251:A5
                    c("CYRILLIC SMALL LETTER GHE WITH UPTURN"), // windows-1251:B4
                    c("EN DASH"), // windows-1252:96
                    c("EM DASH"), // windows-1252:97
                    c("HORIZONTAL BAR"), // windows-1253:AF
                    c("LEFT SINGLE QUOTATION MARK"), // windows-1252:91
                    c("RIGHT SINGLE QUOTATION MARK"), // windows-1252:92
                    c("SINGLE LOW-9 QUOTATION MARK"), // windows-1252:82
                    c("LEFT DOUBLE QUOTATION MARK"), // windows-1252:93
                    c("RIGHT DOUBLE QUOTATION MARK"), // windows-1252:94
                    c("DOUBLE LOW-9 QUOTATION MARK"), // windows-1252:84
                    c("DAGGER"), // windows-1252:86
                    c("DOUBLE DAGGER"), // windows-1252:87
                    c("BULLET"), // windows-1252:95
                    c("HORIZONTAL ELLIPSIS"), // windows-1252:85
                    c("PER MILLE SIGN"), // windows-1252:89
                    c("SINGLE LEFT-POINTING ANGLE QUOTATION MARK"), // windows-1252:8B
                    c("SINGLE RIGHT-POINTING ANGLE QUOTATION MARK"), // windows-1252:9B
                    c("EURO SIGN"), // windows-1252:80
                    c("NUMERO SIGN"), // windows-1251:B9
                    c("TRADE MARK SIGN"), // windows-1252:99
                ]
                .join(""),
            ),
            // Letters that decode to 0x80 - 0xBF in a Latin-1-like encoding,
            // and don't usually stand for themselves when adjacent to mojibake.
            // This excludes spaces, dashes, 'bullet', quotation marks, and ellipses.
            (
                "utf8_continuation_strict",
                vec![
                    r"\x80-\xbf".to_string(),
                    c("LATIN CAPITAL LETTER A WITH OGONEK"), // windows-1250:A5
                    c("LATIN CAPITAL LETTER AE"),            // windows-1257:AF
                    c("LATIN CAPITAL LETTER L WITH CARON"),  // windows-1250:BC
                    c("LATIN CAPITAL LETTER L WITH STROKE"), // windows-1250:A3
                    c("LATIN CAPITAL LETTER O WITH STROKE"), // windows-1257:A8
                    c("LATIN CAPITAL LETTER R WITH CEDILLA"), // windows-1257:AA
                    c("LATIN CAPITAL LETTER S WITH ACUTE"),  // windows-1250:8C
                    c("LATIN CAPITAL LETTER S WITH CARON"),  // windows-1252:8A
                    c("LATIN CAPITAL LETTER S WITH CEDILLA"), // windows-1250:AA
                    c("LATIN CAPITAL LETTER T WITH CARON"),  // windows-1250:8D
                    c("LATIN CAPITAL LETTER Y WITH DIAERESIS"), // windows-1252:9F
                    c("LATIN CAPITAL LETTER Z WITH ACUTE"),  // windows-1250:8F
                    c("LATIN CAPITAL LETTER Z WITH CARON"),  // windows-1252:8E
                    c("LATIN CAPITAL LETTER Z WITH DOT ABOVE"), // windows-1250:AF
                    c("LATIN CAPITAL LIGATURE OE"),          // windows-1252:8C
                    c("LATIN SMALL LETTER A WITH OGONEK"),   // windows-1250:B9
                    c("LATIN SMALL LETTER AE"),              // windows-1257:BF
                    c("LATIN SMALL LETTER F WITH HOOK"),     // windows-1252:83
                    c("LATIN SMALL LETTER L WITH CARON"),    // windows-1250:BE
                    c("LATIN SMALL LETTER L WITH STROKE"),   // windows-1250:B3
                    c("LATIN SMALL LETTER O WITH STROKE"),   // windows-1257:B8
                    c("LATIN SMALL LETTER R WITH CEDILLA"),  // windows-1257:BA
                    c("LATIN SMALL LETTER S WITH ACUTE"),    // windows-1250:9C
                    c("LATIN SMALL LETTER S WITH CARON"),    // windows-1252:9A
                    c("LATIN SMALL LETTER S WITH CEDILLA"),  // windows-1250:BA
                    c("LATIN SMALL LETTER T WITH CARON"),    // windows-1250:9D
                    c("LATIN SMALL LETTER Z WITH ACUTE"),    // windows-1250:9F
                    c("LATIN SMALL LETTER Z WITH CARON"),    // windows-1252:9E
                    c("LATIN SMALL LETTER Z WITH DOT ABOVE"), // windows-1250:BF
                    c("LATIN SMALL LIGATURE OE"),            // windows-1252:9C
                    c("MODIFIER LETTER CIRCUMFLEX ACCENT"),  // windows-1252:88
                    c("CARON"),                              // windows-1250:A1
                    c("BREVE"),                              // windows-1250:A2
                    c("OGONEK"),                             // windows-1250:B2
                    c("SMALL TILDE"),                        // windows-1252:98
                    c("DOUBLE ACUTE ACCENT"),                // windows-1250:BD
                    c("GREEK TONOS"),                        // windows-1253:B4
                    c("GREEK DIALYTIKA TONOS"),              // windows-1253:A1
                    c("GREEK CAPITAL LETTER ALPHA WITH TONOS"), // windows-1253:A2
                    c("GREEK CAPITAL LETTER EPSILON WITH TONOS"), // windows-1253:B8
                    c("GREEK CAPITAL LETTER ETA WITH TONOS"), // windows-1253:B9
                    c("GREEK CAPITAL LETTER IOTA WITH TONOS"), // windows-1253:BA
                    c("GREEK CAPITAL LETTER OMICRON WITH TONOS"), // windows-1253:BC
                    c("GREEK CAPITAL LETTER UPSILON WITH TONOS"), // windows-1253:BE
                    c("GREEK CAPITAL LETTER OMEGA WITH TONOS"), // windows-1253:BF
                    c("CYRILLIC CAPITAL LETTER IO"),         // windows-1251:A8
                    c("CYRILLIC CAPITAL LETTER DJE"),        // windows-1251:80
                    c("CYRILLIC CAPITAL LETTER GJE"),        // windows-1251:81
                    c("CYRILLIC CAPITAL LETTER UKRAINIAN IE"), // windows-1251:AA
                    c("CYRILLIC CAPITAL LETTER DZE"),        // windows-1251:BD
                    c("CYRILLIC CAPITAL LETTER BYELORUSSIAN-UKRAINIAN I"), // windows-1251:B2
                    c("CYRILLIC CAPITAL LETTER YI"),         // windows-1251:AF
                    c("CYRILLIC CAPITAL LETTER JE"),         // windows-1251:A3
                    c("CYRILLIC CAPITAL LETTER LJE"),        // windows-1251:8A
                    c("CYRILLIC CAPITAL LETTER NJE"),        // windows-1251:8C
                    c("CYRILLIC CAPITAL LETTER TSHE"),       // windows-1251:8E
                    c("CYRILLIC CAPITAL LETTER KJE"),        // windows-1251:8D
                    c("CYRILLIC CAPITAL LETTER SHORT U"),    // windows-1251:A1
                    c("CYRILLIC CAPITAL LETTER DZHE"),       // windows-1251:8F
                    c("CYRILLIC SMALL LETTER IO"),           // windows-1251:B8
                    c("CYRILLIC SMALL LETTER DJE"),          // windows-1251:90
                    c("CYRILLIC SMALL LETTER GJE"),          // windows-1251:83
                    c("CYRILLIC SMALL LETTER UKRAINIAN IE"), // windows-1251:BA
                    c("CYRILLIC SMALL LETTER DZE"),          // windows-1251:BE
                    c("CYRILLIC SMALL LETTER BYELORUSSIAN-UKRAINIAN I"), // windows-1251:B3
                    c("CYRILLIC SMALL LETTER YI"),           // windows-1251:BF
                    c("CYRILLIC SMALL LETTER JE"),           // windows-1251:BC
                    c("CYRILLIC SMALL LETTER LJE"),          // windows-1251:9A
                    c("CYRILLIC SMALL LETTER NJE"),          // windows-1251:9C
                    c("CYRILLIC SMALL LETTER TSHE"),         // windows-1251:9E
                    c("CYRILLIC SMALL LETTER KJE"),          // windows-1251:9D
                    c("CYRILLIC SMALL LETTER SHORT U"),      // windows-1251:A2
                    c("CYRILLIC SMALL LETTER DZHE"),         // windows-1251:9F
                    c("CYRILLIC CAPITAL LETTER GHE WITH UPTURN"), // windows-1251:A5
                    c("CYRILLIC SMALL LETTER GHE WITH UPTURN"), // windows-1251:B4
                    c("DAGGER"),                             // windows-1252:86
                    c("DOUBLE DAGGER"),                      // windows-1252:87
                    c("PER MILLE SIGN"),                     // windows-1252:89
                    c("SINGLE LEFT-POINTING ANGLE QUOTATION MARK"), // windows-1252:8B
                    c("SINGLE RIGHT-POINTING ANGLE QUOTATION MARK"), // windows-1252:9B
                    c("EURO SIGN"),                          // windows-1252:80
                    c("NUMERO SIGN"),                        // windows-1251:B9
                    c("TRADE MARK SIGN"),                    // windows-1252:99
                ]
                .join(""),
            ),
        ]
        .into_iter()
        .collect();

        let ours: std::collections::BTreeMap<&str, String> = UTF8_CLUES
            .iter()
            .map(|(&name, &class)| (name, class.to_string()))
            .collect();

        assert_eq!(ours, expected);
    }

    #[test]
    fn test_strict_continuation_set_matches_clue() {
        // Drift check between the build.rs-generated phf set and the
        // `utf8_continuation_strict` clue string (which is itself pinned to
        // ftfy by test_utf8_clues_match_ftfy). The two are independent
        // representations of the same data — if either changes, this fires.
        let clue = UTF8_CLUES["utf8_continuation_strict"];
        let mut expected: std::collections::BTreeSet<char> = ('\u{80}'..='\u{bf}').collect();
        for c in clue.chars().skip(r"\x80-\xbf".len()) {
            expected.insert(c);
        }
        let actual: std::collections::BTreeSet<char> = super::UTF8_CONTINUATION_STRICT_SET
            .iter()
            .copied()
            .collect();
        assert_eq!(actual, expected);
    }
}
