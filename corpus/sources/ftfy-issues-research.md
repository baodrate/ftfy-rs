# Research notes: mojibake samples mined from GitHub issues & web

Collected 2026-06-13 by searching GitHub issue bodies (verbatim) and web
snippets. Confidence legend: [S] = seen verbatim in source; [R] =
reconstructed — must be mechanically verified before use as a string.

These notes are the raw provenance record for `entries/ftfy-issues.json`
(built by `tools/build_issues.py`). Where a corruption chain is known, the
corpus stores the *computed* mojibake, not the transcription below —
transcriptions can be garbled in transit, computed chains cannot.

## Gold-standard pairs (verbatim garbled + intended, maintainer-engaged)

| Issue | Garbled (verbatim) | Intended | Chain | Lang | Status |
|---|---|---|---|---|---|
| #229 | `ÃY` in a surname | `ß` | UTF-8 C3 9F as Latin-1, then Ÿ→Y lossy | de | open failure |
| #141 | `zo¿n beetje alle camera¿s`, `of UC¿TM` | `zo'n`/`camera's` (U+2019), `UC™` | ’/™ → `¿` lossy | nl | unrecoverable |
| #29 | `¼Ò¸®¿¤ - »ç¶ûÇÏ´Â ÀÚ¿©` | `소리엘 - 사랑하는 자여` | EUC-KR as Latin-1 | ko | feature request |
| #149 | `Officiâš´le gecreâš´erde patatten` | `Officiële gecreëerde patatten` | nonstandard ë mangle; ftfy wrongly → U+26B4 | nl | wrong fix |
| #168 | `SchlĂźsselwĂśrter` | `Schlüsselwörter` | UTF-8 as ISO-8859-2; ftfy assumes cp1250 | de | wrong fix |
| #222 | `Ã¥klagarmyndighets` | `åklagarmyndighets` | UTF-8 as Latin-1, at string start | sv | bug |
| #119 | `Ä×èÈÄÄî▒è¤ô_üiâAâjâüâpâXüj_10òb.png` | `時間試験観点（アニメパス）_10秒.png` | Shift-JIS as cp850 | ja | not fixed |
| #231 | `п∙я│п╩п╦ п▓я▀ ...` | `Если Вы не являетесь адресатом...` | KOI8-R-ish via CP866/CP437 box chars | ru | open |
| #192 | `Ä°stanbul`, `RÄ«ga` | `İstanbul`, `Rīga` | UTF-8 as cp1252 | tr/lv | fixed |
| #217 | `Python åˆ\xa0é™¤åˆ—è¡¨ä¸\xadçš„å…ƒç´` | `Python 删除列表中的元素` | UTF-8 as Latin-1, truncated mid-char | zh | edge case |
| #190 | `beï¿œindiging` | `beëindiging` | U+FFFD-adjacent artifact | nl | known failure |
| #203 | `Ã…lesund` | `Ålesund` | UTF-8 as Latin-1/cp1252 | no | fixed |
| #210 | `SÄ…raÅ¡ai` | `Sąrašai` | UTF-8 as windows-1257 | lt | was unsupported |
| #202 | `Bremer/Mccoy – DrÃ¥ber` | `Bremer/Mccoy – Dråber` | partial UTF-8/Latin-1 (å only); ftfy once gave `РDr̴` | da | wrong-fix bug, fixed |
| #146 | `€“Echinacea en Salvia€“` | en-dashes | U+2013 as cp1252 with â dropped | nl | fixed |
| #178 | `as\udced.mp3`, `presi\udcf3n.mp3` | `así`, `presión` | Latin-1 via surrogateescape | es | known failure |
| #154 | `ğŸ˜Š` | 😊 U+1F60A | UTF-8 as windows-1254 | emoji | fixed |
| #180 | `CU╔NTENOS EN ESPA╤OL` | `CUÉNTENOS EN ESPAÑOL` | Latin-1 as CP437 (receipt printer) | es | fixed |
| #169 | `ğŸ�š Cooked Rice` (with U+FFFD) | `🍚 Cooked Rice` | UTF-8 as cp1254 + one byte lost | emoji | partially unrecoverable |
| #169 | `YaÃ«l` | `Yaël` | UTF-8 as Latin-1 | fr | works |
| #103 | `KÃ¶nig`, `K√∂nig`, `K�nig`, `K\x94nig` | `König` | latin1 / MacRoman / FFFD / bare cp1252 byte | de | MacRoman added |
| #164 | `pietÃ\xa0?` | `pietà?` | UTF-8 as Latin-1 (NBSP second byte) | it | fixed |
| #159 | `ì¤€ë‹¤ê³`, `ê°€ì§€ê³` | `준다고`, `가지고` | UTF-8 Korean as Latin-1, truncated final byte | ko | fixed |
| #157 | `LinkÃ¶pings Universitet,\xa0LiU` | `Linköpings Universitet, LiU` | UTF-8 as cp1252 + real NBSP present | sv | fixed |
| #123 | `PUERTO BOYACÁ, BoyacÃ¡` | `Boyacá` | UTF-8 as Latin-1 next to legit accents | es | fixed |
| #120 | `\x00s\x00t\x00a\x00n...` | `standard 07_51.ttf` | UTF-16-BE as 8-bit | en | out of scope |
| #97 | `ongeÃ«venaard` | `ongeëvenaard` | UTF-8 as Latin-1 | nl | fixed |
| #83 | `â??Let the woman take care of youâ??`, `loverÃ¢??s` | curly quotes | UTF-8 quotes with ?-substitution (lossy) | en | known failure |
| #77 | `Ná»‘i láº¡i tuáº§n tra...` | `Nối lại tuần tra...` | UTF-8 Vietnamese as cp1252 w/ corrupted \x9x | vi | failed then |
| #52 | `a€¢ Strong telecom background ... a€“` | `•`, `–` | UTF-8 as cp1252 with â→a corruption | en | known failure |
| #96 | `fÃ¼r`, `BurkinabÃ©` | `für`, `Burkinabé` | UTF-8 as Latin-1 | de/fr | fixed |
| #225 | `ÃƒÅ½Ã‚Â¤ÃƒÅ½Ã¢â‚¬Ëœ…` | Greek, possibly ΤΑΞΙΔΙ ΞΑΝΘΗ (unconfirmed LLM inference — hallucination risk!) | multi-pass + FFFD loss | el | open |
| #18 | `PrevŽn ... c—digo penal` | `Prevén ... código penal` | MacRoman/cp1252 ambiguity | es | open |
| #128 | `DemgemÃ¤Ã ... 3Ã6 Meter` | `Demgemäß ... 3×6 Meter` | UTF-8 bytes as \u escapes | de | guidance |
| #66 | `I&amp;#x92;m blue, da ba dee da ba doo&amp;#133;` | `I'm blue...` | cp1252 numeric character references | en | regression discussion |
| #34 | `Ã¨Â¢â€¹...` GB-series soup | unknown | GBK through cp1252, multiply corrupted | zh | known limitation |

