#!/usr/bin/env python3
"""Generate the mechanically-constructed part of the mojibake corpus.

Every entry here is mojibake *by construction*: a seed sentence (written
correctly, in its real orthography) is pushed through an explicit
corruption chain. These are the "artificial examples" rspeer describes in
"Never Lose a Dead End" — useful for testing the *mechanism* exhaustively
and across encodings, and honestly labelled as synthetic (never passed off
as found-in-the-wild). The found/famous/issue sets supply real-world
provenance on top.

Two things every entry carries, per the corpus provenance rule:

* **seed provenance** — a citation (or, for constructed strings, a
  justification) for where the seed sentence comes from. Most seeds are
  well-known pangrams, the "I Can Eat Glass" UTF-8 sampler, the Iroha, or
  ftfy's own fixtures.
* **chain rationale** — why the corruption is a realistic real-world
  failure, and whether ftfy's repair codec set can even reverse it
  (``ftfy_repair_supported``). Chains whose mis-decode uses an encoding
  *outside* ftfy's candidate set are kept deliberately: ftfy (and plsfix)
  cannot fix them, and a no-op is the correct, predictable behavior — a
  direct test of the "no false positives" guarantee.

Output: corpus/entries/generated-verified.json (deterministic; rerun then
run verify_corpus.py --write).
"""

from __future__ import annotations

import json
import sys
import unicodedata
from pathlib import Path

import ftfy

sys.path.insert(0, str(Path(__file__).parent))
from transforms import ChainError, apply_chain  # noqa: E402

OUT = Path(__file__).resolve().parent.parent / "entries" / "generated-verified.json"

# ftfy's (and plsfix's) repair-candidate encodings — see ftfy/chardata.py
# CHARMAP_ENCODINGS and core/src/chardata.rs. A UTF-8 mojibake is fixable
# in principle only if the mis-decode used one of these.
FTFY_CHARMAP = {
    "latin-1", "sloppy-windows-1252", "sloppy-windows-1251",
    "sloppy-windows-1250", "sloppy-windows-1253", "sloppy-windows-1254",
    "sloppy-windows-1257", "iso-8859-2", "macroman", "cp437",
}

# --- Seed provenance ---------------------------------------------------
# kind -> (source citation, justification)
PANGRAM_SRC = ("https://clagnut.com/blog/2380/ (Onno Hansen / R. Ishida "
               "multilingual pangram collection) and "
               "https://en.wikipedia.org/wiki/Pangram")
SEED_PROV = {
    "pangram": (PANGRAM_SRC,
        "Widely-circulated pangram for this language, catalogued in the "
        "standard multilingual pangram collections. Pangrams pack the full "
        "accented-letter repertoire of the orthography into one line, which "
        "maximizes the surface available for encoding corruption — ideal as "
        "a mojibake seed."),
    "glass": ("https://kermitproject.org/utf8.html and "
              "https://www.columbia.edu/~fdc/utf8/ (Frank da Cruz, "
              "'I Can Eat Glass' / UTF-8 Sampler)",
        "A translation of the 'I can eat glass, it doesn't hurt me' phrase, "
        "a long-standing multilingual Unicode test string (some seeds append "
        "a short clause). Chosen to exercise non-Latin scripts."),
    "iroha": ("https://en.wikipedia.org/wiki/Iroha",
        "The Iroha, a classical Japanese poem that uses every kana exactly "
        "once — effectively the original pangram. Dense kana make it a strong "
        "Shift-JIS / EUC-JP corruption seed."),
    "kuhn": ("https://www.cl.cam.ac.uk/~mgk25/ucs/examples/UTF-8-demo.txt "
             "(Markus Kuhn, UTF-8 demo / sampler)",
        "Mathematics line from Markus Kuhn's canonical UTF-8 demo file; "
        "exercises mathematical-operator and Greek codepoints."),
    "rustaveli": ("https://en.wikipedia.org/wiki/The_Knight_in_the_Panther%27s_Skin",
        "Opening lines of Shota Rustaveli's Georgian national epic (the "
        "author is named in the text). Georgian-script seed."),
    "ftfy-readme": ("plsfix/ftfy README; "
                    "third-party/python-ftfy/tests/test-cases/in-the-wild.json",
        "A clean form of a real-world mojibake example documented by ftfy, "
        "reused here as a seed for systematic corruption."),
    "ftfy-wild": ("third-party/python-ftfy/tests/test-cases/in-the-wild.json",
        "Intended text of a real-world case from ftfy's in-the-wild test set."),
    "constructed": ("constructed for this corpus (corpus/tools/generate_corpus.py)",
        "Constructed test string, not an attested quotation — included to "
        "exercise a specific feature (see the seed's note)."),
}

