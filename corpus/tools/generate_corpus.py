#!/usr/bin/env python3
"""Generate the mechanically-constructed part of the mojibake corpus.

Every entry produced here is mojibake *by construction*: a seed sentence
(written correctly, in its real orthography) is pushed through an
explicit corruption chain. Provenance is perfect and hallucination risk
is zero, which makes this set the backbone of the corpus; the
found-in-the-wild sets supply realism on top.

Output: corpus/entries/generated-verified.json (deterministic; rerun
after editing seeds/chains, then rerun verify_corpus.py --write).
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

# Seed sentences: realistic orthography, heavy non-ASCII usage.
# (lang, text, legacy codecs this language was commonly stored in)
SEEDS = [
    ("fr", "Le cœur déçu mais l'âme plutôt naïve, Louÿs rêva de crapaüter en canoë au delà des îles, près du mälström où brûlent les novæ.", ["windows-1252", "macroman", "cp437", "cp850"]),
    ("fr", "à perturber la réflexion de l'humanité", ["windows-1252"]),
    ("de", "Falsches Üben von Xylophonmusik quält jeden größeren Zwerg. Die heiße Zypernsonne quälte Max und Victoria ja böse auf dem Weg bis zur Küste.", ["windows-1252", "cp437", "cp850", "macroman"]),
    ("de", "„Handwerk bringt dich überall hin“: Von der YOU bis nach Monaco", ["windows-1252"]),
    ("es", "El pingüino Wenceslao hizo kilómetros bajo exhaustiva lluvia y frío, añoraba a su querido cachorro.", ["windows-1252", "cp850"]),
    ("pt", "À noite, vovô Kowalsky vê o ímã cair no pé do pinguim queixoso e vovó põe açúcar no chá de tâmaras do jabuti feliz.", ["windows-1252"]),
    ("it", "Ma la volpe, col suo balzo, ha raggiunto il quieto Fido. Perché l'ascensore non funziona più?", ["windows-1252"]),
    ("ro", "Muzicologă în bej vând whisky și tequila, preț fix. Înțelegerea științifică", ["windows-1250", "iso-8859-2"]),
    ("hu", "Árvíztűrő tükörfúrógép. Jó foxim és don Quijote húszwattos lámpánál ülve egy pár bűvös cipőt készít.", ["windows-1250", "iso-8859-2", "cp852"]),
    ("cs", "Příliš žluťoučký kůň úpěl ďábelské ódy. Zvlášť zákeřný učeň s ďolíčky běží podél zóny úlů.", ["windows-1250", "iso-8859-2", "cp852"]),
    ("pl", "Zażółć gęślą jaźń. Pchnąć w tę łódź jeża lub ośm skrzyń fig.", ["windows-1250", "iso-8859-2"]),
    ("sk", "Kŕdeľ šťastných ďatľov učí pri ústí Váhu mĺkveho koňa obhrýzať kôru a žrať čerstvé mäso.", ["windows-1250"]),
    ("hr", "Gojazni đačić s biciklom drži hmelj i finu vatu u džepu nošnje.", ["windows-1250"]),
    ("tr", "Pijamalı hasta yağız şoföre çabucak güvendi. Öğrencilerimiz İstanbul'da.", ["windows-1254", "iso-8859-9"]),
    ("sv", "Flygande bäckasiner söka hwila på mjuka tuvor. Räksmörgås med surströmming.", ["windows-1252", "macroman", "cp437"]),
    ("no", "Vår sære Zulu fra badeøya spilte jo whist og quickstep i min taxi. Blåbærsyltetøy på smørbrød.", ["windows-1252", "cp865"]),
    ("da", "Quizdeltagerne spiste jordbær med fløde, mens cirkusklovnen Wolther spillede på xylofon. Høj bly gom vandt fræk sexquiz på wc.", ["windows-1252", "cp865"]),
    ("fi", "Törkylempijävongahdus. On sangen hauskaa, että polkupyörä on maanteiden jokapäiväinen ilmiö.", ["windows-1252"]),
    ("is", "Kæmi ný öxi hér, ykist þjófum nú bæði víl og ádrepa. Sævör grét áðan því úlpan var ónýt.", ["windows-1252", "cp861"]),
    ("et", "See väike mölder jõuab rongile hüpata. Õunad ja ürdid künkal.", ["windows-1257"]),
    ("lt", "Įlinkdama fechtuotojo špaga sublykčiojusi pragręžė apvalų arbūzą. Žalgirio mūšis įvyko 1410 metais.", ["windows-1257"]),
    ("lv", "Glāžšķūņa rūķīši dzērumā čiepj Baha koncertflīģeļu vākus. Sarkanās jāņogas garšo labāk.", ["windows-1257"]),
    ("ru", "Съешь же ещё этих мягких французских булок да выпей чаю. В чащах юга жил бы цитрус? Да, но фальшивый экземпляр!", ["windows-1251", "koi8-r", "iso-8859-5", "cp866"]),
    ("ru", "дороге Из-под #футбол", ["windows-1251", "koi8-r"]),
    ("uk", "Чуєш їх, доцю, га? Кумедна ж ти, прощайся без ґольфів! Жебракують філософи при ґанку церкви.", ["windows-1251", "koi8-u"]),
    ("bg", "Жълтата дюля беше щастлива, че пухът, който цъфна, замръзна като гьон. За миг бях в чужд плюшен скърцащ фотьойл.", ["windows-1251"]),
    ("sr", "Љубазни фењерџија чађавог лица хоће да ми покаже штос. Ниш и Ћуприја су градови у Србији.", ["windows-1251"]),
    ("el", "Ξεσκεπάζω την ψυχοφθόρα βδελυγμία. Θέλει αρετή και τόλμη η ελευθερία. Καλημέρα, κόσμε!", ["windows-1253", "iso-8859-7"]),
    ("he", "דג סקרן שט בים מאוכזב ולפתע מצא חברה. שלום עולם, מה שלומך היום?", ["windows-1255", "iso-8859-8"]),
    ("ar", "نص حكيم له سر قاطع وذو شأن عظيم مكتوب على ثوب أخضر ومغلف بجلد أزرق. مرحبا بالعالم!", ["windows-1256"]),
    ("fa", "بر اساس قانون اساسی، زبان رسمی ایران فارسی است. کیمیاگری با طلا و نقره.", ["windows-1256"]),
    ("th", "เป็นมนุษย์สุดประเสริฐเลิศคุณค่า กว่าบรรดาฝูงสัตว์เดรัจฉาน จงฝ่าฟันพัฒนาวิชาการ", ["cp874"]),
    ("vi", "Tôi yêu tiếng nước tôi từ khi mới ra đời. Đêm qua trời mưa to quá, đường phố ngập hết cả.", ["windows-1258"]),
    ("ja", "いろはにほへと ちりぬるを 色は匂へど 散りぬるを。文字化けテストです。", ["shift_jis", "euc-jp"]),
    ("ja", "ハンカクカナモジ ｱｲｳｴｵ と全角の混在テキスト", ["shift_jis"]),
    ("zh-Hans", "我能吞下玻璃而不伤身体。汉字乱码测试:中文编码很复杂。", ["gbk"]),
    ("zh-Hant", "我能吞下玻璃而不傷身體。繁體中文亂碼測試:編碼問題很常見。", ["big5"]),
    ("ko", "키스의 고유조건은 입술끼리 만나야 하고 특별한 기술은 필요치 않다. 다람쥐 헌 쳇바퀴에 타고파.", ["euc-kr"]),
    ("hi", "ऋषियों को सताने वाले दुष्ट राक्षसों के राजा रावण का सर्वनाश करने वाले विष्णुवतार भगवान श्रीराम", []),
    ("ka", "ვეპხის ტყაოსანი შოთა რუსთაველი. ღმერთსი შემვედრე, ნუთუ კვლა დამხსნას სოფლისა შრომასა.", []),
    ("emoji", "I'm so happy 😂👍🏽 family: 👨‍👩‍👧‍👦 flag: 🇺🇳 star: ⭐ heart: ❤️", []),
    ("emoji", "(ง'⌣')ง fight! ¯\\_(ツ)_/¯ ☺💕", []),
    ("mixed", "Naïve café-goers paid €5 — “très chic” — for ½ a crème brûlée…", ["windows-1252", "macroman"]),
    ("symbols", "∮ E·da = Q, n → ∞, ∑ f(i) = ∏ g(i); ⊥ ≠ ∥ — α β γ δ", []),
]

# Universal chains: applicable to any seed (UTF-8 always encodes).
# (suffix, description, chain)
UNIVERSAL_CHAINS = [
    ("u8-latin1", "UTF-8 bytes read as Latin-1",
     [{"encode": "utf-8"}, {"decode": "latin-1"}]),
    ("u8-1252", "UTF-8 bytes read as (sloppy) Windows-1252",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"}]),
    ("u8-1251", "UTF-8 bytes read as (sloppy) Windows-1251",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1251"}]),
    ("u8-mac", "UTF-8 bytes read as MacRoman",
     [{"encode": "utf-8"}, {"decode": "macroman"}]),
    ("u8-437", "UTF-8 bytes read as DOS codepage 437",
     [{"encode": "utf-8"}, {"decode": "cp437"}]),
    ("u8-850", "UTF-8 bytes read as DOS codepage 850",
     [{"encode": "utf-8"}, {"decode": "cp850"}]),
    ("u8-1252-x2", "double mojibake: UTF-8/Windows-1252 twice",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"encode": "utf-8"}, {"decode": "sloppy-windows-1252"}]),
    ("u8-1252-x3", "triple mojibake: UTF-8/Windows-1252 three times",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"encode": "utf-8"}, {"decode": "sloppy-windows-1252"}]),
    ("u8-1252-latin1-mix", "UTF-8/Windows-1252, re-encoded UTF-8, read as Latin-1",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"encode": "utf-8"}, {"decode": "latin-1"}]),
    ("u8-1252-nbsp", "UTF-8/Windows-1252 with NBSP flattened to space",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"transform": "nbsp_to_space"}]),
    ("u8-1252-nbsp-gone", "UTF-8/Windows-1252 with NBSP deleted",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"transform": "nbsp_to_nothing"}]),
    ("u8-1252-curl", "UTF-8/Windows-1252 then smart-quoted by a CMS",
     [{"encode": "utf-8"}, {"decode": "sloppy-windows-1252"},
      {"transform": "curl_apostrophe"}]),
    ("u8-latin1-entities", "UTF-8/Latin-1 then HTML-escaped with named entities",
     [{"encode": "utf-8"}, {"decode": "latin-1"},
      {"transform": "html_named"}]),
    ("cesu8-1252", "CESU-8 (Java-style surrogates) read as sloppy Windows-1252",
     [{"encode": "cesu-8"}, {"decode": "sloppy-windows-1252"}]),
    ("cesu8-latin1", "CESU-8 (Java-style surrogates) read as Latin-1",
     [{"encode": "cesu-8"}, {"decode": "latin-1"}]),
    ("u8-lossy-ascii", "UTF-8 read as ASCII with replacement characters",
     [{"encode": "utf-8"}, {"decode": "ascii", "errors": "replace"}]),
]

# Legacy chains: seed is first stored in its era-appropriate codec, then
# those bytes are misread as something else.
def legacy_chains(codec: str) -> list[tuple[str, str, list[dict]]]:
    chains = [
        (f"{codec}-as-1252", f"{codec} bytes read as sloppy Windows-1252",
         [{"encode": codec}, {"decode": "sloppy-windows-1252"}]),
        (f"{codec}-as-latin1", f"{codec} bytes read as Latin-1",
         [{"encode": codec}, {"decode": "latin-1"}]),
        (f"{codec}-u8-roundtrip", f"{codec}-misread re-encoded: utf-8 -> {codec} misread as utf-8? no — {codec} read as sloppy-1252, common web display bug",
         [{"encode": codec}, {"decode": "sloppy-windows-1252"},
          {"transform": "nbsp_to_space"}]),
    ]
    if codec in ("windows-1251", "koi8-r"):
        other = "koi8-r" if codec == "windows-1251" else "windows-1251"
        chains.append(
            (f"{codec}-as-{other}", f"{codec} bytes read as {other} (classic krakozyabry)",
             [{"encode": codec}, {"decode": other, "errors": "replace"}]))
    return chains


def make_entries() -> list[dict]:
    entries = []
    seen_mojibake = set()
    for lang_idx, (lang, text, legacy) in enumerate(SEEDS):
        all_chains = list(UNIVERSAL_CHAINS)
        for codec in legacy:
            all_chains += legacy_chains(codec)
        for suffix, desc, chain in all_chains:
            try:
                mojibake = apply_chain(text, chain)
            except ChainError:
                continue  # seed not encodable in this codec; skip
            if mojibake == text or mojibake in seen_mojibake:
                continue
            seen_mojibake.add(mojibake)
            output = ftfy.fix_text(mojibake)
            entries.append({
                "id": f"gen-{lang}-{lang_idx:02d}-{suffix}",
                "label": f"[{lang}] {desc}",
                "lang": lang,
                "intended": text,
                "mojibake": mojibake,
                "chain": chain,
                "provenance": {
                    "type": "generated",
                    "source": "corpus/tools/generate_corpus.py",
                    "notes": "mechanically constructed; zero hallucination risk",
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
    print(f"wrote {len(entries)} entries to {OUT}")
    print(f"ftfy {ftfy.__version__} recovers {fixed}/{len(entries)}")


if __name__ == "__main__":
    main()
