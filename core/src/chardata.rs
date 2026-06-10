use rustc_hash::{FxHashMap, FxHashSet};
use unicode_normalization::UnicodeNormalization;

use regex::Regex;

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
    pub static ref HTML_ENTITIES: FxHashMap<String, String> = {
        let mut entities: FxHashMap<String, String> = FxHashMap::default();

        // First pass: insert all the real HTML5 entities.
        for (name, char) in HTML_ITEMS {
            if name.ends_with(";") {
                entities.insert(format!("&{}", name), char.to_string());
            }
        }

        // Second pass: decode lower-case entities that were uppercased, e.g. &NTILDE; as "Ñ".
        // Only add an alias if its key isn't already a real entity
        // i.e. &dd;→ⅆ must not overwrite the real &DD;→ⅅ
        for (name, char) in HTML_ITEMS {
            if name == name.to_lowercase() {
                let entity_upper = format!("&{}", name.to_uppercase());
                entities.entry(entity_upper).or_insert_with(|| char.to_uppercase());
            }
        }
        entities
    };

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
    pub static ref UTF8_DETECTOR_RE: fancy_regex::Regex = {
        fancy_regex::Regex::new(
        &format!(
            r"(?x)
            (?<! [{utf8_continuation_strict}])
            (
                [{utf8_first_of_2}] [{utf8_continuation}]
                |
                [{utf8_first_of_3}] [{utf8_continuation}]{{2}}
                |
                [{utf8_first_of_4}] [{utf8_continuation}]{{3}}
            )+",
            utf8_continuation_strict = UTF8_CLUES["utf8_continuation_strict"],
            utf8_first_of_2 = UTF8_CLUES["utf8_first_of_2"],
            utf8_first_of_3 = UTF8_CLUES["utf8_first_of_3"],
            utf8_first_of_4 = UTF8_CLUES["utf8_first_of_4"],
            utf8_continuation = UTF8_CLUES["utf8_continuation"],
        ),
    )
    .expect("Failed to compile the regex")
    };
}

