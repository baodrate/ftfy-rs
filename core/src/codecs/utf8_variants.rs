/*
This file defines a codec called "utf-8-variants" (or "utf-8-var"), which can
decode text that's been encoded with a popular non-standard version of UTF-8.
This includes CESU-8, the accidental encoding made by layering UTF-8 on top of
UTF-16, as well as Java's twist on CESU-8 that contains a two-byte encoding for
codepoint 0.

The codec does not at all enforce "correct" CESU-8. For example, the Unicode
Consortium's not-quite-standard describing CESU-8 requires that there is only
one possible encoding of any character, so it does not allow mixing of valid
UTF-8 and CESU-8. This codec *does* allow that.

Characters in the Basic Multilingual Plane still have only one encoding. This
codec still enforces the rule, within the BMP, that characters must appear in
their shortest form. There is one exception: the sequence of bytes `0xc0 0x80`,
instead of just `0x00`, may be used to encode the null character `U+0000`, like
in Java.

If you encode with this codec, you get legitimate UTF-8. Decoding with this
codec and then re-encoding is not idempotent, although encoding and then
decoding is. So this module won't produce CESU-8 for you. Look for that
functionality in the sister module, "Breaks Text For You", coming approximately
never.
*/
extern crate encoding;
use std::str;
use std::string::String;

/*
This regular expression matches all possible six-byte CESU-8 sequences,
plus truncations of them at the end of the string. (If any of the
subgroups matches $, then all the subgroups after it also have to match $,
as there are no more characters to match.)
*/
const CESU8_EXPR: &str = r"(?-u:\xED([\xA0-\xAF]|$)(?-u:[\x80-\xBF]|$)(?-u:\xED|$)(?-u:[\xB0-\xBF]|$)(?-u:[\x80-\xBF]|$))";

/*
This expression matches isolated surrogate characters that aren't
CESU-8.
*/
const SURROGATE_EXPR: &str = r"(?-u:\xed([\xa0-\xbf]|$)(?-u:[\x80-\xbf]|$))";

/*
This expression matches the Java encoding of U+0, including if it's
truncated and we need more bytes.
*/
const NULL_EXPR: &str = r"(?-u:\xc0(\x80|$))";

lazy_static! {
    static ref CESU_8_RE: regex::bytes::Regex = regex::bytes::Regex::new(CESU8_EXPR).unwrap();

    /*
    This regex matches cases that we need to decode differently from
    standard UTF-8.
    */
    static ref SPECIAL_BYTES_RE: regex::bytes::Regex = regex::bytes::Regex::new(
        &(NULL_EXPR.to_string() + "|" + CESU8_EXPR + "|" + SURROGATE_EXPR)
    )
    .unwrap();
}

pub fn variant_decode(data: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    /*
    decoded_segments are the pieces of text we have decoded so far,
    and position is our current position in the byte string. (Bytes
    before this position have been consumed, and bytes after it have
    yet to be decoded.)
    */
    let mut decoded_segments = Vec::new();
    let mut position = 0;

    loop {
        let slice = &data[position..];

        if slice.len() == 0 {
            break;
        }

        // Use _buffer_decode_step to decode a segment of text.
        let (decoded, consumed) = buffer_decode_step(slice)?;

        // Either there's nothing left to decode, or we need to wait
        // for more input. Either way, we're done for now.
        if consumed == 0 {
            break;
        }

        position += consumed;

        // Append the decoded text to the list, and update our position.
        decoded_segments.push(decoded);
    }

    Ok(decoded_segments.into_iter().collect())
}

fn buffer_decode_step(slice: &[u8]) -> Result<(String, usize), Box<dyn std::error::Error>> {
    /*
    There are three possibilities for each decoding step:

    - Decode as much real UTF-8 as possible.
    - Decode a six-byte CESU-8 sequence at the current position.
    - Decode a Java-style null at the current position.

    This method figures out which step is appropriate, and does it.
    */

    // Find the next byte position that indicates a variant of UTF-8.
    if let Some(captures) = SPECIAL_BYTES_RE.captures(slice) {
        let matched = captures.get(0).unwrap();

        let cutoff = matched.start();

        if cutoff > 0 {
            let utf8_prefix = &slice[..cutoff];
            return Ok((str::from_utf8(utf8_prefix)?.to_string(), cutoff));
        }

        // Some byte sequence that we intend to handle specially matches
        // at the beginning of the input.
        if slice.starts_with(&[0xc0]) {
            if slice.len() > 1 {
                // Decode the two-byte sequence 0xc0 0x80.
                Ok(("\u{0000}".to_string(), 2))
            } else {
                // We hit the end of the stream.
                // [0xc0] is never valid UTF-8, so this always errors - matching ftfy's decoder
                Ok((str::from_utf8(slice)?.to_string(), slice.len()))
            }
        } else {
            // Decode a possible six-byte sequence starting with 0xed.
            buffer_decode_surrogates(slice)
        }
    } else {
        Ok((str::from_utf8(slice)?.to_string(), slice.len()))
    }
}