# Seeds: (lang, text, legacy-codecs-it-was-historically-stored-in, prov-kind)
SEEDS = [
    ("fr", "Le cœur déçu mais l'âme plutôt naïve, Louÿs rêva de crapaüter en canoë au delà des îles, près du mälström où brûlent les novæ.", ["windows-1252", "macroman", "cp437", "cp850"], "pangram"),
    ("fr", "à perturber la réflexion de l'humanité", ["windows-1252"], "ftfy-readme"),
    ("de", "Falsches Üben von Xylophonmusik quält jeden größeren Zwerg. Die heiße Zypernsonne quälte Max und Victoria ja böse auf dem Weg bis zur Küste.", ["windows-1252", "cp437", "cp850", "macroman"], "pangram"),
    ("de", "„Handwerk bringt dich überall hin“: Von der YOU bis nach Monaco", ["windows-1252"], "ftfy-wild"),
    ("es", "El pingüino Wenceslao hizo kilómetros bajo exhaustiva lluvia y frío, añoraba a su querido cachorro.", ["windows-1252", "cp850"], "pangram"),
    ("pt", "À noite, vovô Kowalsky vê o ímã cair no pé do pinguim queixoso e vovó põe açúcar no chá de tâmaras do jabuti feliz.", ["windows-1252"], "pangram"),
    ("it", "Ma la volpe, col suo balzo, ha raggiunto il quieto Fido. Perché l'ascensore non funziona più?", ["windows-1252"], "pangram"),
    ("ro", "Muzicologă în bej vând whisky și tequila, preț fix. Înțelegerea științifică", ["windows-1250", "iso-8859-2"], "pangram"),
    ("hu", "Árvíztűrő tükörfúrógép. Jó foxim és don Quijote húszwattos lámpánál ülve egy pár bűvös cipőt készít.", ["windows-1250", "iso-8859-2", "cp852"], "pangram"),
    ("cs", "Příliš žluťoučký kůň úpěl ďábelské ódy. Zvlášť zákeřný učeň s ďolíčky běží podél zóny úlů.", ["windows-1250", "iso-8859-2", "cp852"], "pangram"),
    ("pl", "Zażółć gęślą jaźń. Pchnąć w tę łódź jeża lub ośm skrzyń fig.", ["windows-1250", "iso-8859-2"], "pangram"),
    ("sk", "Kŕdeľ šťastných ďatľov učí pri ústí Váhu mĺkveho koňa obhrýzať kôru a žrať čerstvé mäso.", ["windows-1250"], "pangram"),
    ("hr", "Gojazni đačić s biciklom drži hmelj i finu vatu u džepu nošnje.", ["windows-1250"], "pangram"),
    ("tr", "Pijamalı hasta yağız şoföre çabucak güvendi. Öğrencilerimiz İstanbul'da.", ["windows-1254", "iso-8859-9"], "pangram"),
    ("sv", "Flygande bäckasiner söka hwila på mjuka tuvor. Räksmörgås med surströmming.", ["windows-1252", "macroman", "cp437"], "pangram"),
    ("no", "Vår sære Zulu fra badeøya spilte jo whist og quickstep i min taxi. Blåbærsyltetøy på smørbrød.", ["windows-1252", "cp865"], "pangram"),
    ("da", "Quizdeltagerne spiste jordbær med fløde, mens cirkusklovnen Wolther spillede på xylofon. Høj bly gom vandt fræk sexquiz på wc.", ["windows-1252", "cp865"], "pangram"),
    ("fi", "Törkylempijävongahdus. On sangen hauskaa, että polkupyörä on maanteiden jokapäiväinen ilmiö.", ["windows-1252"], "pangram"),
    ("is", "Kæmi ný öxi hér, ykist þjófum nú bæði víl og ádrepa. Sævör grét áðan því úlpan var ónýt.", ["windows-1252", "cp861"], "pangram"),
    ("et", "See väike mölder jõuab rongile hüpata. Õunad ja ürdid künkal.", ["windows-1257"], "constructed"),
    ("lt", "Įlinkdama fechtuotojo špaga sublykčiojusi pragręžė apvalų arbūzą. Žalgirio mūšis įvyko 1410 metais.", ["windows-1257"], "pangram"),
    ("lv", "Glāžšķūņa rūķīši dzērumā čiepj Baha koncertflīģeļu vākus. Sarkanās jāņogas garšo labāk.", ["windows-1257"], "pangram"),
    ("ru", "Съешь же ещё этих мягких французских булок да выпей чаю. В чащах юга жил бы цитрус? Да, но фальшивый экземпляр!", ["windows-1251", "koi8-r", "iso-8859-5", "cp866"], "pangram"),
    ("ru", "дороге Из-под #футбол", ["windows-1251", "koi8-r"], "ftfy-wild"),
    ("uk", "Чуєш їх, доцю, га? Кумедна ж ти, прощайся без ґольфів! Жебракують філософи при ґанку церкви.", ["windows-1251", "koi8-u"], "pangram"),
    ("bg", "Жълтата дюля беше щастлива, че пухът, който цъфна, замръзна като гьон. За миг бях в чужд плюшен скърцащ фотьойл.", ["windows-1251"], "pangram"),
    ("sr", "Љубазни фењерџија чађавог лица хоће да ми покаже штос. Ниш и Ћуприја су градови у Србији.", ["windows-1251"], "pangram"),
    ("el", "Ξεσκεπάζω την ψυχοφθόρα βδελυγμία. Θέλει αρετή και τόλμη η ελευθερία. Καλημέρα, κόσμε!", ["windows-1253", "iso-8859-7"], "pangram"),
    ("he", "דג סקרן שט בים מאוכזב ולפתע מצא חברה. שלום עולם, מה שלומך היום?", ["windows-1255", "iso-8859-8"], "pangram"),
    ("ar", "نص حكيم له سر قاطع وذو شأن عظيم مكتوب على ثوب أخضر ومغلف بجلد أزرق. مرحبا بالعالم!", ["windows-1256"], "pangram"),
    ("fa", "بر اساس قانون اساسی، زبان رسمی ایران فارسی است. کیمیاگری با طلا و نقره.", ["windows-1256"], "constructed"),
    ("th", "เป็นมนุษย์สุดประเสริฐเลิศคุณค่า กว่าบรรดาฝูงสัตว์เดรัจฉาน จงฝ่าฟันพัฒนาวิชาการ", ["cp874"], "pangram"),
    ("vi", "Tôi yêu tiếng nước tôi từ khi mới ra đời. Đêm qua trời mưa to quá, đường phố ngập hết cả.", ["windows-1258"], "constructed"),
    ("ja", "いろはにほへと ちりぬるを 色は匂へど 散りぬるを。文字化けテストです。", ["shift_jis", "euc-jp"], "iroha"),
    ("ja", "ハンカクカナモジ ｱｲｳｴｵ と全角の混在テキスト", ["shift_jis"], "constructed"),
    ("zh-Hans", "我能吞下玻璃而不伤身体。汉字乱码测试:中文编码很复杂。", ["gbk"], "glass"),
    ("zh-Hant", "我能吞下玻璃而不傷身體。繁體中文亂碼測試:編碼問題很常見。", ["big5"], "glass"),
    ("ko", "키스의 고유조건은 입술끼리 만나야 하고 특별한 기술은 필요치 않다. 다람쥐 헌 쳇바퀴에 타고파.", ["euc-kr"], "pangram"),
    ("hi", "ऋषियों को सताने वाले दुष्ट राक्षसों के राजा रावण का सर्वनाश करने वाले विष्णुवतार भगवान श्रीराम", [], "pangram"),
    ("ka", "ვეპხის ტყაოსანი შოთა რუსთაველი. ღმერთსი შემვედრე, ნუთუ კვლა დამხსნას სოფლისა შრომასა.", [], "rustaveli"),
    ("emoji", "I'm so happy 😂👍🏽 family: 👨‍👩‍👧‍👦 flag: 🇺🇳 star: ⭐ heart: ❤️", [], "constructed"),
    ("emoji", "(ง'⌣')ง fight! ¯\\_(ツ)_/¯ ☺💕", [], "constructed"),
    ("mixed", "Naïve café-goers paid €5 — “très chic” — for ½ a crème brûlée…", ["windows-1252", "macroman"], "constructed"),
    ("symbols", "∮ E·da = Q, n → ∞, ∑ f(i) = ∏ g(i); ⊥ ≠ ∥ — α β γ δ", [], "kuhn"),
    ("hy", "Բարև, աշխարհ։ Արագ շագանակագույն աղվեսը ցատկում է ծույլ շան վրայով։", [], "constructed"),
    ("ka2", "სწრაფი ყავისფერი მელა ხტება ზარმაცი ძაღლის თავზე. გამარჯობა, მსოფლიო!", [], "constructed"),
    ("bn", "আমি কাচ খেতে পারি, তাতে আমার কোনো ক্ষতি হয় না। দ্রুত বাদামী শিয়াল।", [], "glass"),
    ("ta", "நான் கண்ணாடி சாப்பிடுவேன், அதனால் எனக்கு வலி ஏற்படாது. வணக்கம் உலகம்.", [], "glass"),
    ("te", "నేను గాజు తినగలను, అది నాకు హాని కలిగించదు. నమస్కారం ప్రపంచం.", [], "glass"),
    ("ml", "എനിക്ക് ഗ്ലാസ് കഴിക്കാം, അതെനിക്ക് വേദനയുണ്ടാക്കില്ല. നമസ്കാരം ലോകം.", [], "glass"),
    ("am", "ሰላም ለዓለም። ፈጣኑ ቡናማ ቀበሮ በሰነፉ ውሻ ላይ ይዘላል።", [], "constructed"),
    ("my", "မင်္ဂလာပါ ကမ္ဘာလောက။ မြန်သော အညိုရောင် မြေခွေး။", [], "constructed"),
    ("km", "សួស្ដី​ពិភពលោក។ កញ្ជ្រោង​ត្នោត​រហ័ស​លោត​លើ​ឆ្កែ​ខ្ជិល។", [], "constructed"),
    ("si", "ආයුබෝවන් ලෝකය. වේගවත් දුඹුරු හිවලා කම්මැලි බල්ලා උඩින් පනියි.", [], "constructed"),
    ("nl", "Pa's wijze lynx bezag vroom het fikse aquaduct. Zo'n café met een croissant.", ["windows-1252"], "pangram"),
    ("ca", "Jove xef, porti whisky amb quinze glaçons d'hidrogen, coi! L'çà i enllà.", ["windows-1252"], "pangram"),
    ("eu", "Iñaki Gorrotxategiren familia jatorrizko Azkoitiakoa da. Kaixo mundua!", ["windows-1252"], "constructed"),
    ("ga", "Chuaigh bé mhórshách le dlúthspád fíorfhinn trí hata mo dhea-phorcáin bhig.", ["windows-1252"], "pangram"),
    ("cy", "Parciais fy jac codi baw hud llawn dŵr ger tŷ Mabon. Helô, fyd!", ["windows-1252"], "pangram"),
    ("mt", "Il-bniedem għaref jagħżel it-triq it-tajba. Ċaqlaq iż-żraben ħomor.", ["windows-1252"], "constructed"),
    ("eo", "Eĥoŝanĝo ĉiuĵaŭde. La rapida bruna vulpo saltas super la maldiligenta hundo.", ["windows-1252"], "pangram"),
    ("mixed2", "Café ☕ + naïve 🤔 + €100 → «façade» — 日本語 mixed with ñoño", ["windows-1252"], "constructed"),
]