## Other trackers
- chardet #296 (MacRoman vs cp1252 misdetection), #148 (cp1254 misdetect of UTF-8)
- charset_normalizer #358 (Spanish CP1252 detected Big5!), #75 (CP866 vs CP1125), #587 (GB2312 vs Big5), #121 (uchardet corpus links)
- WordPress Trac #7683 (é→Ã© after 2.6 upgrade; latin1_swedish_ci holding UTF-8)
- jonisalonen.com MySQL latin1/UTF-8 articles (Ã¡ Ã© Ã³ Ã± in 30k forum posts)
- gehrcke.de Beatport ID3 double-UTF-8
- justinweiss.com "from theyâ€™re to they're"
- Japanese ZIP filename lore: cp932 filenames decoded as CP437 by zip tools

## Fuzzer-seed ideas surfaced
- truncated mojibake (UTF-8 cut mid-character before misdecoding): #217, #159
- mixed legit-accents + mojibake in one string: #123, #202
- mojibake at string start: #222
- real NBSP adjacent to mojibake: #157, #164
- lossy substitutions on top: #141 (¿), #83/#52 (?), #229 (Ÿ→Y), #190/#169 (U+FFFD)
- UTF-16 NUL interleaving: #120
- \u-escaped mojibake: #128
- double-escaped HTML entities of cp1252 control range: #66