/// from https://html.spec.whatwg.org/multipage/named-characters.html
static HTML_ITEMS: [(&str, &str); 2231] = [
    ("Aacute", "\u{00c1}"),
    ("Aacute;", "\u{00c1}"),
    ("aacute", "\u{00e1}"),
    ("aacute;", "\u{00e1}"),
    ("Abreve;", "\u{0102}"),
    ("abreve;", "\u{0103}"),
    ("acd;", "\u{223f}"),
    ("acE;", "\u{223e}\u{0333}"),
    ("Acirc", "\u{00c2}"),
    ("Acirc;", "\u{00c2}"),
    ("acirc", "\u{00e2}"),
    ("acirc;", "\u{00e2}"),
    ("ac;", "\u{223e}"),
    ("acute", "\u{00b4}"),
    ("acute;", "\u{00b4}"),
    ("Acy;", "\u{0410}"),
    ("acy;", "\u{0430}"),
    ("AElig", "\u{00c6}"),
    ("AElig;", "\u{00c6}"),
    ("aelig", "\u{00e6}"),
    ("aelig;", "\u{00e6}"),
    ("Afr;", "\u{1d504}"),
    ("afr;", "\u{1d51e}"),
    ("af;", "\u{2061}"),
    ("Agrave", "\u{00c0}"),
    ("Agrave;", "\u{00c0}"),
    ("agrave", "\u{00e0}"),
    ("agrave;", "\u{00e0}"),
    ("alefsym;", "\u{2135}"),
    ("aleph;", "\u{2135}"),
    ("Alpha;", "\u{0391}"),
    ("alpha;", "\u{03b1}"),
    ("Amacr;", "\u{0100}"),
    ("amacr;", "\u{0101}"),
    ("amalg;", "\u{2a3f}"),
    ("amp", "\u{0026}"),
    ("amp;", "\u{0026}"),
    ("AMP", "\u{0026}"),
    ("AMP;", "\u{0026}"),
    ("andand;", "\u{2a55}"),
    ("andd;", "\u{2a5c}"),
    ("andslope;", "\u{2a58}"),
    ("and;", "\u{2227}"),
    ("And;", "\u{2a53}"),
    ("andv;", "\u{2a5a}"),
    ("ange;", "\u{29a4}"),
    ("angle;", "\u{2220}"),
    ("angmsdaa;", "\u{29a8}"),
    ("angmsdab;", "\u{29a9}"),
    ("angmsdac;", "\u{29aa}"),
    ("angmsdad;", "\u{29ab}"),
    ("angmsdae;", "\u{29ac}"),
    ("angmsdaf;", "\u{29ad}"),
    ("angmsdag;", "\u{29ae}"),
    ("angmsdah;", "\u{29af}"),
    ("angmsd;", "\u{2221}"),
    ("angrt;", "\u{221f}"),
    ("angrtvbd;", "\u{299d}"),
    ("angrtvb;", "\u{22be}"),
    ("angsph;", "\u{2222}"),
    ("angst;", "\u{00c5}"),
    ("ang;", "\u{2220}"),
    ("angzarr;", "\u{237c}"),
    ("Aogon;", "\u{0104}"),
    ("aogon;", "\u{0105}"),
    ("Aopf;", "\u{1d538}"),
    ("aopf;", "\u{1d552}"),
    ("apacir;", "\u{2a6f}"),
    ("ape;", "\u{224a}"),
    ("apE;", "\u{2a70}"),
    ("apid;", "\u{224b}"),
    ("apos;", "\u{0027}"),
    ("ApplyFunction;", "\u{2061}"),
    ("approxeq;", "\u{224a}"),
    ("approx;", "\u{2248}"),
    ("ap;", "\u{2248}"),
    ("Aring", "\u{00c5}"),
    ("Aring;", "\u{00c5}"),
    ("aring", "\u{00e5}"),
    ("aring;", "\u{00e5}"),
    ("Ascr;", "\u{1d49c}"),
    ("ascr;", "\u{1d4b6}"),
    ("Assign;", "\u{2254}"),
    ("ast;", "\u{002a}"),
    ("asympeq;", "\u{224d}"),
    ("asymp;", "\u{2248}"),
    ("Atilde", "\u{00c3}"),
    ("Atilde;", "\u{00c3}"),
    ("atilde", "\u{00e3}"),
    ("atilde;", "\u{00e3}"),
    ("Auml", "\u{00c4}"),
    ("Auml;", "\u{00c4}"),
    ("auml", "\u{00e4}"),
    ("auml;", "\u{00e4}"),
    ("awconint;", "\u{2233}"),
    ("awint;", "\u{2a11}"),
    ("backcong;", "\u{224c}"),
    ("backepsilon;", "\u{03f6}"),
    ("backprime;", "\u{2035}"),
    ("backsimeq;", "\u{22cd}"),
    ("backsim;", "\u{223d}"),
    ("Backslash;", "\u{2216}"),
    ("barvee;", "\u{22bd}"),
    ("Barv;", "\u{2ae7}"),
    ("barwedge;", "\u{2305}"),
    ("barwed;", "\u{2305}"),
    ("Barwed;", "\u{2306}"),
    ("bbrktbrk;", "\u{23b6}"),
    ("bbrk;", "\u{23b5}"),
    ("bcong;", "\u{224c}"),
    ("Bcy;", "\u{0411}"),
    ("bcy;", "\u{0431}"),
    ("bdquo;", "\u{201e}"),
    ("because;", "\u{2235}"),
    ("Because;", "\u{2235}"),
    ("becaus;", "\u{2235}"),
    ("bemptyv;", "\u{29b0}"),
    ("bepsi;", "\u{03f6}"),
    ("Bernoullis;", "\u{212c}"),
    ("bernou;", "\u{212c}"),
    ("Beta;", "\u{0392}"),
    ("beta;", "\u{03b2}"),
    ("beth;", "\u{2136}"),
    ("between;", "\u{226c}"),
    ("Bfr;", "\u{1d505}"),
    ("bfr;", "\u{1d51f}"),
    ("bigcap;", "\u{22c2}"),
    ("bigcirc;", "\u{25ef}"),
    ("bigcup;", "\u{22c3}"),
    ("bigodot;", "\u{2a00}"),
    ("bigoplus;", "\u{2a01}"),
    ("bigotimes;", "\u{2a02}"),
    ("bigsqcup;", "\u{2a06}"),
    ("bigstar;", "\u{2605}"),
    ("bigtriangledown;", "\u{25bd}"),
    ("bigtriangleup;", "\u{25b3}"),
    ("biguplus;", "\u{2a04}"),
    ("bigvee;", "\u{22c1}"),
    ("bigwedge;", "\u{22c0}"),
    ("bkarow;", "\u{290d}"),
    ("blacklozenge;", "\u{29eb}"),
    ("blacksquare;", "\u{25aa}"),
    ("blacktriangledown;", "\u{25be}"),
    ("blacktriangleleft;", "\u{25c2}"),
    ("blacktriangleright;", "\u{25b8}"),
    ("blacktriangle;", "\u{25b4}"),
    ("blank;", "\u{2423}"),
    ("blk12;", "\u{2592}"),
    ("blk14;", "\u{2591}"),
    ("blk34;", "\u{2593}"),
    ("block;", "\u{2588}"),
    ("bnequiv;", "\u{2261}\u{20e5}"),
    ("bne;", "\u{003d}\u{20e5}"),
    ("bnot;", "\u{2310}"),
    ("bNot;", "\u{2aed}"),
    ("Bopf;", "\u{1d539}"),
    ("bopf;", "\u{1d553}"),
    ("bottom;", "\u{22a5}"),
    ("bot;", "\u{22a5}"),
    ("bowtie;", "\u{22c8}"),
    ("boxbox;", "\u{29c9}"),
    ("boxdl;", "\u{2510}"),
    ("boxdL;", "\u{2555}"),
    ("boxDl;", "\u{2556}"),
    ("boxDL;", "\u{2557}"),
    ("boxdr;", "\u{250c}"),
    ("boxdR;", "\u{2552}"),
    ("boxDr;", "\u{2553}"),
    ("boxDR;", "\u{2554}"),
    ("boxhd;", "\u{252c}"),
    ("boxHd;", "\u{2564}"),
    ("boxhD;", "\u{2565}"),
    ("boxHD;", "\u{2566}"),
    ("boxh;", "\u{2500}"),
    ("boxH;", "\u{2550}"),
    ("boxhu;", "\u{2534}"),
    ("boxHu;", "\u{2567}"),
    ("boxhU;", "\u{2568}"),
    ("boxHU;", "\u{2569}"),
    ("boxminus;", "\u{229f}"),
    ("boxplus;", "\u{229e}"),
    ("boxtimes;", "\u{22a0}"),
    ("boxul;", "\u{2518}"),
    ("boxuL;", "\u{255b}"),
    ("boxUl;", "\u{255c}"),
    ("boxUL;", "\u{255d}"),
    ("boxur;", "\u{2514}"),
    ("boxuR;", "\u{2558}"),
    ("boxUr;", "\u{2559}"),
    ("boxUR;", "\u{255a}"),
    ("boxvh;", "\u{253c}"),
    ("boxvH;", "\u{256a}"),
    ("boxVh;", "\u{256b}"),
    ("boxVH;", "\u{256c}"),
    ("boxvl;", "\u{2524}"),
    ("boxvL;", "\u{2561}"),
    ("boxVl;", "\u{2562}"),
    ("boxVL;", "\u{2563}"),
    ("boxvr;", "\u{251c}"),
    ("boxvR;", "\u{255e}"),
    ("boxVr;", "\u{255f}"),
    ("boxVR;", "\u{2560}"),
    ("boxv;", "\u{2502}"),
    ("boxV;", "\u{2551}"),
    ("bprime;", "\u{2035}"),
    ("breve;", "\u{02d8}"),
    ("Breve;", "\u{02d8}"),
    ("brvbar", "\u{00a6}"),
    ("brvbar;", "\u{00a6}"),
    ("bscr;", "\u{1d4b7}"),
    ("Bscr;", "\u{212c}"),
    ("bsemi;", "\u{204f}"),
    ("bsime;", "\u{22cd}"),
    ("bsim;", "\u{223d}"),
    ("bsolb;", "\u{29c5}"),
    ("bsolhsub;", "\u{27c8}"),
    ("bsol;", "\u{005c}"),
    ("bullet;", "\u{2022}"),
    ("bull;", "\u{2022}"),
    ("Bumpeq;", "\u{224e}"),
    ("bumpeq;", "\u{224f}"),
    ("bumpe;", "\u{224f}"),
    ("bumpE;", "\u{2aae}"),
    ("bump;", "\u{224e}"),
    ("Cacute;", "\u{0106}"),
    ("cacute;", "\u{0107}"),
    ("capand;", "\u{2a44}"),
    ("capbrcup;", "\u{2a49}"),
    ("capcap;", "\u{2a4b}"),
    ("capcup;", "\u{2a47}"),
    ("capdot;", "\u{2a40}"),
    ("CapitalDifferentialD;", "\u{2145}"),
    ("caps;", "\u{2229}\u{fe00}"),
    ("cap;", "\u{2229}"),
    ("Cap;", "\u{22d2}"),
    ("caret;", "\u{2041}"),
    ("caron;", "\u{02c7}"),
    ("Cayleys;", "\u{212d}"),
    ("ccaps;", "\u{2a4d}"),
    ("Ccaron;", "\u{010c}"),
    ("ccaron;", "\u{010d}"),
    ("Ccedil", "\u{00c7}"),
    ("Ccedil;", "\u{00c7}"),
    ("ccedil", "\u{00e7}"),
    ("ccedil;", "\u{00e7}"),
    ("Ccirc;", "\u{0108}"),
    ("ccirc;", "\u{0109}"),
    ("Cconint;", "\u{2230}"),
    ("ccupssm;", "\u{2a50}"),
    ("ccups;", "\u{2a4c}"),
    ("Cdot;", "\u{010a}"),
    ("cdot;", "\u{010b}"),
    ("Cedilla;", "\u{00b8}"),
    ("cedil", "\u{00b8}"),
    ("cedil;", "\u{00b8}"),
    ("cemptyv;", "\u{29b2}"),
    ("centerdot;", "\u{00b7}"),
    ("CenterDot;", "\u{00b7}"),
    ("cent", "\u{00a2}"),
    ("cent;", "\u{00a2}"),
    ("cfr;", "\u{1d520}"),
    ("Cfr;", "\u{212d}"),
    ("CHcy;", "\u{0427}"),
    ("chcy;", "\u{0447}"),
    ("checkmark;", "\u{2713}"),
    ("check;", "\u{2713}"),
    ("Chi;", "\u{03a7}"),
    ("chi;", "\u{03c7}"),
    ("circeq;", "\u{2257}"),
    ("circlearrowleft;", "\u{21ba}"),
    ("circlearrowright;", "\u{21bb}"),
    ("circledast;", "\u{229b}"),
    ("circledcirc;", "\u{229a}"),
    ("circleddash;", "\u{229d}"),
    ("CircleDot;", "\u{2299}"),
    ("circledR;", "\u{00ae}"),
    ("circledS;", "\u{24c8}"),
    ("CircleMinus;", "\u{2296}"),
    ("CirclePlus;", "\u{2295}"),
    ("CircleTimes;", "\u{2297}"),
    ("circ;", "\u{02c6}"),
    ("cire;", "\u{2257}"),
    ("cirE;", "\u{29c3}"),
    ("cirfnint;", "\u{2a10}"),
    ("cirmid;", "\u{2aef}"),
    ("cirscir;", "\u{29c2}"),
    ("cir;", "\u{25cb}"),
    ("ClockwiseContourIntegral;", "\u{2232}"),
    ("CloseCurlyDoubleQuote;", "\u{201d}"),
    ("CloseCurlyQuote;", "\u{2019}"),
    ("clubs;", "\u{2663}"),
    ("clubsuit;", "\u{2663}"),
    ("coloneq;", "\u{2254}"),
    ("colone;", "\u{2254}"),
    ("Colone;", "\u{2a74}"),
    ("colon;", "\u{003a}"),
    ("Colon;", "\u{2237}"),
    ("commat;", "\u{0040}"),
    ("comma;", "\u{002c}"),
    ("compfn;", "\u{2218}"),
    ("complement;", "\u{2201}"),
    ("complexes;", "\u{2102}"),
    ("comp;", "\u{2201}"),
    ("congdot;", "\u{2a6d}"),
    ("Congruent;", "\u{2261}"),
    ("cong;", "\u{2245}"),
    ("conint;", "\u{222e}"),
    ("Conint;", "\u{222f}"),
    ("ContourIntegral;", "\u{222e}"),
    ("copf;", "\u{1d554}"),
    ("Copf;", "\u{2102}"),
    ("coprod;", "\u{2210}"),
    ("Coproduct;", "\u{2210}"),
    ("copysr;", "\u{2117}"),
    ("copy", "\u{00a9}"),
    ("copy;", "\u{00a9}"),
    ("COPY", "\u{00a9}"),
    ("COPY;", "\u{00a9}"),
    ("CounterClockwiseContourIntegral;", "\u{2233}"),
    ("crarr;", "\u{21b5}"),
    ("cross;", "\u{2717}"),
    ("Cross;", "\u{2a2f}"),
    ("Cscr;", "\u{1d49e}"),
    ("cscr;", "\u{1d4b8}"),
    ("csube;", "\u{2ad1}"),
    ("csub;", "\u{2acf}"),
    ("csupe;", "\u{2ad2}"),
    ("csup;", "\u{2ad0}"),
    ("ctdot;", "\u{22ef}"),
    ("cudarrl;", "\u{2938}"),
    ("cudarrr;", "\u{2935}"),
    ("cuepr;", "\u{22de}"),
    ("cuesc;", "\u{22df}"),
    ("cularrp;", "\u{293d}"),
    ("cularr;", "\u{21b6}"),
    ("cupbrcap;", "\u{2a48}"),
    ("CupCap;", "\u{224d}"),
    ("cupcap;", "\u{2a46}"),
    ("cupcup;", "\u{2a4a}"),
    ("cupdot;", "\u{228d}"),
    ("cupor;", "\u{2a45}"),
    ("cups;", "\u{222a}\u{fe00}"),
    ("cup;", "\u{222a}"),
    ("Cup;", "\u{22d3}"),
    ("curarrm;", "\u{293c}"),
    ("curarr;", "\u{21b7}"),
    ("curlyeqprec;", "\u{22de}"),
    ("curlyeqsucc;", "\u{22df}"),
    ("curlyvee;", "\u{22ce}"),
    ("curlywedge;", "\u{22cf}"),
    ("curren", "\u{00a4}"),
    ("curren;", "\u{00a4}"),
    ("curvearrowleft;", "\u{21b6}"),
    ("curvearrowright;", "\u{21b7}"),
    ("cuvee;", "\u{22ce}"),
    ("cuwed;", "\u{22cf}"),
    ("cwconint;", "\u{2232}"),
    ("cwint;", "\u{2231}"),
    ("cylcty;", "\u{232d}"),
    ("dagger;", "\u{2020}"),
    ("Dagger;", "\u{2021}"),
    ("daleth;", "\u{2138}"),
    ("darr;", "\u{2193}"),
    ("Darr;", "\u{21a1}"),
    ("dArr;", "\u{21d3}"),
    ("dash;", "\u{2010}"),
    ("dashv;", "\u{22a3}"),
    ("Dashv;", "\u{2ae4}"),
    ("dbkarow;", "\u{290f}"),
    ("dblac;", "\u{02dd}"),
    ("Dcaron;", "\u{010e}"),
    ("dcaron;", "\u{010f}"),
    ("Dcy;", "\u{0414}"),
    ("dcy;", "\u{0434}"),
    ("ddagger;", "\u{2021}"),
    ("ddarr;", "\u{21ca}"),
    ("DDotrahd;", "\u{2911}"),
    ("ddotseq;", "\u{2a77}"),
    ("DD;", "\u{2145}"),
    ("dd;", "\u{2146}"),
    ("deg", "\u{00b0}"),
    ("deg;", "\u{00b0}"),
    ("Delta;", "\u{0394}"),
    ("delta;", "\u{03b4}"),
    ("Del;", "\u{2207}"),
    ("demptyv;", "\u{29b1}"),
    ("dfisht;", "\u{297f}"),
    ("Dfr;", "\u{1d507}"),
    ("dfr;", "\u{1d521}"),
    ("dharl;", "\u{21c3}"),
    ("dharr;", "\u{21c2}"),
    ("dHar;", "\u{2965}"),
    ("DiacriticalAcute;", "\u{00b4}"),
    ("DiacriticalDot;", "\u{02d9}"),
    ("DiacriticalDoubleAcute;", "\u{02dd}"),
    ("DiacriticalGrave;", "\u{0060}"),
    ("DiacriticalTilde;", "\u{02dc}"),
    ("diamondsuit;", "\u{2666}"),
    ("diamond;", "\u{22c4}"),
    ("Diamond;", "\u{22c4}"),
    ("diams;", "\u{2666}"),
    ("diam;", "\u{22c4}"),
    ("die;", "\u{00a8}"),
    ("DifferentialD;", "\u{2146}"),
    ("digamma;", "\u{03dd}"),
    ("disin;", "\u{22f2}"),
    ("divideontimes;", "\u{22c7}"),
    ("divide", "\u{00f7}"),
    ("divide;", "\u{00f7}"),
    ("divonx;", "\u{22c7}"),
    ("div;", "\u{00f7}"),
    ("DJcy;", "\u{0402}"),
    ("djcy;", "\u{0452}"),
    ("dlcorn;", "\u{231e}"),
    ("dlcrop;", "\u{230d}"),
    ("dollar;", "\u{0024}"),
    ("Dopf;", "\u{1d53b}"),
    ("dopf;", "\u{1d555}"),
    ("DotDot;", "\u{20dc}"),
    ("doteqdot;", "\u{2251}"),
    ("doteq;", "\u{2250}"),
    ("DotEqual;", "\u{2250}"),
    ("dotminus;", "\u{2238}"),
    ("dotplus;", "\u{2214}"),
    ("dotsquare;", "\u{22a1}"),
    ("Dot;", "\u{00a8}"),
    ("dot;", "\u{02d9}"),
    ("doublebarwedge;", "\u{2306}"),
    ("DoubleContourIntegral;", "\u{222f}"),
    ("DoubleDot;", "\u{00a8}"),
    ("DoubleDownArrow;", "\u{21d3}"),
    ("DoubleLeftArrow;", "\u{21d0}"),
    ("DoubleLeftRightArrow;", "\u{21d4}"),
    ("DoubleLeftTee;", "\u{2ae4}"),
    ("DoubleLongLeftArrow;", "\u{27f8}"),
    ("DoubleLongLeftRightArrow;", "\u{27fa}"),
    ("DoubleLongRightArrow;", "\u{27f9}"),
    ("DoubleRightArrow;", "\u{21d2}"),
    ("DoubleRightTee;", "\u{22a8}"),
    ("DoubleUpArrow;", "\u{21d1}"),
    ("DoubleUpDownArrow;", "\u{21d5}"),
    ("DoubleVerticalBar;", "\u{2225}"),
    ("DownArrowBar;", "\u{2913}"),
    ("downarrow;", "\u{2193}"),
    ("DownArrow;", "\u{2193}"),
    ("Downarrow;", "\u{21d3}"),
    ("DownArrowUpArrow;", "\u{21f5}"),
    ("DownBreve;", "\u{0311}"),
    ("downdownarrows;", "\u{21ca}"),
    ("downharpoonleft;", "\u{21c3}"),
    ("downharpoonright;", "\u{21c2}"),
    ("DownLeftRightVector;", "\u{2950}"),
    ("DownLeftTeeVector;", "\u{295e}"),
    ("DownLeftVectorBar;", "\u{2956}"),
    ("DownLeftVector;", "\u{21bd}"),
    ("DownRightTeeVector;", "\u{295f}"),
    ("DownRightVectorBar;", "\u{2957}"),
    ("DownRightVector;", "\u{21c1}"),
    ("DownTeeArrow;", "\u{21a7}"),
    ("DownTee;", "\u{22a4}"),
    ("drbkarow;", "\u{2910}"),
    ("drcorn;", "\u{231f}"),
    ("drcrop;", "\u{230c}"),
    ("Dscr;", "\u{1d49f}"),
    ("dscr;", "\u{1d4b9}"),
    ("DScy;", "\u{0405}"),
    ("dscy;", "\u{0455}"),
    ("dsol;", "\u{29f6}"),
    ("Dstrok;", "\u{0110}"),
    ("dstrok;", "\u{0111}"),
    ("dtdot;", "\u{22f1}"),
    ("dtrif;", "\u{25be}"),
    ("dtri;", "\u{25bf}"),
    ("duarr;", "\u{21f5}"),
    ("duhar;", "\u{296f}"),
    ("dwangle;", "\u{29a6}"),
    ("DZcy;", "\u{040f}"),
    ("dzcy;", "\u{045f}"),
    ("dzigrarr;", "\u{27ff}"),
    ("Eacute", "\u{00c9}"),
    ("Eacute;", "\u{00c9}"),
    ("eacute", "\u{00e9}"),
    ("eacute;", "\u{00e9}"),
    ("easter;", "\u{2a6e}"),
    ("Ecaron;", "\u{011a}"),
    ("ecaron;", "\u{011b}"),
    ("Ecirc", "\u{00ca}"),
    ("Ecirc;", "\u{00ca}"),
    ("ecirc", "\u{00ea}"),
    ("ecirc;", "\u{00ea}"),
    ("ecir;", "\u{2256}"),
    ("ecolon;", "\u{2255}"),
    ("Ecy;", "\u{042d}"),
    ("ecy;", "\u{044d}"),
    ("eDDot;", "\u{2a77}"),
    ("Edot;", "\u{0116}"),
    ("edot;", "\u{0117}"),
    ("eDot;", "\u{2251}"),
    ("ee;", "\u{2147}"),
    ("efDot;", "\u{2252}"),
    ("Efr;", "\u{1d508}"),
    ("efr;", "\u{1d522}"),
    ("Egrave", "\u{00c8}"),
    ("Egrave;", "\u{00c8}"),
    ("egrave", "\u{00e8}"),
    ("egrave;", "\u{00e8}"),
    ("egsdot;", "\u{2a98}"),
    ("egs;", "\u{2a96}"),
    ("eg;", "\u{2a9a}"),
    ("Element;", "\u{2208}"),
    ("elinters;", "\u{23e7}"),
    ("ell;", "\u{2113}"),
    ("elsdot;", "\u{2a97}"),
    ("els;", "\u{2a95}"),
    ("el;", "\u{2a99}"),
    ("Emacr;", "\u{0112}"),
    ("emacr;", "\u{0113}"),
    ("emptyset;", "\u{2205}"),
    ("EmptySmallSquare;", "\u{25fb}"),
    ("empty;", "\u{2205}"),
    ("EmptyVerySmallSquare;", "\u{25ab}"),
    ("emptyv;", "\u{2205}"),
    ("emsp13;", "\u{2004}"),
    ("emsp14;", "\u{2005}"),
    ("emsp;", "\u{2003}"),
    ("ENG;", "\u{014a}"),
    ("eng;", "\u{014b}"),
    ("ensp;", "\u{2002}"),
    ("Eogon;", "\u{0118}"),
    ("eogon;", "\u{0119}"),
    ("Eopf;", "\u{1d53c}"),
    ("eopf;", "\u{1d556}"),
    ("eparsl;", "\u{29e3}"),
    ("epar;", "\u{22d5}"),
    ("eplus;", "\u{2a71}"),
    ("Epsilon;", "\u{0395}"),
    ("epsilon;", "\u{03b5}"),
    ("epsi;", "\u{03b5}"),
    ("epsiv;", "\u{03f5}"),
    ("eqcirc;", "\u{2256}"),
    ("eqcolon;", "\u{2255}"),
    ("eqsim;", "\u{2242}"),
    ("eqslantgtr;", "\u{2a96}"),
    ("eqslantless;", "\u{2a95}"),
    ("equals;", "\u{003d}"),
    ("EqualTilde;", "\u{2242}"),
    ("Equal;", "\u{2a75}"),
    ("equest;", "\u{225f}"),
    ("Equilibrium;", "\u{21cc}"),
    ("equivDD;", "\u{2a78}"),
    ("equiv;", "\u{2261}"),
    ("eqvparsl;", "\u{29e5}"),
    ("erarr;", "\u{2971}"),
    ("erDot;", "\u{2253}"),
    ("escr;", "\u{212f}"),
    ("Escr;", "\u{2130}"),
    ("esdot;", "\u{2250}"),
    ("esim;", "\u{2242}"),
    ("Esim;", "\u{2a73}"),
    ("Eta;", "\u{0397}"),
    ("eta;", "\u{03b7}"),
    ("ETH", "\u{00d0}"),
    ("ETH;", "\u{00d0}"),
    ("eth", "\u{00f0}"),
    ("eth;", "\u{00f0}"),
    ("Euml", "\u{00cb}"),
    ("Euml;", "\u{00cb}"),
    ("euml", "\u{00eb}"),
    ("euml;", "\u{00eb}"),
    ("euro;", "\u{20ac}"),
    ("excl;", "\u{0021}"),
    ("Exists;", "\u{2203}"),
    ("exist;", "\u{2203}"),
    ("expectation;", "\u{2130}"),
    ("exponentiale;", "\u{2147}"),
    ("ExponentialE;", "\u{2147}"),
    ("fallingdotseq;", "\u{2252}"),
    ("Fcy;", "\u{0424}"),
    ("fcy;", "\u{0444}"),
    ("female;", "\u{2640}"),
    ("ffilig;", "\u{fb03}"),
    ("fflig;", "\u{fb00}"),
    ("ffllig;", "\u{fb04}"),
    ("Ffr;", "\u{1d509}"),
    ("ffr;", "\u{1d523}"),
    ("filig;", "\u{fb01}"),
    ("FilledSmallSquare;", "\u{25fc}"),
    ("FilledVerySmallSquare;", "\u{25aa}"),
    ("fjlig;", "\u{0066}\u{006a}"),
    ("flat;", "\u{266d}"),
    ("fllig;", "\u{fb02}"),
    ("fltns;", "\u{25b1}"),
    ("fnof;", "\u{0192}"),
    ("Fopf;", "\u{1d53d}"),
    ("fopf;", "\u{1d557}"),
    ("forall;", "\u{2200}"),
    ("ForAll;", "\u{2200}"),
    ("fork;", "\u{22d4}"),
    ("forkv;", "\u{2ad9}"),
    ("Fouriertrf;", "\u{2131}"),
    ("fpartint;", "\u{2a0d}"),
    ("frac12", "\u{00bd}"),
    ("frac12;", "\u{00bd}"),
    ("frac13;", "\u{2153}"),
    ("frac14", "\u{00bc}"),
    ("frac14;", "\u{00bc}"),
    ("frac15;", "\u{2155}"),
    ("frac16;", "\u{2159}"),
    ("frac18;", "\u{215b}"),
    ("frac23;", "\u{2154}"),
    ("frac25;", "\u{2156}"),
    ("frac34", "\u{00be}"),
    ("frac34;", "\u{00be}"),
    ("frac35;", "\u{2157}"),
    ("frac38;", "\u{215c}"),
    ("frac45;", "\u{2158}"),
    ("frac56;", "\u{215a}"),
    ("frac58;", "\u{215d}"),
    ("frac78;", "\u{215e}"),
    ("frasl;", "\u{2044}"),
    ("frown;", "\u{2322}"),
    ("fscr;", "\u{1d4bb}"),
    ("Fscr;", "\u{2131}"),
    ("gacute;", "\u{01f5}"),
    ("Gammad;", "\u{03dc}"),
    ("gammad;", "\u{03dd}"),
    ("Gamma;", "\u{0393}"),
    ("gamma;", "\u{03b3}"),
    ("gap;", "\u{2a86}"),
    ("Gbreve;", "\u{011e}"),
    ("gbreve;", "\u{011f}"),
    ("Gcedil;", "\u{0122}"),
    ("Gcirc;", "\u{011c}"),
    ("gcirc;", "\u{011d}"),
    ("Gcy;", "\u{0413}"),
    ("gcy;", "\u{0433}"),
    ("Gdot;", "\u{0120}"),
    ("gdot;", "\u{0121}"),
    ("gel;", "\u{22db}"),
    ("gEl;", "\u{2a8c}"),
    ("geqq;", "\u{2267}"),
    ("geqslant;", "\u{2a7e}"),
    ("geq;", "\u{2265}"),
    ("gescc;", "\u{2aa9}"),
    ("gesdotol;", "\u{2a84}"),
    ("gesdoto;", "\u{2a82}"),
    ("gesdot;", "\u{2a80}"),
    ("gesles;", "\u{2a94}"),
    ("gesl;", "\u{22db}\u{fe00}"),
    ("ges;", "\u{2a7e}"),
    ("ge;", "\u{2265}"),
    ("gE;", "\u{2267}"),
    ("Gfr;", "\u{1d50a}"),
    ("gfr;", "\u{1d524}"),
    ("ggg;", "\u{22d9}"),
    ("gg;", "\u{226b}"),
    ("Gg;", "\u{22d9}"),
    ("gimel;", "\u{2137}"),
    ("GJcy;", "\u{0403}"),
    ("gjcy;", "\u{0453}"),
    ("gla;", "\u{2aa5}"),
    ("glE;", "\u{2a92}"),
    ("glj;", "\u{2aa4}"),
    ("gl;", "\u{2277}"),
    ("gnapprox;", "\u{2a8a}"),
    ("gnap;", "\u{2a8a}"),
    ("gneqq;", "\u{2269}"),
    ("gneq;", "\u{2a88}"),
    ("gnE;", "\u{2269}"),
    ("gne;", "\u{2a88}"),
    ("gnsim;", "\u{22e7}"),
    ("Gopf;", "\u{1d53e}"),
    ("gopf;", "\u{1d558}"),
    ("grave;", "\u{0060}"),
    ("GreaterEqualLess;", "\u{22db}"),
    ("GreaterEqual;", "\u{2265}"),
    ("GreaterFullEqual;", "\u{2267}"),
    ("GreaterGreater;", "\u{2aa2}"),
    ("GreaterLess;", "\u{2277}"),
    ("GreaterSlantEqual;", "\u{2a7e}"),
    ("GreaterTilde;", "\u{2273}"),
    ("Gscr;", "\u{1d4a2}"),
    ("gscr;", "\u{210a}"),
    ("gsime;", "\u{2a8e}"),
    ("gsiml;", "\u{2a90}"),
    ("gsim;", "\u{2273}"),
    ("gtcc;", "\u{2aa7}"),
    ("gtcir;", "\u{2a7a}"),
    ("gtdot;", "\u{22d7}"),
    ("gtlPar;", "\u{2995}"),
    ("gtquest;", "\u{2a7c}"),
    ("gtrapprox;", "\u{2a86}"),
    ("gtrarr;", "\u{2978}"),
    ("gtrdot;", "\u{22d7}"),
    ("gtreqless;", "\u{22db}"),
    ("gtreqqless;", "\u{2a8c}"),
    ("gtrless;", "\u{2277}"),
    ("gtrsim;", "\u{2273}"),
    ("gt", "\u{003e}"),
    ("gt;", "\u{003e}"),
    ("GT", "\u{003e}"),
    ("GT;", "\u{003e}"),
    ("Gt;", "\u{226b}"),
    ("gvertneqq;", "\u{2269}\u{fe00}"),
    ("gvnE;", "\u{2269}\u{fe00}"),
    ("Hacek;", "\u{02c7}"),
    ("hairsp;", "\u{200a}"),
    ("half;", "\u{00bd}"),
    ("hamilt;", "\u{210b}"),
    ("HARDcy;", "\u{042a}"),
    ("hardcy;", "\u{044a}"),
    ("harrcir;", "\u{2948}"),
    ("harr;", "\u{2194}"),
    ("hArr;", "\u{21d4}"),
    ("harrw;", "\u{21ad}"),
    ("Hat;", "\u{005e}"),
    ("hbar;", "\u{210f}"),
    ("Hcirc;", "\u{0124}"),
    ("hcirc;", "\u{0125}"),
    ("hearts;", "\u{2665}"),
    ("heartsuit;", "\u{2665}"),
    ("hellip;", "\u{2026}"),
    ("hercon;", "\u{22b9}"),
    ("hfr;", "\u{1d525}"),
    ("Hfr;", "\u{210c}"),
    ("HilbertSpace;", "\u{210b}"),
    ("hksearow;", "\u{2925}"),
    ("hkswarow;", "\u{2926}"),
    ("hoarr;", "\u{21ff}"),
    ("homtht;", "\u{223b}"),
    ("hookleftarrow;", "\u{21a9}"),
    ("hookrightarrow;", "\u{21aa}"),
    ("hopf;", "\u{1d559}"),
    ("Hopf;", "\u{210d}"),
    ("horbar;", "\u{2015}"),
    ("HorizontalLine;", "\u{2500}"),
    ("hscr;", "\u{1d4bd}"),
    ("Hscr;", "\u{210b}"),
    ("hslash;", "\u{210f}"),
    ("Hstrok;", "\u{0126}"),
    ("hstrok;", "\u{0127}"),
    ("HumpDownHump;", "\u{224e}"),
    ("HumpEqual;", "\u{224f}"),
    ("hybull;", "\u{2043}"),
    ("hyphen;", "\u{2010}"),
    ("Iacute", "\u{00cd}"),
    ("Iacute;", "\u{00cd}"),
    ("iacute", "\u{00ed}"),
    ("iacute;", "\u{00ed}"),
    ("Icirc", "\u{00ce}"),
    ("Icirc;", "\u{00ce}"),
    ("icirc", "\u{00ee}"),
    ("icirc;", "\u{00ee}"),
    ("ic;", "\u{2063}"),
    ("Icy;", "\u{0418}"),
    ("icy;", "\u{0438}"),
    ("Idot;", "\u{0130}"),
    ("IEcy;", "\u{0415}"),
    ("iecy;", "\u{0435}"),
    ("iexcl", "\u{00a1}"),
    ("iexcl;", "\u{00a1}"),
    ("iff;", "\u{21d4}"),
    ("ifr;", "\u{1d526}"),
    ("Ifr;", "\u{2111}"),
    ("Igrave", "\u{00cc}"),
    ("Igrave;", "\u{00cc}"),
    ("igrave", "\u{00ec}"),
    ("igrave;", "\u{00ec}"),
    ("iiiint;", "\u{2a0c}"),
    ("iiint;", "\u{222d}"),
    ("iinfin;", "\u{29dc}"),
    ("iiota;", "\u{2129}"),
    ("ii;", "\u{2148}"),
    ("IJlig;", "\u{0132}"),
    ("ijlig;", "\u{0133}"),
    ("Imacr;", "\u{012a}"),
    ("imacr;", "\u{012b}"),
    ("image;", "\u{2111}"),
    ("ImaginaryI;", "\u{2148}"),
    ("imagline;", "\u{2110}"),
    ("imagpart;", "\u{2111}"),
    ("imath;", "\u{0131}"),
    ("imof;", "\u{22b7}"),
    ("imped;", "\u{01b5}"),
    ("Implies;", "\u{21d2}"),
    ("Im;", "\u{2111}"),
    ("incare;", "\u{2105}"),
    ("infintie;", "\u{29dd}"),
    ("infin;", "\u{221e}"),
    ("inodot;", "\u{0131}"),
    ("intcal;", "\u{22ba}"),
    ("integers;", "\u{2124}"),
    ("Integral;", "\u{222b}"),
    ("intercal;", "\u{22ba}"),
    ("Intersection;", "\u{22c2}"),
    ("intlarhk;", "\u{2a17}"),
    ("intprod;", "\u{2a3c}"),
    ("int;", "\u{222b}"),
    ("Int;", "\u{222c}"),
    ("in;", "\u{2208}"),
    ("InvisibleComma;", "\u{2063}"),
    ("InvisibleTimes;", "\u{2062}"),
    ("IOcy;", "\u{0401}"),
    ("iocy;", "\u{0451}"),
    ("Iogon;", "\u{012e}"),
    ("iogon;", "\u{012f}"),
    ("Iopf;", "\u{1d540}"),
    ("iopf;", "\u{1d55a}"),
    ("Iota;", "\u{0399}"),
    ("iota;", "\u{03b9}"),
    ("iprod;", "\u{2a3c}"),
    ("iquest", "\u{00bf}"),
    ("iquest;", "\u{00bf}"),
    ("iscr;", "\u{1d4be}"),
    ("Iscr;", "\u{2110}"),
    ("isindot;", "\u{22f5}"),
    ("isinE;", "\u{22f9}"),
    ("isins;", "\u{22f4}"),
    ("isinsv;", "\u{22f3}"),
    ("isin;", "\u{2208}"),
    ("isinv;", "\u{2208}"),
    ("Itilde;", "\u{0128}"),
    ("itilde;", "\u{0129}"),
    ("it;", "\u{2062}"),
    ("Iukcy;", "\u{0406}"),
    ("iukcy;", "\u{0456}"),
    ("Iuml", "\u{00cf}"),
    ("Iuml;", "\u{00cf}"),
    ("iuml", "\u{00ef}"),
    ("iuml;", "\u{00ef}"),
    ("Jcirc;", "\u{0134}"),
    ("jcirc;", "\u{0135}"),
    ("Jcy;", "\u{0419}"),
    ("jcy;", "\u{0439}"),
    ("Jfr;", "\u{1d50d}"),
    ("jfr;", "\u{1d527}"),
    ("jmath;", "\u{0237}"),
    ("Jopf;", "\u{1d541}"),
    ("jopf;", "\u{1d55b}"),
    ("Jscr;", "\u{1d4a5}"),
    ("jscr;", "\u{1d4bf}"),
    ("Jsercy;", "\u{0408}"),
    ("jsercy;", "\u{0458}"),
    ("Jukcy;", "\u{0404}"),
    ("jukcy;", "\u{0454}"),
    ("Kappa;", "\u{039a}"),
    ("kappa;", "\u{03ba}"),
    ("kappav;", "\u{03f0}"),
    ("Kcedil;", "\u{0136}"),
    ("kcedil;", "\u{0137}"),
    ("Kcy;", "\u{041a}"),
    ("kcy;", "\u{043a}"),
    ("Kfr;", "\u{1d50e}"),
    ("kfr;", "\u{1d528}"),
    ("kgreen;", "\u{0138}"),
    ("KHcy;", "\u{0425}"),
    ("khcy;", "\u{0445}"),
    ("KJcy;", "\u{040c}"),
    ("kjcy;", "\u{045c}"),
    ("Kopf;", "\u{1d542}"),
    ("kopf;", "\u{1d55c}"),
    ("Kscr;", "\u{1d4a6}"),
    ("kscr;", "\u{1d4c0}"),
    ("lAarr;", "\u{21da}"),
    ("Lacute;", "\u{0139}"),
    ("lacute;", "\u{013a}"),
    ("laemptyv;", "\u{29b4}"),
    ("lagran;", "\u{2112}"),
    ("Lambda;", "\u{039b}"),
    ("lambda;", "\u{03bb}"),
    ("langd;", "\u{2991}"),
    ("langle;", "\u{27e8}"),
    ("lang;", "\u{27e8}"),
    ("Lang;", "\u{27ea}"),
    ("Laplacetrf;", "\u{2112}"),
    ("lap;", "\u{2a85}"),
    ("laquo", "\u{00ab}"),
    ("laquo;", "\u{00ab}"),
    ("larrbfs;", "\u{291f}"),
    ("larrb;", "\u{21e4}"),
    ("larrfs;", "\u{291d}"),
    ("larrhk;", "\u{21a9}"),
    ("larrlp;", "\u{21ab}"),
    ("larrpl;", "\u{2939}"),
    ("larrsim;", "\u{2973}"),
    ("larrtl;", "\u{21a2}"),
    ("larr;", "\u{2190}"),
    ("Larr;", "\u{219e}"),
    ("lArr;", "\u{21d0}"),
    ("latail;", "\u{2919}"),
    ("lAtail;", "\u{291b}"),
    ("lates;", "\u{2aad}\u{fe00}"),
    ("late;", "\u{2aad}"),
    ("lat;", "\u{2aab}"),
    ("lbarr;", "\u{290c}"),
    ("lBarr;", "\u{290e}"),
    ("lbbrk;", "\u{2772}"),
    ("lbrace;", "\u{007b}"),
    ("lbrack;", "\u{005b}"),
    ("lbrke;", "\u{298b}"),
    ("lbrksld;", "\u{298f}"),
    ("lbrkslu;", "\u{298d}"),
    ("Lcaron;", "\u{013d}"),
    ("lcaron;", "\u{013e}"),
    ("Lcedil;", "\u{013b}"),
    ("lcedil;", "\u{013c}"),
    ("lceil;", "\u{2308}"),
    ("lcub;", "\u{007b}"),
    ("Lcy;", "\u{041b}"),
    ("lcy;", "\u{043b}"),
    ("ldca;", "\u{2936}"),
    ("ldquor;", "\u{201e}"),
    ("ldquo;", "\u{201c}"),
    ("ldrdhar;", "\u{2967}"),
    ("ldrushar;", "\u{294b}"),
    ("ldsh;", "\u{21b2}"),
    ("LeftAngleBracket;", "\u{27e8}"),
    ("LeftArrowBar;", "\u{21e4}"),
    ("LeftArrowRightArrow;", "\u{21c6}"),
    ("leftarrowtail;", "\u{21a2}"),
    ("leftarrow;", "\u{2190}"),
    ("LeftArrow;", "\u{2190}"),
    ("Leftarrow;", "\u{21d0}"),
    ("LeftCeiling;", "\u{2308}"),
    ("LeftDoubleBracket;", "\u{27e6}"),
    ("LeftDownTeeVector;", "\u{2961}"),
    ("LeftDownVectorBar;", "\u{2959}"),
    ("LeftDownVector;", "\u{21c3}"),
    ("LeftFloor;", "\u{230a}"),
    ("leftharpoondown;", "\u{21bd}"),
    ("leftharpoonup;", "\u{21bc}"),
    ("leftleftarrows;", "\u{21c7}"),
    ("leftrightarrows;", "\u{21c6}"),
    ("leftrightarrow;", "\u{2194}"),
    ("LeftRightArrow;", "\u{2194}"),
    ("Leftrightarrow;", "\u{21d4}"),
    ("leftrightharpoons;", "\u{21cb}"),
    ("leftrightsquigarrow;", "\u{21ad}"),
    ("LeftRightVector;", "\u{294e}"),
    ("LeftTeeArrow;", "\u{21a4}"),
    ("LeftTee;", "\u{22a3}"),
    ("LeftTeeVector;", "\u{295a}"),
    ("leftthreetimes;", "\u{22cb}"),
    ("LeftTriangleBar;", "\u{29cf}"),
    ("LeftTriangleEqual;", "\u{22b4}"),
    ("LeftTriangle;", "\u{22b2}"),
    ("LeftUpDownVector;", "\u{2951}"),
    ("LeftUpTeeVector;", "\u{2960}"),
    ("LeftUpVectorBar;", "\u{2958}"),
    ("LeftUpVector;", "\u{21bf}"),
    ("LeftVectorBar;", "\u{2952}"),
    ("LeftVector;", "\u{21bc}"),
    ("leg;", "\u{22da}"),
    ("lEg;", "\u{2a8b}"),
    ("leqq;", "\u{2266}"),
    ("leqslant;", "\u{2a7d}"),
    ("leq;", "\u{2264}"),
    ("lescc;", "\u{2aa8}"),
    ("lesdotor;", "\u{2a83}"),
    ("lesdoto;", "\u{2a81}"),
    ("lesdot;", "\u{2a7f}"),
    ("lesges;", "\u{2a93}"),
    ("lesg;", "\u{22da}\u{fe00}"),
    ("lessapprox;", "\u{2a85}"),
    ("lessdot;", "\u{22d6}"),
    ("lesseqgtr;", "\u{22da}"),
    ("lesseqqgtr;", "\u{2a8b}"),
    ("LessEqualGreater;", "\u{22da}"),
    ("LessFullEqual;", "\u{2266}"),
    ("LessGreater;", "\u{2276}"),
    ("lessgtr;", "\u{2276}"),
    ("LessLess;", "\u{2aa1}"),
    ("lesssim;", "\u{2272}"),
    ("LessSlantEqual;", "\u{2a7d}"),
    ("LessTilde;", "\u{2272}"),
    ("les;", "\u{2a7d}"),
    ("le;", "\u{2264}"),
    ("lE;", "\u{2266}"),
    ("lfisht;", "\u{297c}"),
    ("lfloor;", "\u{230a}"),
    ("Lfr;", "\u{1d50f}"),
    ("lfr;", "\u{1d529}"),
    ("lgE;", "\u{2a91}"),
    ("lg;", "\u{2276}"),
    ("lhard;", "\u{21bd}"),
    ("lHar;", "\u{2962}"),
    ("lharul;", "\u{296a}"),
    ("lharu;", "\u{21bc}"),
    ("lhblk;", "\u{2584}"),
    ("LJcy;", "\u{0409}"),
    ("ljcy;", "\u{0459}"),
    ("llarr;", "\u{21c7}"),
    ("llcorner;", "\u{231e}"),
    ("Lleftarrow;", "\u{21da}"),
    ("llhard;", "\u{296b}"),
    ("lltri;", "\u{25fa}"),
    ("ll;", "\u{226a}"),
    ("Ll;", "\u{22d8}"),
    ("Lmidot;", "\u{013f}"),
    ("lmidot;", "\u{0140}"),
    ("lmoustache;", "\u{23b0}"),
    ("lmoust;", "\u{23b0}"),
    ("lnapprox;", "\u{2a89}"),
    ("lnap;", "\u{2a89}"),
    ("lneqq;", "\u{2268}"),
    ("lneq;", "\u{2a87}"),
    ("lnE;", "\u{2268}"),
    ("lne;", "\u{2a87}"),
    ("lnsim;", "\u{22e6}"),
    ("loang;", "\u{27ec}"),
    ("loarr;", "\u{21fd}"),
    ("lobrk;", "\u{27e6}"),
    ("longleftarrow;", "\u{27f5}"),
    ("LongLeftArrow;", "\u{27f5}"),
    ("Longleftarrow;", "\u{27f8}"),
    ("longleftrightarrow;", "\u{27f7}"),
    ("LongLeftRightArrow;", "\u{27f7}"),
    ("Longleftrightarrow;", "\u{27fa}"),
    ("longmapsto;", "\u{27fc}"),
    ("longrightarrow;", "\u{27f6}"),
    ("LongRightArrow;", "\u{27f6}"),
    ("Longrightarrow;", "\u{27f9}"),
    ("looparrowleft;", "\u{21ab}"),
    ("looparrowright;", "\u{21ac}"),
    ("lopar;", "\u{2985}"),
    ("Lopf;", "\u{1d543}"),
    ("lopf;", "\u{1d55d}"),
    ("loplus;", "\u{2a2d}"),
    ("lotimes;", "\u{2a34}"),
    ("lowast;", "\u{2217}"),
    ("lowbar;", "\u{005f}"),
    ("LowerLeftArrow;", "\u{2199}"),
    ("LowerRightArrow;", "\u{2198}"),
    ("lozenge;", "\u{25ca}"),
    ("lozf;", "\u{29eb}"),
    ("loz;", "\u{25ca}"),
    ("lparlt;", "\u{2993}"),
    ("lpar;", "\u{0028}"),
    ("lrarr;", "\u{21c6}"),
    ("lrcorner;", "\u{231f}"),
    ("lrhard;", "\u{296d}"),
    ("lrhar;", "\u{21cb}"),
    ("lrm;", "\u{200e}"),
    ("lrtri;", "\u{22bf}"),
    ("lsaquo;", "\u{2039}"),
    ("lscr;", "\u{1d4c1}"),
    ("Lscr;", "\u{2112}"),
    ("lsh;", "\u{21b0}"),
    ("Lsh;", "\u{21b0}"),
    ("lsime;", "\u{2a8d}"),
    ("lsimg;", "\u{2a8f}"),
    ("lsim;", "\u{2272}"),
    ("lsqb;", "\u{005b}"),
    ("lsquor;", "\u{201a}"),
    ("lsquo;", "\u{2018}"),
    ("Lstrok;", "\u{0141}"),
    ("lstrok;", "\u{0142}"),
    ("ltcc;", "\u{2aa6}"),
    ("ltcir;", "\u{2a79}"),
    ("ltdot;", "\u{22d6}"),
    ("lthree;", "\u{22cb}"),
    ("ltimes;", "\u{22c9}"),
    ("ltlarr;", "\u{2976}"),
    ("ltquest;", "\u{2a7b}"),
    ("ltrie;", "\u{22b4}"),
    ("ltrif;", "\u{25c2}"),
    ("ltri;", "\u{25c3}"),
    ("ltrPar;", "\u{2996}"),
    ("lt", "\u{003c}"),
    ("lt;", "\u{003c}"),
    ("LT", "\u{003c}"),
    ("LT;", "\u{003c}"),
    ("Lt;", "\u{226a}"),
    ("lurdshar;", "\u{294a}"),
    ("luruhar;", "\u{2966}"),
    ("lvertneqq;", "\u{2268}\u{fe00}"),
    ("lvnE;", "\u{2268}\u{fe00}"),
    ("macr", "\u{00af}"),
    ("macr;", "\u{00af}"),
    ("male;", "\u{2642}"),
    ("maltese;", "\u{2720}"),
    ("malt;", "\u{2720}"),
    ("mapstodown;", "\u{21a7}"),
    ("mapstoleft;", "\u{21a4}"),
    ("mapsto;", "\u{21a6}"),
    ("mapstoup;", "\u{21a5}"),
    ("map;", "\u{21a6}"),
    ("Map;", "\u{2905}"),
    ("marker;", "\u{25ae}"),
    ("mcomma;", "\u{2a29}"),
    ("Mcy;", "\u{041c}"),
    ("mcy;", "\u{043c}"),
    ("mdash;", "\u{2014}"),
    ("mDDot;", "\u{223a}"),
    ("measuredangle;", "\u{2221}"),
    ("MediumSpace;", "\u{205f}"),
    ("Mellintrf;", "\u{2133}"),
    ("Mfr;", "\u{1d510}"),
    ("mfr;", "\u{1d52a}"),
    ("mho;", "\u{2127}"),
    ("micro", "\u{00b5}"),
    ("micro;", "\u{00b5}"),
    ("midast;", "\u{002a}"),
    ("midcir;", "\u{2af0}"),
    ("middot", "\u{00b7}"),
    ("middot;", "\u{00b7}"),
    ("mid;", "\u{2223}"),
    ("minusb;", "\u{229f}"),
    ("minusd;", "\u{2238}"),
    ("minusdu;", "\u{2a2a}"),
    ("MinusPlus;", "\u{2213}"),
    ("minus;", "\u{2212}"),
    ("mlcp;", "\u{2adb}"),
    ("mldr;", "\u{2026}"),
    ("mnplus;", "\u{2213}"),
    ("models;", "\u{22a7}"),
    ("Mopf;", "\u{1d544}"),
    ("mopf;", "\u{1d55e}"),
    ("mp;", "\u{2213}"),
    ("mscr;", "\u{1d4c2}"),
    ("Mscr;", "\u{2133}"),
    ("mstpos;", "\u{223e}"),
    ("multimap;", "\u{22b8}"),
    ("mumap;", "\u{22b8}"),
    ("Mu;", "\u{039c}"),
    ("mu;", "\u{03bc}"),
    ("nabla;", "\u{2207}"),
    ("Nacute;", "\u{0143}"),
    ("nacute;", "\u{0144}"),
    ("nang;", "\u{2220}\u{20d2}"),
    ("napE;", "\u{2a70}\u{0338}"),
    ("napid;", "\u{224b}\u{0338}"),
    ("napos;", "\u{0149}"),
    ("napprox;", "\u{2249}"),
    ("nap;", "\u{2249}"),
    ("naturals;", "\u{2115}"),
    ("natural;", "\u{266e}"),
    ("natur;", "\u{266e}"),
    ("nbsp", "\u{00a0}"),
    ("nbsp;", "\u{00a0}"),
    ("nbumpe;", "\u{224f}\u{0338}"),
    ("nbump;", "\u{224e}\u{0338}"),
    ("ncap;", "\u{2a43}"),
    ("Ncaron;", "\u{0147}"),
    ("ncaron;", "\u{0148}"),
    ("Ncedil;", "\u{0145}"),
    ("ncedil;", "\u{0146}"),
    ("ncongdot;", "\u{2a6d}\u{0338}"),
    ("ncong;", "\u{2247}"),
    ("ncup;", "\u{2a42}"),
    ("Ncy;", "\u{041d}"),
    ("ncy;", "\u{043d}"),
    ("ndash;", "\u{2013}"),
    ("nearhk;", "\u{2924}"),
    ("nearrow;", "\u{2197}"),
    ("nearr;", "\u{2197}"),
    ("neArr;", "\u{21d7}"),
    ("nedot;", "\u{2250}\u{0338}"),
    ("NegativeMediumSpace;", "\u{200b}"),
    ("NegativeThickSpace;", "\u{200b}"),
    ("NegativeThinSpace;", "\u{200b}"),
    ("NegativeVeryThinSpace;", "\u{200b}"),
    ("nequiv;", "\u{2262}"),
    ("nesear;", "\u{2928}"),
    ("nesim;", "\u{2242}\u{0338}"),
    ("NestedGreaterGreater;", "\u{226b}"),
    ("NestedLessLess;", "\u{226a}"),
    ("ne;", "\u{2260}"),
    ("NewLine;", "\u{000a}"),
    ("nexists;", "\u{2204}"),
    ("nexist;", "\u{2204}"),
    ("Nfr;", "\u{1d511}"),
    ("nfr;", "\u{1d52b}"),
    ("ngeqq;", "\u{2267}\u{0338}"),
    ("ngeqslant;", "\u{2a7e}\u{0338}"),
    ("ngeq;", "\u{2271}"),
    ("nges;", "\u{2a7e}\u{0338}"),
    ("ngE;", "\u{2267}\u{0338}"),
    ("nge;", "\u{2271}"),
    ("nGg;", "\u{22d9}\u{0338}"),
    ("ngsim;", "\u{2275}"),
    ("ngtr;", "\u{226f}"),
    ("nGt;", "\u{226b}\u{20d2}"),
    ("ngt;", "\u{226f}"),
    ("nGtv;", "\u{226b}\u{0338}"),
    ("nharr;", "\u{21ae}"),
    ("nhArr;", "\u{21ce}"),
    ("nhpar;", "\u{2af2}"),
    ("nisd;", "\u{22fa}"),
    ("nis;", "\u{22fc}"),
    ("ni;", "\u{220b}"),
    ("niv;", "\u{220b}"),
    ("NJcy;", "\u{040a}"),
    ("njcy;", "\u{045a}"),
    ("nlarr;", "\u{219a}"),
    ("nlArr;", "\u{21cd}"),
    ("nldr;", "\u{2025}"),
    ("nleftarrow;", "\u{219a}"),
    ("nLeftarrow;", "\u{21cd}"),
    ("nleftrightarrow;", "\u{21ae}"),
    ("nLeftrightarrow;", "\u{21ce}"),
    ("nleqq;", "\u{2266}\u{0338}"),
    ("nleqslant;", "\u{2a7d}\u{0338}"),
    ("nleq;", "\u{2270}"),
    ("nless;", "\u{226e}"),
    ("nles;", "\u{2a7d}\u{0338}"),
    ("nlE;", "\u{2266}\u{0338}"),
    ("nle;", "\u{2270}"),
    ("nLl;", "\u{22d8}\u{0338}"),
    ("nlsim;", "\u{2274}"),
    ("nltrie;", "\u{22ec}"),
    ("nltri;", "\u{22ea}"),
    ("nLt;", "\u{226a}\u{20d2}"),
    ("nlt;", "\u{226e}"),
    ("nLtv;", "\u{226a}\u{0338}"),
    ("nmid;", "\u{2224}"),
    ("NoBreak;", "\u{2060}"),
    ("NonBreakingSpace;", "\u{00a0}"),
    ("nopf;", "\u{1d55f}"),
    ("Nopf;", "\u{2115}"),
    ("NotCongruent;", "\u{2262}"),
    ("NotCupCap;", "\u{226d}"),
    ("NotDoubleVerticalBar;", "\u{2226}"),
    ("NotElement;", "\u{2209}"),
    ("NotEqualTilde;", "\u{2242}\u{0338}"),
    ("NotEqual;", "\u{2260}"),
    ("NotExists;", "\u{2204}"),
    ("NotGreaterEqual;", "\u{2271}"),
    ("NotGreaterFullEqual;", "\u{2267}\u{0338}"),
    ("NotGreaterGreater;", "\u{226b}\u{0338}"),
    ("NotGreaterLess;", "\u{2279}"),
    ("NotGreaterSlantEqual;", "\u{2a7e}\u{0338}"),
    ("NotGreaterTilde;", "\u{2275}"),
    ("NotGreater;", "\u{226f}"),
    ("NotHumpDownHump;", "\u{224e}\u{0338}"),
    ("NotHumpEqual;", "\u{224f}\u{0338}"),
    ("notindot;", "\u{22f5}\u{0338}"),
    ("notinE;", "\u{22f9}\u{0338}"),
    ("notin;", "\u{2209}"),
    ("notinva;", "\u{2209}"),
    ("notinvb;", "\u{22f7}"),
    ("notinvc;", "\u{22f6}"),
    ("NotLeftTriangleBar;", "\u{29cf}\u{0338}"),
    ("NotLeftTriangleEqual;", "\u{22ec}"),
    ("NotLeftTriangle;", "\u{22ea}"),
    ("NotLessEqual;", "\u{2270}"),
    ("NotLessGreater;", "\u{2278}"),
    ("NotLessLess;", "\u{226a}\u{0338}"),
    ("NotLessSlantEqual;", "\u{2a7d}\u{0338}"),
    ("NotLessTilde;", "\u{2274}"),
    ("NotLess;", "\u{226e}"),
    ("NotNestedGreaterGreater;", "\u{2aa2}\u{0338}"),
    ("NotNestedLessLess;", "\u{2aa1}\u{0338}"),
    ("notni;", "\u{220c}"),
    ("notniva;", "\u{220c}"),
    ("notnivb;", "\u{22fe}"),
    ("notnivc;", "\u{22fd}"),
    ("NotPrecedesEqual;", "\u{2aaf}\u{0338}"),
    ("NotPrecedesSlantEqual;", "\u{22e0}"),
    ("NotPrecedes;", "\u{2280}"),
    ("NotReverseElement;", "\u{220c}"),
    ("NotRightTriangleBar;", "\u{29d0}\u{0338}"),
    ("NotRightTriangleEqual;", "\u{22ed}"),
    ("NotRightTriangle;", "\u{22eb}"),
    ("NotSquareSubsetEqual;", "\u{22e2}"),
    ("NotSquareSubset;", "\u{228f}\u{0338}"),
    ("NotSquareSupersetEqual;", "\u{22e3}"),
    ("NotSquareSuperset;", "\u{2290}\u{0338}"),
    ("NotSubsetEqual;", "\u{2288}"),
    ("NotSubset;", "\u{2282}\u{20d2}"),
    ("NotSucceedsEqual;", "\u{2ab0}\u{0338}"),
    ("NotSucceedsSlantEqual;", "\u{22e1}"),
    ("NotSucceedsTilde;", "\u{227f}\u{0338}"),
    ("NotSucceeds;", "\u{2281}"),
    ("NotSupersetEqual;", "\u{2289}"),
    ("NotSuperset;", "\u{2283}\u{20d2}"),
    ("NotTildeEqual;", "\u{2244}"),
    ("NotTildeFullEqual;", "\u{2247}"),
    ("NotTildeTilde;", "\u{2249}"),
    ("NotTilde;", "\u{2241}"),
    ("not", "\u{00ac}"),
    ("not;", "\u{00ac}"),
    ("Not;", "\u{2aec}"),
    ("NotVerticalBar;", "\u{2224}"),
    ("nparallel;", "\u{2226}"),
    ("nparsl;", "\u{2afd}\u{20e5}"),
    ("npart;", "\u{2202}\u{0338}"),
    ("npar;", "\u{2226}"),
    ("npolint;", "\u{2a14}"),
    ("nprcue;", "\u{22e0}"),
    ("npreceq;", "\u{2aaf}\u{0338}"),
    ("nprec;", "\u{2280}"),
    ("npre;", "\u{2aaf}\u{0338}"),
    ("npr;", "\u{2280}"),
    ("nrarrc;", "\u{2933}\u{0338}"),
    ("nrarr;", "\u{219b}"),
    ("nrArr;", "\u{21cf}"),
    ("nrarrw;", "\u{219d}\u{0338}"),
    ("nrightarrow;", "\u{219b}"),
    ("nRightarrow;", "\u{21cf}"),
    ("nrtrie;", "\u{22ed}"),
    ("nrtri;", "\u{22eb}"),
    ("nsccue;", "\u{22e1}"),
    ("nsce;", "\u{2ab0}\u{0338}"),
    ("Nscr;", "\u{1d4a9}"),
    ("nscr;", "\u{1d4c3}"),
    ("nsc;", "\u{2281}"),
    ("nshortmid;", "\u{2224}"),
    ("nshortparallel;", "\u{2226}"),
    ("nsimeq;", "\u{2244}"),
    ("nsime;", "\u{2244}"),
    ("nsim;", "\u{2241}"),
    ("nsmid;", "\u{2224}"),
    ("nspar;", "\u{2226}"),
    ("nsqsube;", "\u{22e2}"),
    ("nsqsupe;", "\u{22e3}"),
    ("nsube;", "\u{2288}"),
    ("nsubE;", "\u{2ac5}\u{0338}"),
    ("nsubseteqq;", "\u{2ac5}\u{0338}"),
    ("nsubseteq;", "\u{2288}"),
    ("nsubset;", "\u{2282}\u{20d2}"),
    ("nsub;", "\u{2284}"),
    ("nsucceq;", "\u{2ab0}\u{0338}"),
    ("nsucc;", "\u{2281}"),
    ("nsupe;", "\u{2289}"),
    ("nsupE;", "\u{2ac6}\u{0338}"),
    ("nsupseteqq;", "\u{2ac6}\u{0338}"),
    ("nsupseteq;", "\u{2289}"),
    ("nsupset;", "\u{2283}\u{20d2}"),
    ("nsup;", "\u{2285}"),
    ("ntgl;", "\u{2279}"),
    ("Ntilde", "\u{00d1}"),
    ("Ntilde;", "\u{00d1}"),
    ("ntilde", "\u{00f1}"),
    ("ntilde;", "\u{00f1}"),
    ("ntlg;", "\u{2278}"),
    ("ntrianglelefteq;", "\u{22ec}"),
    ("ntriangleleft;", "\u{22ea}"),
    ("ntrianglerighteq;", "\u{22ed}"),
    ("ntriangleright;", "\u{22eb}"),
    ("numero;", "\u{2116}"),
    ("numsp;", "\u{2007}"),
    ("num;", "\u{0023}"),
    ("Nu;", "\u{039d}"),
    ("nu;", "\u{03bd}"),
    ("nvap;", "\u{224d}\u{20d2}"),
    ("nvdash;", "\u{22ac}"),
    ("nvDash;", "\u{22ad}"),
    ("nVdash;", "\u{22ae}"),
    ("nVDash;", "\u{22af}"),
    ("nvge;", "\u{2265}\u{20d2}"),
    ("nvgt;", "\u{003e}\u{20d2}"),
    ("nvHarr;", "\u{2904}"),
    ("nvinfin;", "\u{29de}"),
    ("nvlArr;", "\u{2902}"),
    ("nvle;", "\u{2264}\u{20d2}"),
    ("nvltrie;", "\u{22b4}\u{20d2}"),
    ("nvlt;", "\u{003c}\u{20d2}"),
    ("nvrArr;", "\u{2903}"),
    ("nvrtrie;", "\u{22b5}\u{20d2}"),
    ("nvsim;", "\u{223c}\u{20d2}"),
    ("nwarhk;", "\u{2923}"),
    ("nwarrow;", "\u{2196}"),
    ("nwarr;", "\u{2196}"),
    ("nwArr;", "\u{21d6}"),
    ("nwnear;", "\u{2927}"),
    ("Oacute", "\u{00d3}"),
    ("Oacute;", "\u{00d3}"),
    ("oacute", "\u{00f3}"),
    ("oacute;", "\u{00f3}"),
    ("oast;", "\u{229b}"),
    ("Ocirc", "\u{00d4}"),
    ("Ocirc;", "\u{00d4}"),
    ("ocirc", "\u{00f4}"),
    ("ocirc;", "\u{00f4}"),
    ("ocir;", "\u{229a}"),
    ("Ocy;", "\u{041e}"),
    ("ocy;", "\u{043e}"),
    ("odash;", "\u{229d}"),
    ("Odblac;", "\u{0150}"),
    ("odblac;", "\u{0151}"),
    ("odiv;", "\u{2a38}"),
    ("odot;", "\u{2299}"),
    ("odsold;", "\u{29bc}"),
    ("OElig;", "\u{0152}"),
    ("oelig;", "\u{0153}"),
    ("ofcir;", "\u{29bf}"),
    ("Ofr;", "\u{1d512}"),
    ("ofr;", "\u{1d52c}"),
    ("ogon;", "\u{02db}"),
    ("Ograve", "\u{00d2}"),
    ("Ograve;", "\u{00d2}"),
    ("ograve", "\u{00f2}"),
    ("ograve;", "\u{00f2}"),
    ("ogt;", "\u{29c1}"),
    ("ohbar;", "\u{29b5}"),
    ("ohm;", "\u{03a9}"),
    ("oint;", "\u{222e}"),
    ("olarr;", "\u{21ba}"),
    ("olcir;", "\u{29be}"),
    ("olcross;", "\u{29bb}"),
    ("oline;", "\u{203e}"),
    ("olt;", "\u{29c0}"),
    ("Omacr;", "\u{014c}"),
    ("omacr;", "\u{014d}"),
    ("Omega;", "\u{03a9}"),
    ("omega;", "\u{03c9}"),
    ("Omicron;", "\u{039f}"),
    ("omicron;", "\u{03bf}"),
    ("omid;", "\u{29b6}"),
    ("ominus;", "\u{2296}"),
    ("Oopf;", "\u{1d546}"),
    ("oopf;", "\u{1d560}"),
    ("opar;", "\u{29b7}"),
    ("OpenCurlyDoubleQuote;", "\u{201c}"),
    ("OpenCurlyQuote;", "\u{2018}"),
    ("operp;", "\u{29b9}"),
    ("oplus;", "\u{2295}"),
    ("orarr;", "\u{21bb}"),
    ("orderof;", "\u{2134}"),
    ("order;", "\u{2134}"),
    ("ordf", "\u{00aa}"),
    ("ordf;", "\u{00aa}"),
    ("ordm", "\u{00ba}"),
    ("ordm;", "\u{00ba}"),
    ("ord;", "\u{2a5d}"),
    ("origof;", "\u{22b6}"),
    ("oror;", "\u{2a56}"),
    ("orslope;", "\u{2a57}"),
    ("or;", "\u{2228}"),
    ("Or;", "\u{2a54}"),
    ("orv;", "\u{2a5b}"),
    ("Oscr;", "\u{1d4aa}"),
    ("oscr;", "\u{2134}"),
    ("Oslash", "\u{00d8}"),
    ("Oslash;", "\u{00d8}"),
    ("oslash", "\u{00f8}"),
    ("oslash;", "\u{00f8}"),
    ("osol;", "\u{2298}"),
    ("oS;", "\u{24c8}"),
    ("Otilde", "\u{00d5}"),
    ("Otilde;", "\u{00d5}"),
    ("otilde", "\u{00f5}"),
    ("otilde;", "\u{00f5}"),
    ("otimesas;", "\u{2a36}"),
    ("otimes;", "\u{2297}"),
    ("Otimes;", "\u{2a37}"),
    ("Ouml", "\u{00d6}"),
    ("Ouml;", "\u{00d6}"),
    ("ouml", "\u{00f6}"),
    ("ouml;", "\u{00f6}"),
    ("ovbar;", "\u{233d}"),
    ("OverBar;", "\u{203e}"),
    ("OverBrace;", "\u{23de}"),
    ("OverBracket;", "\u{23b4}"),
    ("OverParenthesis;", "\u{23dc}"),
    ("parallel;", "\u{2225}"),
    ("para", "\u{00b6}"),
    ("para;", "\u{00b6}"),
    ("parsim;", "\u{2af3}"),
    ("parsl;", "\u{2afd}"),
    ("PartialD;", "\u{2202}"),
    ("part;", "\u{2202}"),
    ("par;", "\u{2225}"),
    ("Pcy;", "\u{041f}"),
    ("pcy;", "\u{043f}"),
    ("percnt;", "\u{0025}"),
    ("period;", "\u{002e}"),
    ("permil;", "\u{2030}"),
    ("perp;", "\u{22a5}"),
    ("pertenk;", "\u{2031}"),
    ("Pfr;", "\u{1d513}"),
    ("pfr;", "\u{1d52d}"),
    ("Phi;", "\u{03a6}"),
    ("phi;", "\u{03c6}"),
    ("phiv;", "\u{03d5}"),
    ("phmmat;", "\u{2133}"),
    ("phone;", "\u{260e}"),
    ("pitchfork;", "\u{22d4}"),
    ("Pi;", "\u{03a0}"),
    ("pi;", "\u{03c0}"),
    ("piv;", "\u{03d6}"),
    ("planckh;", "\u{210e}"),
    ("planck;", "\u{210f}"),
    ("plankv;", "\u{210f}"),
    ("plusacir;", "\u{2a23}"),
    ("plusb;", "\u{229e}"),
    ("pluscir;", "\u{2a22}"),
    ("plusdo;", "\u{2214}"),
    ("plusdu;", "\u{2a25}"),
    ("pluse;", "\u{2a72}"),
    ("PlusMinus;", "\u{00b1}"),
    ("plusmn", "\u{00b1}"),
    ("plusmn;", "\u{00b1}"),
    ("plussim;", "\u{2a26}"),
    ("plustwo;", "\u{2a27}"),
    ("plus;", "\u{002b}"),
    ("pm;", "\u{00b1}"),
    ("Poincareplane;", "\u{210c}"),
    ("pointint;", "\u{2a15}"),
    ("popf;", "\u{1d561}"),
    ("Popf;", "\u{2119}"),
    ("pound", "\u{00a3}"),
    ("pound;", "\u{00a3}"),
    ("prap;", "\u{2ab7}"),
    ("prcue;", "\u{227c}"),
    ("precapprox;", "\u{2ab7}"),
    ("preccurlyeq;", "\u{227c}"),
    ("PrecedesEqual;", "\u{2aaf}"),
    ("PrecedesSlantEqual;", "\u{227c}"),
    ("PrecedesTilde;", "\u{227e}"),
    ("Precedes;", "\u{227a}"),
    ("preceq;", "\u{2aaf}"),
    ("precnapprox;", "\u{2ab9}"),
    ("precneqq;", "\u{2ab5}"),
    ("precnsim;", "\u{22e8}"),
    ("precsim;", "\u{227e}"),
    ("prec;", "\u{227a}"),
    ("pre;", "\u{2aaf}"),
    ("prE;", "\u{2ab3}"),
    ("primes;", "\u{2119}"),
    ("prime;", "\u{2032}"),
    ("Prime;", "\u{2033}"),
    ("prnap;", "\u{2ab9}"),
    ("prnE;", "\u{2ab5}"),
    ("prnsim;", "\u{22e8}"),
    ("prod;", "\u{220f}"),
    ("Product;", "\u{220f}"),
    ("profalar;", "\u{232e}"),
    ("profline;", "\u{2312}"),
    ("profsurf;", "\u{2313}"),
    ("Proportional;", "\u{221d}"),
    ("Proportion;", "\u{2237}"),
    ("propto;", "\u{221d}"),
    ("prop;", "\u{221d}"),
    ("prsim;", "\u{227e}"),
    ("pr;", "\u{227a}"),
    ("Pr;", "\u{2abb}"),
    ("prurel;", "\u{22b0}"),
    ("Pscr;", "\u{1d4ab}"),
    ("pscr;", "\u{1d4c5}"),
    ("Psi;", "\u{03a8}"),
    ("psi;", "\u{03c8}"),
    ("puncsp;", "\u{2008}"),
    ("Qfr;", "\u{1d514}"),
    ("qfr;", "\u{1d52e}"),
    ("qint;", "\u{2a0c}"),
    ("qopf;", "\u{1d562}"),
    ("Qopf;", "\u{211a}"),
    ("qprime;", "\u{2057}"),
    ("Qscr;", "\u{1d4ac}"),
    ("qscr;", "\u{1d4c6}"),
    ("quaternions;", "\u{210d}"),
    ("quatint;", "\u{2a16}"),
    ("questeq;", "\u{225f}"),
    ("quest;", "\u{003f}"),
    ("quot", "\u{0022}"),
    ("quot;", "\u{0022}"),
    ("QUOT", "\u{0022}"),
    ("QUOT;", "\u{0022}"),
    ("rAarr;", "\u{21db}"),
    ("race;", "\u{223d}\u{0331}"),
    ("Racute;", "\u{0154}"),
    ("racute;", "\u{0155}"),
    ("radic;", "\u{221a}"),
    ("raemptyv;", "\u{29b3}"),
    ("rangd;", "\u{2992}"),
    ("range;", "\u{29a5}"),
    ("rangle;", "\u{27e9}"),
    ("rang;", "\u{27e9}"),
    ("Rang;", "\u{27eb}"),
    ("raquo", "\u{00bb}"),
    ("raquo;", "\u{00bb}"),
    ("rarrap;", "\u{2975}"),
    ("rarrbfs;", "\u{2920}"),
    ("rarrb;", "\u{21e5}"),
    ("rarrc;", "\u{2933}"),
    ("rarrfs;", "\u{291e}"),
    ("rarrhk;", "\u{21aa}"),
    ("rarrlp;", "\u{21ac}"),
    ("rarrpl;", "\u{2945}"),
    ("rarrsim;", "\u{2974}"),
    ("rarrtl;", "\u{21a3}"),
    ("Rarrtl;", "\u{2916}"),
    ("rarr;", "\u{2192}"),
    ("Rarr;", "\u{21a0}"),
    ("rArr;", "\u{21d2}"),
    ("rarrw;", "\u{219d}"),
    ("ratail;", "\u{291a}"),
    ("rAtail;", "\u{291c}"),
    ("rationals;", "\u{211a}"),
    ("ratio;", "\u{2236}"),
    ("rbarr;", "\u{290d}"),
    ("rBarr;", "\u{290f}"),
    ("RBarr;", "\u{2910}"),
    ("rbbrk;", "\u{2773}"),
    ("rbrace;", "\u{007d}"),
    ("rbrack;", "\u{005d}"),
    ("rbrke;", "\u{298c}"),
    ("rbrksld;", "\u{298e}"),
    ("rbrkslu;", "\u{2990}"),
    ("Rcaron;", "\u{0158}"),
    ("rcaron;", "\u{0159}"),
    ("Rcedil;", "\u{0156}"),
    ("rcedil;", "\u{0157}"),
    ("rceil;", "\u{2309}"),
    ("rcub;", "\u{007d}"),
    ("Rcy;", "\u{0420}"),
    ("rcy;", "\u{0440}"),
    ("rdca;", "\u{2937}"),
    ("rdldhar;", "\u{2969}"),
    ("rdquor;", "\u{201d}"),
    ("rdquo;", "\u{201d}"),
    ("rdsh;", "\u{21b3}"),
    ("realine;", "\u{211b}"),
    ("realpart;", "\u{211c}"),
    ("reals;", "\u{211d}"),
    ("real;", "\u{211c}"),
    ("rect;", "\u{25ad}"),
    ("reg", "\u{00ae}"),
    ("reg;", "\u{00ae}"),
    ("REG", "\u{00ae}"),
    ("REG;", "\u{00ae}"),
    ("Re;", "\u{211c}"),
    ("ReverseElement;", "\u{220b}"),
    ("ReverseEquilibrium;", "\u{21cb}"),
    ("ReverseUpEquilibrium;", "\u{296f}"),
    ("rfisht;", "\u{297d}"),
    ("rfloor;", "\u{230b}"),
    ("rfr;", "\u{1d52f}"),
    ("Rfr;", "\u{211c}"),
    ("rhard;", "\u{21c1}"),
    ("rHar;", "\u{2964}"),
    ("rharul;", "\u{296c}"),
    ("rharu;", "\u{21c0}"),
    ("Rho;", "\u{03a1}"),
    ("rho;", "\u{03c1}"),
    ("rhov;", "\u{03f1}"),
    ("RightAngleBracket;", "\u{27e9}"),
    ("RightArrowBar;", "\u{21e5}"),
    ("RightArrowLeftArrow;", "\u{21c4}"),
    ("rightarrowtail;", "\u{21a3}"),
    ("rightarrow;", "\u{2192}"),
    ("RightArrow;", "\u{2192}"),
    ("Rightarrow;", "\u{21d2}"),
    ("RightCeiling;", "\u{2309}"),
    ("RightDoubleBracket;", "\u{27e7}"),
    ("RightDownTeeVector;", "\u{295d}"),
    ("RightDownVectorBar;", "\u{2955}"),
    ("RightDownVector;", "\u{21c2}"),
    ("RightFloor;", "\u{230b}"),
    ("rightharpoondown;", "\u{21c1}"),
    ("rightharpoonup;", "\u{21c0}"),
    ("rightleftarrows;", "\u{21c4}"),
    ("rightleftharpoons;", "\u{21cc}"),
    ("rightrightarrows;", "\u{21c9}"),
    ("rightsquigarrow;", "\u{219d}"),
    ("RightTeeArrow;", "\u{21a6}"),
    ("RightTee;", "\u{22a2}"),
    ("RightTeeVector;", "\u{295b}"),
    ("rightthreetimes;", "\u{22cc}"),
    ("RightTriangleBar;", "\u{29d0}"),
    ("RightTriangleEqual;", "\u{22b5}"),
    ("RightTriangle;", "\u{22b3}"),
    ("RightUpDownVector;", "\u{294f}"),
    ("RightUpTeeVector;", "\u{295c}"),
    ("RightUpVectorBar;", "\u{2954}"),
    ("RightUpVector;", "\u{21be}"),
    ("RightVectorBar;", "\u{2953}"),
    ("RightVector;", "\u{21c0}"),
    ("ring;", "\u{02da}"),
    ("risingdotseq;", "\u{2253}"),
    ("rlarr;", "\u{21c4}"),
    ("rlhar;", "\u{21cc}"),
    ("rlm;", "\u{200f}"),
    ("rmoustache;", "\u{23b1}"),
    ("rmoust;", "\u{23b1}"),
    ("rnmid;", "\u{2aee}"),
    ("roang;", "\u{27ed}"),
    ("roarr;", "\u{21fe}"),
    ("robrk;", "\u{27e7}"),
    ("ropar;", "\u{2986}"),
    ("ropf;", "\u{1d563}"),
    ("Ropf;", "\u{211d}"),
    ("roplus;", "\u{2a2e}"),
    ("rotimes;", "\u{2a35}"),
    ("RoundImplies;", "\u{2970}"),
    ("rpargt;", "\u{2994}"),
    ("rpar;", "\u{0029}"),
    ("rppolint;", "\u{2a12}"),
    ("rrarr;", "\u{21c9}"),
    ("Rrightarrow;", "\u{21db}"),
    ("rsaquo;", "\u{203a}"),
    ("rscr;", "\u{1d4c7}"),
    ("Rscr;", "\u{211b}"),
    ("rsh;", "\u{21b1}"),
    ("Rsh;", "\u{21b1}"),
    ("rsqb;", "\u{005d}"),
    ("rsquor;", "\u{2019}"),
    ("rsquo;", "\u{2019}"),
    ("rthree;", "\u{22cc}"),
    ("rtimes;", "\u{22ca}"),
    ("rtrie;", "\u{22b5}"),
    ("rtrif;", "\u{25b8}"),
    ("rtriltri;", "\u{29ce}"),
    ("rtri;", "\u{25b9}"),
    ("RuleDelayed;", "\u{29f4}"),
    ("ruluhar;", "\u{2968}"),
    ("rx;", "\u{211e}"),
    ("Sacute;", "\u{015a}"),
    ("sacute;", "\u{015b}"),
    ("sbquo;", "\u{201a}"),
    ("scap;", "\u{2ab8}"),
    ("Scaron;", "\u{0160}"),
    ("scaron;", "\u{0161}"),
    ("sccue;", "\u{227d}"),
    ("Scedil;", "\u{015e}"),
    ("scedil;", "\u{015f}"),
    ("sce;", "\u{2ab0}"),
    ("scE;", "\u{2ab4}"),
    ("Scirc;", "\u{015c}"),
    ("scirc;", "\u{015d}"),
    ("scnap;", "\u{2aba}"),
    ("scnE;", "\u{2ab6}"),
    ("scnsim;", "\u{22e9}"),
    ("scpolint;", "\u{2a13}"),
    ("scsim;", "\u{227f}"),
    ("sc;", "\u{227b}"),
    ("Sc;", "\u{2abc}"),
    ("Scy;", "\u{0421}"),
    ("scy;", "\u{0441}"),
    ("sdotb;", "\u{22a1}"),
    ("sdote;", "\u{2a66}"),
    ("sdot;", "\u{22c5}"),
    ("searhk;", "\u{2925}"),
    ("searrow;", "\u{2198}"),
    ("searr;", "\u{2198}"),
    ("seArr;", "\u{21d8}"),
    ("sect", "\u{00a7}"),
    ("sect;", "\u{00a7}"),
    ("semi;", "\u{003b}"),
    ("seswar;", "\u{2929}"),
    ("setminus;", "\u{2216}"),
    ("setmn;", "\u{2216}"),
    ("sext;", "\u{2736}"),
    ("sfrown;", "\u{2322}"),
    ("Sfr;", "\u{1d516}"),
    ("sfr;", "\u{1d530}"),
    ("sharp;", "\u{266f}"),
    ("SHCHcy;", "\u{0429}"),
    ("shchcy;", "\u{0449}"),
    ("SHcy;", "\u{0428}"),
    ("shcy;", "\u{0448}"),
    ("ShortDownArrow;", "\u{2193}"),
    ("ShortLeftArrow;", "\u{2190}"),
    ("shortmid;", "\u{2223}"),
    ("shortparallel;", "\u{2225}"),
    ("ShortRightArrow;", "\u{2192}"),
    ("ShortUpArrow;", "\u{2191}"),
    ("shy", "\u{00ad}"),
    ("shy;", "\u{00ad}"),
    ("sigmaf;", "\u{03c2}"),
    ("Sigma;", "\u{03a3}"),
    ("sigma;", "\u{03c3}"),
    ("sigmav;", "\u{03c2}"),
    ("simdot;", "\u{2a6a}"),
    ("simeq;", "\u{2243}"),
    ("sime;", "\u{2243}"),
    ("simgE;", "\u{2aa0}"),
    ("simg;", "\u{2a9e}"),
    ("simlE;", "\u{2a9f}"),
    ("siml;", "\u{2a9d}"),
    ("simne;", "\u{2246}"),
    ("simplus;", "\u{2a24}"),
    ("simrarr;", "\u{2972}"),
    ("sim;", "\u{223c}"),
    ("slarr;", "\u{2190}"),
    ("SmallCircle;", "\u{2218}"),
    ("smallsetminus;", "\u{2216}"),
    ("smashp;", "\u{2a33}"),
    ("smeparsl;", "\u{29e4}"),
    ("smid;", "\u{2223}"),
    ("smile;", "\u{2323}"),
    ("smtes;", "\u{2aac}\u{fe00}"),
    ("smte;", "\u{2aac}"),
    ("smt;", "\u{2aaa}"),
    ("SOFTcy;", "\u{042c}"),
    ("softcy;", "\u{044c}"),
    ("solbar;", "\u{233f}"),
    ("solb;", "\u{29c4}"),
    ("sol;", "\u{002f}"),
    ("Sopf;", "\u{1d54a}"),
    ("sopf;", "\u{1d564}"),
    ("spades;", "\u{2660}"),
    ("spadesuit;", "\u{2660}"),
    ("spar;", "\u{2225}"),
    ("sqcaps;", "\u{2293}\u{fe00}"),
    ("sqcap;", "\u{2293}"),
    ("sqcups;", "\u{2294}\u{fe00}"),
    ("sqcup;", "\u{2294}"),
    ("Sqrt;", "\u{221a}"),
    ("sqsube;", "\u{2291}"),
    ("sqsubseteq;", "\u{2291}"),
    ("sqsubset;", "\u{228f}"),
    ("sqsub;", "\u{228f}"),
    ("sqsupe;", "\u{2292}"),
    ("sqsupseteq;", "\u{2292}"),
    ("sqsupset;", "\u{2290}"),
    ("sqsup;", "\u{2290}"),
    ("SquareIntersection;", "\u{2293}"),
    ("SquareSubsetEqual;", "\u{2291}"),
    ("SquareSubset;", "\u{228f}"),
    ("SquareSupersetEqual;", "\u{2292}"),
    ("SquareSuperset;", "\u{2290}"),
    ("square;", "\u{25a1}"),
    ("Square;", "\u{25a1}"),
    ("SquareUnion;", "\u{2294}"),
    ("squarf;", "\u{25aa}"),
    ("squf;", "\u{25aa}"),
    ("squ;", "\u{25a1}"),
    ("srarr;", "\u{2192}"),
    ("Sscr;", "\u{1d4ae}"),
    ("sscr;", "\u{1d4c8}"),
    ("ssetmn;", "\u{2216}"),
    ("ssmile;", "\u{2323}"),
    ("sstarf;", "\u{22c6}"),
    ("starf;", "\u{2605}"),
    ("Star;", "\u{22c6}"),
    ("star;", "\u{2606}"),
    ("straightepsilon;", "\u{03f5}"),
    ("straightphi;", "\u{03d5}"),
    ("strns;", "\u{00af}"),
    ("subdot;", "\u{2abd}"),
    ("subedot;", "\u{2ac3}"),
    ("sube;", "\u{2286}"),
    ("subE;", "\u{2ac5}"),
    ("submult;", "\u{2ac1}"),
    ("subne;", "\u{228a}"),
    ("subnE;", "\u{2acb}"),
    ("subplus;", "\u{2abf}"),
    ("subrarr;", "\u{2979}"),
    ("subseteqq;", "\u{2ac5}"),
    ("subseteq;", "\u{2286}"),
    ("SubsetEqual;", "\u{2286}"),
    ("subsetneqq;", "\u{2acb}"),
    ("subsetneq;", "\u{228a}"),
    ("subset;", "\u{2282}"),
    ("Subset;", "\u{22d0}"),
    ("subsim;", "\u{2ac7}"),
    ("subsub;", "\u{2ad5}"),
    ("subsup;", "\u{2ad3}"),
    ("sub;", "\u{2282}"),
    ("Sub;", "\u{22d0}"),
    ("succapprox;", "\u{2ab8}"),
    ("succcurlyeq;", "\u{227d}"),
    ("SucceedsEqual;", "\u{2ab0}"),
    ("SucceedsSlantEqual;", "\u{227d}"),
    ("SucceedsTilde;", "\u{227f}"),
    ("Succeeds;", "\u{227b}"),
    ("succeq;", "\u{2ab0}"),
    ("succnapprox;", "\u{2aba}"),
    ("succneqq;", "\u{2ab6}"),
    ("succnsim;", "\u{22e9}"),
    ("succsim;", "\u{227f}"),
    ("succ;", "\u{227b}"),
    ("SuchThat;", "\u{220b}"),
    ("sum;", "\u{2211}"),
    ("Sum;", "\u{2211}"),
    ("sung;", "\u{266a}"),
    ("sup1", "\u{00b9}"),
    ("sup1;", "\u{00b9}"),
    ("sup2", "\u{00b2}"),
    ("sup2;", "\u{00b2}"),
    ("sup3", "\u{00b3}"),
    ("sup3;", "\u{00b3}"),
    ("supdot;", "\u{2abe}"),
    ("supdsub;", "\u{2ad8}"),
    ("supedot;", "\u{2ac4}"),
    ("SupersetEqual;", "\u{2287}"),
    ("Superset;", "\u{2283}"),
    ("supe;", "\u{2287}"),
    ("supE;", "\u{2ac6}"),
    ("suphsol;", "\u{27c9}"),
    ("suphsub;", "\u{2ad7}"),
    ("suplarr;", "\u{297b}"),
    ("supmult;", "\u{2ac2}"),
    ("supne;", "\u{228b}"),
    ("supnE;", "\u{2acc}"),
    ("supplus;", "\u{2ac0}"),
    ("supseteqq;", "\u{2ac6}"),
    ("supseteq;", "\u{2287}"),
    ("supsetneqq;", "\u{2acc}"),
    ("supsetneq;", "\u{228b}"),
    ("supset;", "\u{2283}"),
    ("Supset;", "\u{22d1}"),
    ("supsim;", "\u{2ac8}"),
    ("supsub;", "\u{2ad4}"),
    ("supsup;", "\u{2ad6}"),
    ("sup;", "\u{2283}"),
    ("Sup;", "\u{22d1}"),
    ("swarhk;", "\u{2926}"),
    ("swarrow;", "\u{2199}"),
    ("swarr;", "\u{2199}"),
    ("swArr;", "\u{21d9}"),
    ("swnwar;", "\u{292a}"),
    ("szlig", "\u{00df}"),
    ("szlig;", "\u{00df}"),
    ("Tab;", "\u{0009}"),
    ("target;", "\u{2316}"),
    ("Tau;", "\u{03a4}"),
    ("tau;", "\u{03c4}"),
    ("tbrk;", "\u{23b4}"),
    ("Tcaron;", "\u{0164}"),
    ("tcaron;", "\u{0165}"),
    ("Tcedil;", "\u{0162}"),
    ("tcedil;", "\u{0163}"),
    ("Tcy;", "\u{0422}"),
    ("tcy;", "\u{0442}"),
    ("tdot;", "\u{20db}"),
    ("telrec;", "\u{2315}"),
    ("Tfr;", "\u{1d517}"),
    ("tfr;", "\u{1d531}"),
    ("there4;", "\u{2234}"),
    ("therefore;", "\u{2234}"),
    ("Therefore;", "\u{2234}"),
    ("thetasym;", "\u{03d1}"),
    ("Theta;", "\u{0398}"),
    ("theta;", "\u{03b8}"),
    ("thetav;", "\u{03d1}"),
    ("thickapprox;", "\u{2248}"),
    ("thicksim;", "\u{223c}"),
    ("ThickSpace;", "\u{205f}\u{200a}"),
    ("ThinSpace;", "\u{2009}"),
    ("thinsp;", "\u{2009}"),
    ("thkap;", "\u{2248}"),
    ("thksim;", "\u{223c}"),
    ("THORN", "\u{00de}"),
    ("THORN;", "\u{00de}"),
    ("thorn", "\u{00fe}"),
    ("thorn;", "\u{00fe}"),
    ("TildeEqual;", "\u{2243}"),
    ("TildeFullEqual;", "\u{2245}"),
    ("TildeTilde;", "\u{2248}"),
    ("tilde;", "\u{02dc}"),
    ("Tilde;", "\u{223c}"),
    ("timesbar;", "\u{2a31}"),
    ("timesb;", "\u{22a0}"),
    ("timesd;", "\u{2a30}"),
    ("times", "\u{00d7}"),
    ("times;", "\u{00d7}"),
    ("tint;", "\u{222d}"),
    ("toea;", "\u{2928}"),
    ("topbot;", "\u{2336}"),
    ("topcir;", "\u{2af1}"),
    ("topfork;", "\u{2ada}"),
    ("Topf;", "\u{1d54b}"),
    ("topf;", "\u{1d565}"),
    ("top;", "\u{22a4}"),
    ("tosa;", "\u{2929}"),
    ("tprime;", "\u{2034}"),
    ("trade;", "\u{2122}"),
    ("TRADE;", "\u{2122}"),
    ("triangledown;", "\u{25bf}"),
    ("trianglelefteq;", "\u{22b4}"),
    ("triangleleft;", "\u{25c3}"),
    ("triangleq;", "\u{225c}"),
    ("trianglerighteq;", "\u{22b5}"),
    ("triangleright;", "\u{25b9}"),
    ("triangle;", "\u{25b5}"),
    ("tridot;", "\u{25ec}"),
    ("trie;", "\u{225c}"),
    ("triminus;", "\u{2a3a}"),
    ("TripleDot;", "\u{20db}"),
    ("triplus;", "\u{2a39}"),
    ("trisb;", "\u{29cd}"),
    ("tritime;", "\u{2a3b}"),
    ("trpezium;", "\u{23e2}"),
    ("Tscr;", "\u{1d4af}"),
    ("tscr;", "\u{1d4c9}"),
    ("TScy;", "\u{0426}"),
    ("tscy;", "\u{0446}"),
    ("TSHcy;", "\u{040b}"),
    ("tshcy;", "\u{045b}"),
    ("Tstrok;", "\u{0166}"),
    ("tstrok;", "\u{0167}"),
    ("twixt;", "\u{226c}"),
    ("twoheadleftarrow;", "\u{219e}"),
    ("twoheadrightarrow;", "\u{21a0}"),
    ("Uacute", "\u{00da}"),
    ("Uacute;", "\u{00da}"),
    ("uacute", "\u{00fa}"),
    ("uacute;", "\u{00fa}"),
    ("Uarrocir;", "\u{2949}"),
    ("uarr;", "\u{2191}"),
    ("Uarr;", "\u{219f}"),
    ("uArr;", "\u{21d1}"),
    ("Ubrcy;", "\u{040e}"),
    ("ubrcy;", "\u{045e}"),
    ("Ubreve;", "\u{016c}"),
    ("ubreve;", "\u{016d}"),
    ("Ucirc", "\u{00db}"),
    ("Ucirc;", "\u{00db}"),
    ("ucirc", "\u{00fb}"),
    ("ucirc;", "\u{00fb}"),
    ("Ucy;", "\u{0423}"),
    ("ucy;", "\u{0443}"),
    ("udarr;", "\u{21c5}"),
    ("Udblac;", "\u{0170}"),
    ("udblac;", "\u{0171}"),
    ("udhar;", "\u{296e}"),
    ("ufisht;", "\u{297e}"),
    ("Ufr;", "\u{1d518}"),
    ("ufr;", "\u{1d532}"),
    ("Ugrave", "\u{00d9}"),
    ("Ugrave;", "\u{00d9}"),
    ("ugrave", "\u{00f9}"),
    ("ugrave;", "\u{00f9}"),
    ("uharl;", "\u{21bf}"),
    ("uharr;", "\u{21be}"),
    ("uHar;", "\u{2963}"),
    ("uhblk;", "\u{2580}"),
    ("ulcorner;", "\u{231c}"),
    ("ulcorn;", "\u{231c}"),
    ("ulcrop;", "\u{230f}"),
    ("ultri;", "\u{25f8}"),
    ("Umacr;", "\u{016a}"),
    ("umacr;", "\u{016b}"),
    ("uml", "\u{00a8}"),
    ("uml;", "\u{00a8}"),
    ("UnderBar;", "\u{005f}"),
    ("UnderBrace;", "\u{23df}"),
    ("UnderBracket;", "\u{23b5}"),
    ("UnderParenthesis;", "\u{23dd}"),
    ("UnionPlus;", "\u{228e}"),
    ("Union;", "\u{22c3}"),
    ("Uogon;", "\u{0172}"),
    ("uogon;", "\u{0173}"),
    ("Uopf;", "\u{1d54c}"),
    ("uopf;", "\u{1d566}"),
    ("UpArrowBar;", "\u{2912}"),
    ("UpArrowDownArrow;", "\u{21c5}"),
    ("uparrow;", "\u{2191}"),
    ("UpArrow;", "\u{2191}"),
    ("Uparrow;", "\u{21d1}"),
    ("updownarrow;", "\u{2195}"),
    ("UpDownArrow;", "\u{2195}"),
    ("Updownarrow;", "\u{21d5}"),
    ("UpEquilibrium;", "\u{296e}"),
    ("upharpoonleft;", "\u{21bf}"),
    ("upharpoonright;", "\u{21be}"),
    ("uplus;", "\u{228e}"),
    ("UpperLeftArrow;", "\u{2196}"),
    ("UpperRightArrow;", "\u{2197}"),
    ("upsih;", "\u{03d2}"),
    ("Upsilon;", "\u{03a5}"),
    ("upsilon;", "\u{03c5}"),
    ("upsi;", "\u{03c5}"),
    ("Upsi;", "\u{03d2}"),
    ("UpTeeArrow;", "\u{21a5}"),
    ("UpTee;", "\u{22a5}"),
    ("upuparrows;", "\u{21c8}"),
    ("urcorner;", "\u{231d}"),
    ("urcorn;", "\u{231d}"),
    ("urcrop;", "\u{230e}"),
    ("Uring;", "\u{016e}"),
    ("uring;", "\u{016f}"),
    ("urtri;", "\u{25f9}"),
    ("Uscr;", "\u{1d4b0}"),
    ("uscr;", "\u{1d4ca}"),
    ("utdot;", "\u{22f0}"),
    ("Utilde;", "\u{0168}"),
    ("utilde;", "\u{0169}"),
    ("utrif;", "\u{25b4}"),
    ("utri;", "\u{25b5}"),
    ("uuarr;", "\u{21c8}"),
    ("Uuml", "\u{00dc}"),
    ("Uuml;", "\u{00dc}"),
    ("uuml", "\u{00fc}"),
    ("uuml;", "\u{00fc}"),
    ("uwangle;", "\u{29a7}"),
    ("vangrt;", "\u{299c}"),
    ("varepsilon;", "\u{03f5}"),
    ("varkappa;", "\u{03f0}"),
    ("varnothing;", "\u{2205}"),
    ("varphi;", "\u{03d5}"),
    ("varpi;", "\u{03d6}"),
    ("varpropto;", "\u{221d}"),
    ("varrho;", "\u{03f1}"),
    ("varr;", "\u{2195}"),
    ("vArr;", "\u{21d5}"),
    ("varsigma;", "\u{03c2}"),
    ("varsubsetneqq;", "\u{2acb}\u{fe00}"),
    ("varsubsetneq;", "\u{228a}\u{fe00}"),
    ("varsupsetneqq;", "\u{2acc}\u{fe00}"),
    ("varsupsetneq;", "\u{228b}\u{fe00}"),
    ("vartheta;", "\u{03d1}"),
    ("vartriangleleft;", "\u{22b2}"),
    ("vartriangleright;", "\u{22b3}"),
    ("vBar;", "\u{2ae8}"),
    ("Vbar;", "\u{2aeb}"),
    ("vBarv;", "\u{2ae9}"),
    ("Vcy;", "\u{0412}"),
    ("vcy;", "\u{0432}"),
    ("Vdashl;", "\u{2ae6}"),
    ("vdash;", "\u{22a2}"),
    ("vDash;", "\u{22a8}"),
    ("Vdash;", "\u{22a9}"),
    ("VDash;", "\u{22ab}"),
    ("veebar;", "\u{22bb}"),
    ("veeeq;", "\u{225a}"),
    ("vee;", "\u{2228}"),
    ("Vee;", "\u{22c1}"),
    ("vellip;", "\u{22ee}"),
    ("verbar;", "\u{007c}"),
    ("Verbar;", "\u{2016}"),
    ("VerticalBar;", "\u{2223}"),
    ("VerticalLine;", "\u{007c}"),
    ("VerticalSeparator;", "\u{2758}"),
    ("VerticalTilde;", "\u{2240}"),
    ("vert;", "\u{007c}"),
    ("Vert;", "\u{2016}"),
    ("VeryThinSpace;", "\u{200a}"),
    ("Vfr;", "\u{1d519}"),
    ("vfr;", "\u{1d533}"),
    ("vltri;", "\u{22b2}"),
    ("vnsub;", "\u{2282}\u{20d2}"),
    ("vnsup;", "\u{2283}\u{20d2}"),
    ("Vopf;", "\u{1d54d}"),
    ("vopf;", "\u{1d567}"),
    ("vprop;", "\u{221d}"),
    ("vrtri;", "\u{22b3}"),
    ("Vscr;", "\u{1d4b1}"),
    ("vscr;", "\u{1d4cb}"),
    ("vsubne;", "\u{228a}\u{fe00}"),
    ("vsubnE;", "\u{2acb}\u{fe00}"),
    ("vsupne;", "\u{228b}\u{fe00}"),
    ("vsupnE;", "\u{2acc}\u{fe00}"),
    ("Vvdash;", "\u{22aa}"),
    ("vzigzag;", "\u{299a}"),
    ("Wcirc;", "\u{0174}"),
    ("wcirc;", "\u{0175}"),
    ("wedbar;", "\u{2a5f}"),
    ("wedgeq;", "\u{2259}"),
    ("wedge;", "\u{2227}"),
    ("Wedge;", "\u{22c0}"),
    ("weierp;", "\u{2118}"),
    ("Wfr;", "\u{1d51a}"),
    ("wfr;", "\u{1d534}"),
    ("Wopf;", "\u{1d54e}"),
    ("wopf;", "\u{1d568}"),
    ("wp;", "\u{2118}"),
    ("wreath;", "\u{2240}"),
    ("wr;", "\u{2240}"),
    ("Wscr;", "\u{1d4b2}"),
    ("wscr;", "\u{1d4cc}"),
    ("xcap;", "\u{22c2}"),
    ("xcirc;", "\u{25ef}"),
    ("xcup;", "\u{22c3}"),
    ("xdtri;", "\u{25bd}"),
    ("Xfr;", "\u{1d51b}"),
    ("xfr;", "\u{1d535}"),
    ("xharr;", "\u{27f7}"),
    ("xhArr;", "\u{27fa}"),
    ("Xi;", "\u{039e}"),
    ("xi;", "\u{03be}"),
    ("xlarr;", "\u{27f5}"),
    ("xlArr;", "\u{27f8}"),
    ("xmap;", "\u{27fc}"),
    ("xnis;", "\u{22fb}"),
    ("xodot;", "\u{2a00}"),
    ("Xopf;", "\u{1d54f}"),
    ("xopf;", "\u{1d569}"),
    ("xoplus;", "\u{2a01}"),
    ("xotime;", "\u{2a02}"),
    ("xrarr;", "\u{27f6}"),
    ("xrArr;", "\u{27f9}"),
    ("Xscr;", "\u{1d4b3}"),
    ("xscr;", "\u{1d4cd}"),
    ("xsqcup;", "\u{2a06}"),
    ("xuplus;", "\u{2a04}"),
    ("xutri;", "\u{25b3}"),
    ("xvee;", "\u{22c1}"),
    ("xwedge;", "\u{22c0}"),
    ("Yacute", "\u{00dd}"),
    ("Yacute;", "\u{00dd}"),
    ("yacute", "\u{00fd}"),
    ("yacute;", "\u{00fd}"),
    ("YAcy;", "\u{042f}"),
    ("yacy;", "\u{044f}"),
    ("Ycirc;", "\u{0176}"),
    ("ycirc;", "\u{0177}"),
    ("Ycy;", "\u{042b}"),
    ("ycy;", "\u{044b}"),
    ("yen", "\u{00a5}"),
    ("yen;", "\u{00a5}"),
    ("Yfr;", "\u{1d51c}"),
    ("yfr;", "\u{1d536}"),
    ("YIcy;", "\u{0407}"),
    ("yicy;", "\u{0457}"),
    ("Yopf;", "\u{1d550}"),
    ("yopf;", "\u{1d56a}"),
    ("Yscr;", "\u{1d4b4}"),
    ("yscr;", "\u{1d4ce}"),
    ("YUcy;", "\u{042e}"),
    ("yucy;", "\u{044e}"),
    ("yuml", "\u{00ff}"),
    ("yuml;", "\u{00ff}"),
    ("Yuml;", "\u{0178}"),
    ("Zacute;", "\u{0179}"),
    ("zacute;", "\u{017a}"),
    ("Zcaron;", "\u{017d}"),
    ("zcaron;", "\u{017e}"),
    ("Zcy;", "\u{0417}"),
    ("zcy;", "\u{0437}"),
    ("Zdot;", "\u{017b}"),
    ("zdot;", "\u{017c}"),
    ("zeetrf;", "\u{2128}"),
    ("ZeroWidthSpace;", "\u{200b}"),
    ("Zeta;", "\u{0396}"),
    ("zeta;", "\u{03b6}"),
    ("zfr;", "\u{1d537}"),
    ("Zfr;", "\u{2128}"),
    ("ZHcy;", "\u{0416}"),
    ("zhcy;", "\u{0436}"),
    ("zigrarr;", "\u{21dd}"),
    ("zopf;", "\u{1d56b}"),
    ("Zopf;", "\u{2124}"),
    ("Zscr;", "\u{1d4b5}"),
    ("zscr;", "\u{1d4cf}"),
    ("zwj;", "\u{200d}"),
    ("zwnj;", "\u{200c}"),
];

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
}