# Per-seed extra justification for the constructed ones.
CONSTRUCTED_NOTE = {
    "et": "the standard Estonian pangram differs; this is a constructed Estonian sentence covering õ/ä/ö/ü, used to seed windows-1257 corruption.",
    "fa": "a Persian sentence (about Persian being Iran's official language), used to seed windows-1256 corruption — not an attested pangram.",
    "vi": "a Vietnamese sentence with dense diacritic stacking, used to seed windows-1258 corruption.",
    "ja": "mixes half-width katakana (ｱｲｳｴｵ) with full-width text to exercise Shift-JIS single/double-byte handling and fix_character_width.",
    "emoji": "exercises astral-plane emoji, ZWJ family sequences, skin-tone modifiers, and flag (regional-indicator) pairs.",
    "hy": "constructed Armenian (greeting + quick-fox style) to exercise Armenian-script handling.",
    "ka2": "constructed Georgian 'quick brown fox' style sentence.",
    "am": "constructed Amharic (Geʽez script) sentence.",
    "my": "constructed Burmese sentence (complex script with stacked consonants).",
    "km": "constructed Khmer sentence (complex script, includes ZWSP word breaks).",
    "si": "constructed Sinhala sentence.",
    "eu": "constructed Basque sentence.",
    "mt": "constructed Maltese sentence (ċ/ġ/ħ/ż).",
    "mixed": "mixed Latin accents, curly quotes, €, em dash, ½, and ellipsis — a 'typical Western web text' stressor.",
    "mixed2": "deliberately mixes scripts (Latin accents, emoji, €, guillemets, CJK) in one line.",
}

