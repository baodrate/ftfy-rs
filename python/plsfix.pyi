from typing import List, Optional, Union

class TextFixerConfig:
    unescape_html: Union[bool, str]
    remove_terminal_escapes: bool
    fix_encoding: bool
    restore_byte_a0: bool
    replace_lossy_sequences: bool
    decode_inconsistent_utf8: bool
    fix_c1_controls: bool
    fix_latin_ligatures: bool
    fix_character_width: bool
    uncurl_quotes: bool
    fix_line_breaks: bool
    fix_surrogates: bool
    remove_control_chars: bool
    normalization: Optional[str]
    max_decode_length: int
    explain: bool
    def __init__(
        self,
        unescape_html: Union[bool, str, None] = "auto",
        remove_terminal_escapes: bool = True,
        fix_encoding: bool = True,
        restore_byte_a0: bool = True,
        replace_lossy_sequences: bool = True,
        decode_inconsistent_utf8: bool = True,
        fix_c1_controls: bool = True,
        fix_latin_ligatures: bool = True,
        fix_character_width: bool = True,
        uncurl_quotes: bool = True,
        fix_line_breaks: bool = True,
        fix_surrogates: bool = True,
        remove_control_chars: bool = True,
        normalization: Optional[str] = "NFC",
        max_decode_length: int = 1_000_000,
        explain: bool = True,
    ) -> None: ...

class ExplanationStep:
    transformation: str

class ExplainedText:
    text: str
    steps: Optional[List[ExplanationStep]]

def fix_text(text: str, config: Optional[TextFixerConfig] = None) -> str: ...
def fix_and_explain(
    text: str, config: Optional[TextFixerConfig] = None
) -> ExplainedText: ...
