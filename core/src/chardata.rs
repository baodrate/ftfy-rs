use rustc_hash::{FxHashMap, FxHashSet};
use unicode_normalization::UnicodeNormalization;

use regex::Regex;

use crate::codecs::sloppy::{
    Codec, CodecType, CP437, ISO_8859_2, LATIN_1, MACROMAN, SLOPPY_WINDOWS_1250,
    SLOPPY_WINDOWS_1251, SLOPPY_WINDOWS_1252, SLOPPY_WINDOWS_1253, SLOPPY_WINDOWS_1254,
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
        codecs.push((CodecType::SloppyWindows1250, &*SLOPPY_WINDOWS_1250));
        codecs.push((CodecType::SloppyWindows1251, &*SLOPPY_WINDOWS_1251));
        codecs.push((CodecType::SloppyWindows1253, &*SLOPPY_WINDOWS_1253));
        codecs.push((CodecType::SloppyWindows1254, &*SLOPPY_WINDOWS_1254));
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

    pub static ref CONTROL_CHARS: FxHashSet<u32> ={
        /*
        Build a translate mapping that strips likely-unintended control characters.
        See `plsfix::fixes::remove_control_chars` for a description of these
        codepoint ranges and why they should be removed.
        */
        let mut control_chars: FxHashSet<u32> = FxHashSet::default();

        let ranges = vec![
            0x00..0x09,
            0x0B..0x0C,
            0x0E..0x20,
            0x7F..0x80,
            0x206A..0x2070,
            0xFFF9..0xFFFD,
        ];

        for range in ranges {
            for i in range {
                control_chars.insert(i);
            }
        }

        control_chars.insert(0x0B);
        control_chars.insert(0x7F);
        control_chars.insert(0xFEFF);

        control_chars
    };


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
    pub static ref C1_CONTROL_RE: fancy_regex::Regex =
        fancy_regex::Regex::new(r"[\x80-\x9f]").unwrap();

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
    pub static ref UTF8_DETECTOR_RE: fancy_regex::Regex = {
        fancy_regex::Regex::new(
        &format!(
            r"(?<![{utf8_continuation_strict}])
(
[{utf8_first_of_2}][{utf8_continuation}]
|
[{utf8_first_of_3}][{utf8_continuation}]{{2}}
|
[{utf8_first_of_4}][{utf8_continuation}]{{3}}
)+",
            // Letters that decode to 0x80 - 0xBF in a Latin-1-like encoding,
            // and don't usually stand for themselves when adjacent to mojibake.
            // This excludes spaces, dashes, quotation marks, and ellipses.
            utf8_continuation_strict = r"\x80-\xbfĄąĽľŁłŒœŚśŞşŠšŤťŸŹźŻżŽžƒˆˇ˘˛˜˝΄΅ΆΈΉΊΌΎΏЁЂЃЄЅІЇЈЉЊЋЌЎЏёђѓєѕіїјљњћќўџҐґ†‡•‰‹›€№™",
            // Letters that decode to 0xC2 - 0xDF in a Latin-1-like encoding
            utf8_first_of_2 = "ÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞßĂĆČĎĐĘĚĞİĹŃŇŐŘŞŢŮŰΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩΪΫάέήίВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ",
            // Letters that decode to 0xE0 - 0xEF in a Latin-1-like encoding
            utf8_first_of_3 = "àáâãäåæçèéêëìíîïăćčďęěĺŕΰαβγδεζηθικλμνξοабвгдежзийклмноп",
            // Letters that decode to 0xF0 or 0xF3 in a Latin-1-like encoding.
            // # (Other leading bytes correspond only to unassigned codepoints)
            utf8_first_of_4 = "ðóđğπσру",
            // Letters that decode to 0x80 - 0xBF in a Latin-1-like encoding,
            // including a space standing in for 0xA0
            utf8_continuation = r"\x80-\xbfĄąĽľŁłŒœŚśŞşŠšŤťŸŹźŻżŽžƒˆˇ˘˛˜˝΄΅ΆΈΉΊΌΎΏЁЂЃЄЅІЇЈЉЊЋЌЎЏёђѓєѕіїјљњћќўџҐґ–—―‘’‚“”„†‡•…‰‹›€№™ "
        )
        .replace("\n", ""),
    )
    .expect("Failed to compile the regex")
    };
}