# --- Chains ------------------------------------------------------------
# (suffix, description, chain, rationale)
def _rationale(text):
    return text

UNIVERSAL_CHAINS = [
    ("u8-latin1", "UTF-8 bytes read as Latin-1",
     [{"encode": "utf-8"}, {"decode": "latin-1"}],
     "UTF-8 text served/stored as ISO-8859-1 — the single most common mojibake on the web (wrong Content-Type, latin1 DB column)."),
    ("u8-1252", "UTF-8 bytes read as (sloppy) Windows-1252",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"}],
     "UTF-8 read as Windows-1252, the default 'ANSI' codepage on Western Windows; near-ubiquitous."),
    ("u8-1251", "UTF-8 bytes read as (sloppy) Windows-1251",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1251"}],
     "UTF-8 Cyrillic read as the Russian/Bulgarian Windows codepage."),
    ("u8-mac", "UTF-8 bytes read as MacRoman",
     [{"encode": "utf-8"}, {"decode": "macroman"}],
     "UTF-8 read as MacRoman — classic on files created/opened across Mac and Windows."),
    ("u8-437", "UTF-8 bytes read as DOS codepage 437",
     [{"encode": "utf-8"}, {"decode": "cp437"}],
     "UTF-8 shown through the original IBM-PC OEM codepage (consoles, old ZIP tools)."),
    ("u8-1252-x2", "double mojibake: UTF-8/Windows-1252 twice",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"encode": "utf-8"}, {"decode": "sloppy-windows-1252"}],
     "Two round-trips through a UTF-8/cp1252 pipeline — e.g. data re-imported through the same broken step twice."),
    ("u8-1252-x3", "triple mojibake: UTF-8/Windows-1252 three times",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"encode": "utf-8"}, {"decode": "sloppy-windows-1252"}],
     "Three layers; the ftfy README's Mona-Lisa example is this depth."),
    ("u8-1252-latin1-mix", "UTF-8/Windows-1252, re-encoded UTF-8, read as Latin-1",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"encode": "utf-8"}, {"decode": "latin-1"}],
     "Mixed-codec double layer; happens when two systems disagree about the codepage at each hop."),
    ("u8-1252-nbsp", "UTF-8/Windows-1252 with NBSP flattened to space",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"transform": "nbsp_to_space"}],
     "An HTML/CMS pipeline turned the recovered U+00A0 into an ASCII space — the case ftfy's restore_byte_a0 targets."),
    ("u8-1252-nbsp-gone", "UTF-8/Windows-1252 with NBSP deleted",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"transform": "nbsp_to_nothing"}],
     "Same but the NBSP was dropped entirely (some scrapers strip it)."),
    ("u8-1252-curl", "UTF-8/Windows-1252 then smart-quoted by a CMS",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"transform": "curl_apostrophe"}],
     "A blog/word-processor applied 'smart quotes' on top of the mojibake; can't be decoded until the quote is straightened."),
    ("u8-latin1-entities", "UTF-8/Latin-1 then HTML-escaped with named entities",
     [{"encode": "utf-8"}, {"decode": "latin-1"},
      {"transform": "html_named"}],
     "The mojibake was then HTML-entity-escaped before being stored as plain text."),
    ("cesu8-1252", "CESU-8 (Java-style surrogates) read as sloppy Windows-1252",
     [{"encode": "cesu-8"}, {"decode": "sloppy-windows-1252"}],
     "CESU-8 from broken Java/Oracle stacks (astral chars as 6-byte surrogate pairs) read as cp1252."),
    ("cesu8-latin1", "CESU-8 (Java-style surrogates) read as Latin-1",
     [{"encode": "cesu-8"}, {"decode": "latin-1"}],
     "CESU-8 read as Latin-1."),
    ("u8-lossy-ascii", "UTF-8 read as ASCII with replacement characters",
     [{"encode": "utf-8"}, {"decode": "ascii", "errors": "replace"}],
     "A strict ASCII reader replaced every non-ASCII byte with U+FFFD — lossy and unrecoverable by design."),
]