fn buffer_decode_surrogates(slice: &[u8]) -> Result<(String, usize), Box<dyn std::error::Error>> {
    /*
    When we have improperly encoded surrogates, we can still see the
    bits that they were meant to represent.

    The surrogates were meant to encode a 20-bit number, to which we
    add 0x10000 to get a codepoint. That 20-bit number now appears in
    this form:

        11101101 1010abcd 10efghij 11101101 1011klmn 10opqrst

    The CESU8_RE above matches byte sequences of this form. Then we need
    to extract the bits and assemble a codepoint number from them.
    */
    if slice.len() < 6 {
        /*
        We found 0xed near the end of the stream, and there aren't
        six bytes to decode. Delegate to the superclass method to
        handle it as normal UTF-8. It might be a Hangul character
        or an error.
        */
        return Ok((str::from_utf8(slice)?.to_string(), slice.len()));
    } else {
        if CESU_8_RE.is_match(slice) {
            // Given this is a CESU-8 sequence, do some math to pull out
            // the intended 20-bit value, and consume six bytes.
            let codepoint = ((slice[1] as u32 & 0x0F) << 16)
                + ((slice[2] as u32 & 0x3F) << 10)
                + ((slice[4] as u32 & 0x0F) << 6)
                + (slice[5] as u32 & 0x3F)
                + 0x10000;

            let c_u32 = codepoint as u32;

            return Ok((char::from_u32(c_u32).unwrap().to_string(), 6));
        } else {
            // This looked like a CESU-8 sequence, but it wasn't one.
            // 0xed indicates the start of a three-byte sequence, so give
            // three bytes to the superclass to decode as usual.
            return Ok((str::from_utf8(&slice[..3])?.to_string(), 3));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_decode_ascii() {
        let input = "Hello, world!".as_bytes();
        let output = variant_decode(input).unwrap();
        assert_eq!("Hello, world!", output);
    }

    #[test]
    fn test_decode_cesu8() {
        let input = [0xED, 0xA0, 0x81, 0xED, 0xB0, 0x80];
        // Represents the character 🡀 (U+1F800)
        let output = variant_decode(&input).unwrap();
        let output_bytes = output.as_bytes().to_vec();
        assert_eq!(vec![0xF0, 0x90, 0x90, 0x80], output_bytes);
    }

    #[test]
    fn test_decode_empty() {
        let input = [];
        let output = variant_decode(&input).unwrap();
        assert_eq!("", output);
    }

    #[test]
    fn test_decode_null() {
        let input = [0xC0, 0x80];
        let output = variant_decode(&input).unwrap();
        assert_eq!("\0", output);
    }

    #[test]
    fn test_decode_ascii_and_null() {
        let input = [0x41, 0x42, 0xC0, 0x80, 0x43];
        let output = variant_decode(&input).unwrap();
        assert_eq!("AB\0C", output);
    }

    #[test]
    fn test_decode_cesu8_and_null() {
        let input = [0xED, 0xA0, 0x81, 0xED, 0xB0, 0x80, 0xC0, 0x80];
        let output = variant_decode(&input).unwrap();
        let expected_output = String::from_utf8(vec![0xF0, 0x90, 0x90, 0x80, 0x00]).unwrap();
        assert_eq!(expected_output, output);
    }

    #[test]
    fn test_decode_cesu8_and_ascii() {
        let input = [0xED, 0xA0, 0x81, 0xED, 0xB0, 0x80, 0x41, 0x42, 0x43];
        let output = variant_decode(&input).unwrap();
        let expected_output =
            String::from_utf8(vec![0xF0, 0x90, 0x90, 0x80, 0x41, 0x42, 0x43]).unwrap();
        assert_eq!(expected_output, output);
    }

    #[test]
    fn test_decode_multiple_cesu8() {
        let input = [
            0xED, 0xA0, 0x81, 0xED, 0xB0, 0x80, 0xED, 0xA0, 0xA1, 0xED, 0xB1, 0x81,
        ];
        let output = variant_decode(&input).unwrap();
        let expected_output =
            String::from_utf8(vec![0xf0, 0x90, 0x90, 0x80, 0xf0, 0x98, 0x91, 0x81]).unwrap();
        assert_eq!(expected_output, output);
    }

    #[test]
    fn test_decode_multiple_null() {
        let input = [0xC0, 0x80, 0xC0, 0x80, 0xC0, 0x80];
        let output = variant_decode(&input).unwrap();
        assert_eq!("\0\0\0", output);
    }

    #[test]
    fn test_decode_multiple_ascii() {
        let input = [0x41, 0x41, 0x41];
        let output = variant_decode(&input).unwrap();
        assert_eq!("AAA", output);
    }
}

/// Ported from ftfy's `tests/test_encodings.py`
#[cfg(test)]
mod ftfy_test_encodings {
    use super::*;
    use pretty_assertions::assert_eq;

    // A CESU-8 surrogate pair decodes to its astral codepoint, and an overlong NUL decodes to U+0000.
    #[test]
    fn test_cesu8() {
        // TODO: confirm that we're actually decoding as CESU-8
        let test_bytes =
            b"\xed\xa6\x9d\xed\xbd\xb7 is an unassigned character, and \xc0\x80 is null";
        let test_text = "\u{77777} is an unassigned character, and \u{0} is null";
        assert_eq!(variant_decode(test_bytes).unwrap(), test_text);
    }

    #[test]
    fn test_russian_crash() {
        let thebytes = b"\xe8\xed\xe2\xe5\xed\xf2\xe0\xf0\xe8\xe7\xe0\xf6\xe8\xff ";
        // We don't care what the result is, but this shouldn't crash
        let _ = variant_decode(thebytes);
        // TODO: we haven't implemented `guess_bytes()`
    }
}

/// Ported from ftfy's `tests/test_bytes.py`
#[cfg(test)]
mod ftfy_test_bytes {
    use super::*;
    use crate::codecs::sloppy::Codec;
    use crate::codecs::sloppy::MACROMAN;
    use crate::codecs::sloppy::SLOPPY_WINDOWS_1252;
    use pretty_assertions::assert_eq;

    #[test]
    #[ignore = "unable to test without guess_bytes"]
    fn test_guess_bytes() {
        let test_encodings = [
            // TODO: utf-8/utf-16?
            &SLOPPY_WINDOWS_1252,
        ];
        let test_strings = [
            &"Renée\nFleming",
            &"Noël\nCoward",
            &"Señor\nCardgage",
            &"€ • £ • ¥",
            &"¿Qué?",
        ];

        for string in test_strings {
            assert!(!string.is_empty());

            for encoding in test_encodings {
                let encoded = encoding.encode(string).unwrap();
                assert!(!encoded.is_empty());
                // TODO: test with `guess_bytes()` once implemented
                // assert_eq!(guess_bytes(encoded), encoding);
            }

            if string.contains("\n") {
                let string = string.replace("\n", "\r");
                let encoded = MACROMAN.encode(string.as_str()).unwrap();
                assert!(!encoded.is_empty());
                // TODO: test with `guess_bytes()` once implemented
                unimplemented!();
            }
        }
    }

    #[test]
    #[ignore = "unable to test without guess_bytes"]
    fn test_guess_bytes_null() {
        let bowdlerized_null = b"null\xc0\x80separated";
        let expected = "null\x00separated";
        assert_eq!(variant_decode(bowdlerized_null).unwrap(), expected);
        // TODO: enable test after implementing `guess_bytes()`
        unimplemented!()
    }

    #[test]
    #[ignore = "unable to test without incremental decoder"]
    fn test_incomplete_sequences() {
        let test_bytes = b"surrogates: \xed\xa0\x80\xed\xb0\x80 / null: \xc0\x80";
        let test_string = "surrogates: \u{10000} / null: \u{0}";
        assert_eq!(variant_decode(test_bytes).unwrap(), test_string);

        // Test that we can feed this string to decode() in multiple pieces, and no
        // matter where the break between those pieces is, we get the same result.
        // TODO: enable test after implementing incremental decoder
        unimplemented!()
    }

    // A lone trailing 0xc0 must error (matching ftfy's utf-8-variants codec),
    // not be silently dropped as it was before.
    #[test]
    fn test_decode_ascii_then_trailing_c0_errors() {
        // ftfy: b"A\xc0".decode("utf-8-variants") -> UnicodeDecodeError
        let input = [0x41, 0xC0];
        assert!(
            variant_decode(&input).is_err(),
            "lone trailing 0xc0 after ASCII must error, not drop the byte"
        );
    }

    #[test]
    fn test_decode_lone_trailing_c0_errors() {
        // ftfy: b"\xc0".decode("utf-8-variants") -> UnicodeDecodeError
        let input = [0xC0];
        assert!(
            variant_decode(&input).is_err(),
            "lone 0xc0 must error, not decode to empty string"
        );
    }
}
