# Research notes: classic & famous mojibake

Collected 2026-06-13 via web-search snippets (WebFetch/curl blocked in the
sandbox). Confidence: **[S]** garbled string seen verbatim in a snippet;
**[M]** string seen but intended-text/chain reconstructed; **[FLAG]**
memory/computed, never seen in a snippet — must round-trip before use.

Every [S]/[M] entry that became a corpus entry was additionally
**re-computed** from its intended text through the stated chain and the
result compared byte-for-byte against the transcribed garbled string (see
`tools/build_famous.py` and the verification log). Only round-tripping
samples are marked `verified: true`.

## Famous incidents (all promoted to entries/famous-incidents.json)

| Name | Garbled [conf] | Intended | Chain | Source |
|---|---|---|---|---|
| Bush hid the facts | `畂桳栠摩琠敨映捡獴` [S] | `Bush hid the facts` | ASCII as UTF-16LE (IsTextUnicode bug, Notepad 2004) | en.wikipedia.org/wiki/Bush_hid_the_facts |
| 锟斤拷 kūnjīnkǎo | `锟斤拷` [S] | two U+FFFD | UTF-8(��) as GBK | github.com/Ovler-Young/Mojibake-recovery |
| 烫烫烫 | `烫烫烫` [S] | 0xCC fill | MSVC stack-fill bytes as GBK | blog.csdn.net/jarelzhou/...19013037 |
| 屯屯屯 | `屯屯屯` [S] | 0xCD fill | MSVC heap-fill bytes as GBK | cnblogs.com/imjustice/...2623915 |
| бНОПНЯ | `бНОПНЯ` [S] | `Вопрос` | CP1251 as KOI8-R | neolurk.org/wiki/БНОПНЯ |
| оПХБЕР | `оПХБЕР` [S]/[M] | `Привет` | CP1251 as KOI8-R | dic.academic.ru/.../1069825 |
| ALA Aseroë (single) | `AseroÃ«` [S] | `Aseroë` | UTF-8 as Latin-1 | datafix.com.au/BASHing/2020-04-01.html |
| ALA Aseroë (double) | `AseroÃƒÂ«` [S] | `Aseroë` | UTF-8/cp1252 twice | datafix.com.au/BASHing/2020-04-01.html |
| ALA Naïs | `NaÃ¯s` [S] | `Naïs` | UTF-8 as Latin-1 | datafix.com.au/BASHing/2020-04-01.html |
| Norwegian | `SmÃ¸rbrÃ¸d` [S] | `Smørbrød` | UTF-8 as Latin-1 | en.wikipedia.org/wiki/Mojibake |
| Korean | `í•œêµ­ì–´` [S] | `한국어` | UTF-8 as cp1252 | ssojet.com euc-kr-vs-utf-8 |
| Russian привет | `Ð¿Ñ€Ð¸Ð²ÐµÑ‚` [S] | `привет` | UTF-8 as cp1252 | grokipedia.com/page/Mojibake |
| German | `fÃ¼r` [S] | `für` | UTF-8 as Latin-1 | en.wikipedia.org/wiki/Mojibake |
| café | `cafÃ©` [S] | `café` | UTF-8 as Latin-1 | unicodefyi.com/glossary/mojibake |
| em dash | `â€"` [S] | `—` | UTF-8 as cp1252 | adam.scherlis.com .../new-frontiers-in-mojibake |
| aren't/don't | `arenâ€™t…donâ€™t` [S] | curly quotes | UTF-8 as cp1252 | rspeer ftfy 3.0 blog |
| León Rocha's | `LeÃ³n Rochaâ€™s` [S] | `León Rocha's` | cp1252/utf-8 + uncurl (explain tuple given) | alexwlchan.net .../ftfy-fix-and-explain |
| Thai shrug | (fixed `(ง'⌣')ง` [S]) | `(ง'⌣')ง` | UTF-8 as cp1252 | plsfix/ftfy README |
| Japanese mojibake | `譁�蟄怜喧縺�` [S] | `文字化け` | UTF-8 as Shift-JIS | en.wikipedia.org/wiki/Mojibake |
| UTF-8 BOM | `锘` [S] | (BOM) | EF BB BF as GBK | blog.csdn.net/jarelzhou |

## Incidents documented but NOT turned into string entries (no verbatim string)

- **Harry Potter / krakozyabry envelope** (Wikipedia photo): KOI8-R address
  shown as ISO-8859-1, hand-copied onto an envelope, deciphered by Russian
  postal workers. Incident [S], exact garbled address **not** in any
  snippet — not fabricated.
- **Hungarian** `árvíztűrő tükörfúrógép` pangram: ő/ű mangled across
  ISO-8859-2 / CP852 / Windows-1250. Phrase [S]; specific garbling [FLAG],
  so we generate it mechanically in `generate_corpus.py` instead.
- **Polish** *krzaczki*, **Vietnamese** *chữ ma* / *loạn mã*, **Arabic**
  ISO-8859-6/cp1256: context [S], no verbatim garbled string recovered.
- **Outlook Windows-1258 bug** (cloudmailin): headline only.

## "Never lose a dead end" (posts.arborelia.net) — UNRECOVERED

The blog post the user cited could not be retrieved: WebFetch returns 403
for the host, and 10+ search-query variants (exact title, slug,
arborelia+mojibake+hallucination, HN/lobsters/Mastodon discussion)
returned nothing — no snippet, no quote. **No reliable memory of its
text exists, so its contents are not reconstructed here.**

What is independently verifiable, and what this corpus is built around: the
structural fact rspeer's tools rely on — *real mojibake is mechanically
invertible; a mojibake string carries the information needed to recover
the original.* An AI that imagines a "mojibake-looking" string is very
unlikely to produce one that actually decodes back through a real codec
chain. The corpus operationalizes exactly this test: an entry is
`verified: true` only if a concrete encode/decode chain reproduces its
garbled bytes from a real intended string. That filter is what would catch
a hallucinated sample. Adam Scherlis's "New Frontiers in Mojibake" (2022)
covers adjacent ground (em-dash byte walkthrough, invisible-codepoint
oddities) and is reachable, unlike the arborelia post.

## Discovered tools / corpora for future expansion
- uchardet test corpus (gitlab.freedesktop.org/uchardet/uchardet) — clean
  per-language/per-encoding files, ideal seeds for mechanical generation.
- Ovler-Young/Mojibake-recovery — Chinese-focused recovery + examples.
- Dampfkraft "Field Guide to Japanese Mojibake" — 繧/縺/繝 fingerprints,
  EUC-JP-as-Shift-JIS half-width-katakana texture.
- miyagawa/Encode-DoubleEncodedUTF8 — double-encoding test vectors.