# UTF-8 mojibake whose mis-decode uses an encoding OUTSIDE ftfy's repair
# set. Genuine, reversible-by-hand mojibake that ftfy/plsfix cannot fix; a
# no-op is the correct behavior. Applied to every seed for breadth.
UNSUPPORTED_CHAINS = [
    ("u8-cp850", "UTF-8 read as DOS cp850 (Western Europe)",
     [{"encode": "utf-8"}, {"decode": "cp850"}],
     "cp850 was the default OEM codepage in much of Western Europe; ftfy ships cp437 but NOT cp850, so this is unfixable."),
    ("u8-cp852", "UTF-8 read as DOS cp852 (Central Europe)",
     [{"encode": "utf-8"}, {"decode": "cp852"}],
     "Central-European DOS codepage; not in ftfy's candidate set."),
    ("u8-cp866", "UTF-8 read as DOS cp866 (Cyrillic)",
     [{"encode": "utf-8"}, {"decode": "cp866"}],
     "Russian DOS codepage; not in ftfy's candidate set."),
    ("u8-koi8r", "UTF-8 read as KOI8-R (Cyrillic)",
     [{"encode": "utf-8"}, {"decode": "koi8-r"}],
     "The dominant pre-Windows Russian Unix encoding; not in ftfy's candidate set."),
    ("u8-iso8859-5", "UTF-8 read as ISO-8859-5 (Cyrillic)",
     [{"encode": "utf-8"}, {"decode": "iso-8859-5"}],
     "ISO Latin/Cyrillic; ftfy ships iso-8859-2 but not -5."),
    ("u8-iso8859-7", "UTF-8 read as ISO-8859-7 (Greek)",
     [{"encode": "utf-8"}, {"decode": "iso-8859-7", "errors": "replace"}],
     "ISO Greek; not in ftfy's candidate set (it has windows-1253 but not iso-8859-7)."),
    ("u8-cp874", "UTF-8 read as cp874 (Thai)",
     [{"encode": "utf-8"}, {"decode": "cp874", "errors": "replace"}],
     "Thai Windows codepage; not in ftfy's candidate set."),
    ("u8-1256", "UTF-8 read as Windows-1256 (Arabic)",
     [{"encode": "utf-8"}, {"decode": "cp1256", "errors": "replace"}],
     "Arabic Windows codepage; not in ftfy's candidate set."),
    ("u8-1258", "UTF-8 read as Windows-1258 (Vietnamese)",
     [{"encode": "utf-8"}, {"decode": "cp1258", "errors": "replace"}],
     "Vietnamese Windows codepage; not in ftfy's candidate set."),
    ("u8-utf16le-latin1", "UTF-8 text mistakenly UTF-16-LE-encoded then read as Latin-1",
     [{"encode": "utf-16-le"}, {"decode": "latin-1"}],
     "UTF-16 bytes (with interleaved NULs) read as 8-bit — the ftfy issue #120 pattern; out of scope for both fixers."),
]


