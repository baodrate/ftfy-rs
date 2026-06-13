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

## "Never Lose a Dead End" (posts.arborelia.net, 2024-10-31) — RECOVERED

Could not be fetched from the sandbox (the environment's HTTP egress proxy
denies every non-allowlisted host with `x-deny-reason: host_not_allowed`;
WebFetch, curl-with-sandbox-disabled, and reader proxies like r.jina.ai
are all denied identically, and the blog has no public GitHub source repo).
The user then supplied the post as a PDF, so its content is now known.

**What the post actually says.** rspeer added Windows-1257 (Baltic) support
to ftfy 6.3 and went looking in the OSCAR web-crawl corpus for a *real*
example to test it (deliberately not a constructed one — "then I'd just be
testing whether my own assumptions ... fit my own assumptions"). She found:

> `Å iaip ÄÆdomu, kaip ÄÆsivaizduoji.`  → ftfy → `Šiaip įdomu, kaip įsivaizduoji.`

Chain: UTF-8 decoded as Windows-1257, with the first word's NBSP
(`Å`+U+00A0) flattened to a space. The page (title ≈ "Never Lose a Dead
End Lyrics Review", on the now-parked ratu.lt) had inconsistent mojibake —
e.g. `vaikystÄ—je` containing a *real em dash* (cp1257 0x97 = U+2014) next
to a correctly-used em dash, and LLM-believed "words" like `kokybÄ—s`,
`ÄÆdomu`, and `www.youtube.com/embed/PmqdXrR9wrU`.

**The twist:** the entire page was AI-generated. There was no song, no
interview, no real encoding error. "The LLM that created it generated fake
encoding errors because that's what it believed Lithuanian looks like."
rspeer never got her real Windows-1257 example — "My search led to a dead
end." (Tags: #ai slop #ftfy #mojibake. Related: her wordfreq "text is fake
now" note.)

**The correction this forces on our methodology.** Our earlier framing —
"an LLM-imagined mojibake string won't decode through a real codec chain"
— is *wrong*, and the post is the counterexample. The fake mojibake
round-trips perfectly (verified: `"Šiaip įdomu, kaip įsivaizduoji."`
→ UTF-8 → Windows-1257 → NBSP-flatten reproduces `"Å iaip ÄÆdomu, kaip
ÄÆsivaizduoji."` byte-for-byte). So a round-trip check proves *mechanical
validity* but **cannot** distinguish real-world mojibake from a convincing
AI imitation. Only **provenance** can. That is why the corpus now requires
a citation or justification on every entry, and why this exact example is
included in `entries/ai-hallucinated.json` with
`real_encoding_error: false`.

**Empirical note:** ftfy 6.3.0 (per the post) decoded that sentence, but
ftfy 6.3.1 and current plsfix both *decline* it (the badness gate rejects
it) — and they agree, so it is not a plsfix differential. Clean Baltic
mojibake (`Sąrašai`, `Žalgiris`, `Rīga`) IS recovered identically by both,
confirming windows-1257 support landed in plsfix (the README's old "Baltic
gap" note was stale and has been removed).

## Discovered tools / corpora for future expansion
- uchardet test corpus (gitlab.freedesktop.org/uchardet/uchardet) — clean
  per-language/per-encoding files, ideal seeds for mechanical generation.
- Ovler-Young/Mojibake-recovery — Chinese-focused recovery + examples.
- Dampfkraft "Field Guide to Japanese Mojibake" — 繧/縺/繝 fingerprints,
  EUC-JP-as-Shift-JIS half-width-katakana texture.
- miyagawa/Encode-DoubleEncodedUTF8 — double-encoding test vectors.

## Round 2 research (CJK + double-encoding agents, 2026-06-13)

Two further agents mined CJK and double-encoding samples. New entries
promoted to `famous-incidents.json` (all recomputed from intended text and
byte-verified against the cited form, per the corpus rule):

| Entry | Garbled | Intended | Chain | Source |
|---|---|---|---|---|
| jp-kasou-machine | `莉ｮ諠ｳ繝槭す…` | 仮想マシンサービス | UTF-8 as CP932 | github.com/anthropics/claude-code#36061 |
| jp-zip-hitomi | `é╨é╞é▌.png` | ひとみ.png | cp932 as cp437 | eatpeppershothot.blogspot.com |
| zh-nihao-latin1 | `ä½\xa0å¥½` | 你好 | UTF-8 as Latin-1 | Old New Thing |
| zh-nihao-gbk-latin1 | `ÄãºÃ` | 你好 | GBK as Latin-1 | ssojet |
| beatport-ubersprung | `Ãœbersprung…` | Übersprung (Original Mix) | UTF-8 as cp1252 (ID3 was double-encoded) | gehrcke.de |
| hotel-doctest | `HÃ"TEL` | HÔTEL | UTF-8 as cp1252 | ftfy docs |
| ete | `Ã©tÃ©` | été | UTF-8 as Latin-1 | blog.conceptnet.io |
| mysql-doubly-spanish | `ÃƒÂ¡ ÃƒÂ©…` | á é ó ñ | UTF-8/cp1252 ×2 | jonisalonen.com |
| theyre-curl | `theyâ€™re` | they're | UTF-8 as cp1252 | justinweiss.com |
| ala-{elsinoe,helicoon,parepichloe,zignoella} | `…Ã«`/`…Ã¶` | Elsinoë/Helicoön/… | UTF-8 as Latin-1 | datafix.com.au |
| muller-double | `MÃƒÂ¼ller` | Müller | UTF-8/cp1252 ×2 | blogs.perl.org/chansen |
| jp-fieldguide-{sjis,eucjp} | 繧/縺/繝 texture | 日本語のテキスト | UTF-8 / EUC-JP as Shift-JIS | dampfkraft.com |

### Deliberately NOT added (would require fabrication or non-Python codecs)
- **Vietnamese TCVN3/VISCII** (`tiếng Việt`→`tiÕng ViÖt`, `â`→`aÃ¢`): needs
  TCVN3/VISCII codecs Python lacks; not reproducible without a custom map,
  so excluded rather than transcribed by hand.
- **联通 Notepad bug**: the agent's "GBK bytes look like valid UTF-8" claim
  did not check out — `联通`.encode('gbk') = C1 AA CD A8 is *not* valid
  UTF-8 (it's an IsTextUnicode misdetection, like 'Bush hid the facts',
  not a clean decode). Excluded to avoid a hallucinated chain. (Good
  example of the round-trip rule catching a plausible-but-wrong sample.)
- **ISO-2022-JP escape-stripping** (`?$B%1!<%?%$(B`): escape-sequence
  mangling, not a codec misread; out of scope for both fixers.

### "Never Lose a Dead End" — now recovered (see the dedicated section above)
During web research all four agents failed to surface this post (the
domain is indexed, but no mojibake/ftfy post on it is — and the egress
proxy blocks direct fetches). The user supplied it as a PDF, so its
content and its actual lesson are now captured above, and the example
itself lives in `entries/ai-hallucinated.json`. Note the lesson is the
*opposite* of what we first assumed: hallucinated mojibake CAN round-trip,
so provenance — not just invertibility — is the real discriminator.