def ftfy_repairable(chain) -> bool:
    """True if the chain contains a UTF-8 -> charmap-codec transition that
    ftfy's repair heuristic could in principle reverse."""
    for a, b in zip(chain, chain[1:]):
        if a.get("encode") == "utf-8" and b.get("decode") in FTFY_CHARMAP:
            return True
    return False


def legacy_chains(codec: str):
    """Chains where the seed was first stored in an era-appropriate legacy
    codec, then those bytes were misread. These are *not* UTF-8 mojibake,
    so ftfy cannot fix them (it only reverses UTF-8 mix-ups)."""
    chains = [
        (f"{codec}-as-1252", f"{codec} bytes read as sloppy Windows-1252",
         [{"encode": codec}, {"decode": "sloppy-windows-1252"}],
         f"Text stored in {codec} then displayed as Windows-1252 — legacy-to-legacy mojibake; ftfy only reverses UTF-8 mix-ups, so it cannot fix this."),
        (f"{codec}-as-latin1", f"{codec} bytes read as Latin-1",
         [{"encode": codec}, {"decode": "latin-1"}],
         f"Text stored in {codec} then read as Latin-1; legacy-to-legacy, not UTF-8 mojibake."),
    ]
    if codec in ("windows-1251", "koi8-r"):
        other = "koi8-r" if codec == "windows-1251" else "windows-1251"
        chains.append(
            (f"{codec}-as-{other}", f"{codec} bytes read as {other} (classic krakozyabры)",
             [{"encode": codec}, {"decode": other, "errors": "replace"}],
             f"The iconic Russian KOI8-R/CP1251 swap; legacy-to-legacy, not fixable by ftfy."))
    cjk_cross = {
        "shift_jis": ["euc-jp", "cp437", "gbk"],
        "euc-jp": ["shift_jis"],
        "gbk": ["big5", "shift_jis"],
        "big5": ["gbk", "shift_jis"],
        "euc-kr": ["shift_jis", "gbk"],
    }
    for other in cjk_cross.get(codec, []):
        chains.append(
            (f"{codec}-as-{other}", f"{codec} bytes read as {other} (CJK cross-decode)",
             [{"encode": codec}, {"decode": other, "errors": "replace"}],
             f"CJK text stored in {codec} and decoded as {other}; multi-byte legacy-to-legacy mojibake, far outside ftfy's UTF-8 repair scope."))
    return chains


def seed_provenance(kind: str, lang: str) -> dict:
    source, justification = SEED_PROV[kind]
    if kind == "constructed" and lang in CONSTRUCTED_NOTE:
        justification = CONSTRUCTED_NOTE[lang]
    return {"kind": kind, "source": source, "justification": justification}


def make_entries():
    entries = []
    seen = set()
    for idx, (lang, text, legacy, prov_kind) in enumerate(SEEDS):
        sprov = seed_provenance(prov_kind, lang)
        chains = list(UNIVERSAL_CHAINS) + list(UNSUPPORTED_CHAINS)
        for codec in legacy:
            chains += legacy_chains(codec)
        for suffix, desc, chain, rationale in chains:
            try:
                mojibake = apply_chain(text, chain)
            except ChainError:
                continue
            if mojibake == text or mojibake in seen:
                continue
            seen.add(mojibake)
            output = ftfy.fix_text(mojibake)
            supported = ftfy_repairable(chain)
            entries.append({
                "id": f"gen-{lang}-{idx:02d}-{suffix}",
                "label": f"[{lang}] {desc}",
                "lang": lang,
                "intended": text,
                "mojibake": mojibake,
                "chain": chain,
                "provenance": {
                    "type": "generated",
                    "source": "corpus/tools/generate_corpus.py (a documented "
                              "chain applied to a cited seed)",
                    "seed": sprov,
                    "chain_rationale": rationale,
                    "ftfy_repair_supported": supported,
                    "notes": (
                        "Synthetic mojibake (round-trips by construction). "
                        + ("ftfy's codec set can reverse this in principle."
                           if supported else
                           "Mis-decode uses an encoding OUTSIDE ftfy's repair "
                           "set, so ftfy/plsfix correctly leave it unchanged — "
                           "a predictable no-op, included to test the "
                           "no-false-positives guarantee.")),
                },
                "verified": True,
                "ftfy_output": output,
                "ftfy_recovers": unicodedata.normalize("NFC", text) == output,
            })
    return entries


def main() -> None:
    entries = make_entries()
    OUT.write_text(json.dumps(entries, ensure_ascii=False, indent=2) + "\n",
                   encoding="utf-8")
    fixed = sum(e["ftfy_recovers"] for e in entries)
    supported = sum(e["provenance"]["ftfy_repair_supported"] for e in entries)
    print(f"wrote {len(entries)} entries to {OUT}")
    print(f"  {supported} use an ftfy-supported repair codec; "
          f"{len(entries) - supported} are deliberately unsupported")
    print(f"  ftfy {ftfy.__version__} recovers {fixed}/{len(entries)}")


if __name__ == "__main__":
    main()
