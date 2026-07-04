#!/usr/bin/env python3
"""Converts I18n/Strings.cs into rust/src/i18n/data.rs.

Parses the uniform `["key"] = "value",` dictionary blocks and string arrays.
Run after any change to Strings.cs on the C# side:
    python rust/tools/convert_strings.py
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "I18n" / "Strings.cs"
DST = ROOT / "rust" / "src" / "i18n" / "data.rs"

LANGS = ["En", "Ar", "Fr", "Tr", "Ur", "Id"]

text = SRC.read_text(encoding="utf-8")


def cs_unescape(s: str) -> str:
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            n = s[i + 1]
            if n == "u" and i + 5 < len(s):
                out.append(chr(int(s[i + 2 : i + 6], 16)))
                i += 6
                continue
            out.append({"n": "\n", "t": "\t", "r": "\r", "0": "\0"}.get(n, n))
            i += 2
        else:
            out.append(c)
            i += 1
    return "".join(out)


def rs_escape(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t")


def block_of(name: str) -> str | None:
    m = re.search(rf"static readonly [^\n]* {re.escape(name)}\b[^{{]*", text)
    if not m:
        return None
    i = text.index("{", m.end() - 1)
    depth, j = 0, i
    while True:
        if text[j] == "{":
            depth += 1
        elif text[j] == "}":
            depth -= 1
            if depth == 0:
                return text[i + 1 : j]
        j += 1


DICT_ENTRY = re.compile(r'\["((?:[^"\\]|\\.)*)"\]\s*=\s*"((?:[^"\\]|\\.)*)"')
STR_LIT = re.compile(r'"((?:[^"\\]|\\.)*)"')


def parse_dict(name: str) -> dict[str, str]:
    b = block_of(name)
    if b is None:
        return {}
    return {cs_unescape(k): cs_unescape(v) for k, v in DICT_ENTRY.findall(b)}


def parse_array(name: str) -> list[str] | None:
    b = block_of(name)
    if b is None:
        return None
    return [cs_unescape(s) for s in STR_LIT.findall(b)]


def keyed_table(rust_name: str, cs_prefix: str) -> tuple[str, int]:
    per_lang = {lang: parse_dict(f"{cs_prefix}{lang}") for lang in LANGS}
    if not per_lang["En"]:
        sys.exit(f"parse failure: {cs_prefix}En is empty")
    keys = sorted(per_lang["En"].keys())
    for lang in LANGS[1:]:
        extra = set(per_lang[lang]) - set(keys)
        if extra:
            sys.exit(f"{cs_prefix}{lang} has keys missing from En: {sorted(extra)}")
    lines = [f"pub static {rust_name}: [(&str, [&str; LANGS]); {len(keys)}] = ["]
    for k in keys:
        vals = ", ".join(f'"{rs_escape(per_lang[lang].get(k, ""))}"' for lang in LANGS)
        lines.append(f'    ("{rs_escape(k)}", [{vals}]),')
    lines.append("];")
    return "\n".join(lines), len(keys)


def array_table(rust_name: str, names_by_lang: dict[str, str | None], length: int) -> str:
    rows = []
    for lang in LANGS:
        cs = names_by_lang.get(lang)
        arr = parse_array(cs) if cs else None
        if cs and arr is None:
            sys.exit(f"parse failure: array {cs} not found")
        if arr is not None and len(arr) != length:
            sys.exit(f"{cs}: expected {length} entries, got {len(arr)}")
        if arr is None:
            rows.append("    None,")
        else:
            vals = ", ".join(f'"{rs_escape(s)}"' for s in arr)
            rows.append(f"    Some([{vals}]),")
    return f"pub static {rust_name}: [Option<[&str; {length}]>; LANGS] = [\n" + "\n".join(rows) + "\n];"


# UI dicts are named _en/_ar/... (lowercase), other tables _eventsEn etc.
def keyed_table_lower(rust_name: str) -> tuple[str, int]:
    per_lang = {lang: parse_dict(f"_{lang.lower()}") for lang in LANGS}
    if not per_lang["En"]:
        sys.exit("parse failure: _en is empty")
    keys = sorted(per_lang["En"].keys())
    lines = [f"pub static {rust_name}: [(&str, [&str; LANGS]); {len(keys)}] = ["]
    for k in keys:
        vals = ", ".join(f'"{rs_escape(per_lang[lang].get(k, ""))}"' for lang in LANGS)
        lines.append(f'    ("{rs_escape(k)}", [{vals}]),')
    lines.append("];")
    return "\n".join(lines), len(keys)


ui, n_ui = keyed_table_lower("UI")
prayers, n_pr = keyed_table("PRAYERS", "_prayer")
events, n_ev = keyed_table("EVENTS", "_events")

hijri_rows = []
for lang in LANGS:
    arr = parse_array(f"_hijri{lang}")
    if arr is None or len(arr) != 12:
        sys.exit(f"_hijri{lang}: bad or missing (need 12 months)")
    vals = ", ".join(f'"{rs_escape(s)}"' for s in arr)
    hijri_rows.append(f"    [{vals}],")
hijri = "pub static HIJRI_MONTHS: [[&str; 12]; LANGS] = [\n" + "\n".join(hijri_rows) + "\n];"

weekdays = array_table(
    "WEEKDAYS",
    {"En": None, "Ar": "_arWeekdays", "Fr": "_frWeekdays", "Tr": "_trWeekdays", "Ur": "_urWeekdays", "Id": "_idWeekdays"},
    7,
)
months = array_table(
    "MONTHS",
    {"En": None, "Ar": "_arMonths", "Fr": "_frMonths", "Tr": "_trMonths", "Ur": "_urMonths", "Id": "_idMonths"},
    12,
)

out = f"""// GENERATED from I18n/Strings.cs by rust/tools/convert_strings.py — do not hand-edit.
// Lang order: En Ar Fr Tr Ur Id. Empty slot = fall back to English at runtime.

pub const LANGS: usize = {len(LANGS)};

{ui}

{prayers}

{events}

{hijri}

{weekdays}

{months}
"""
DST.parent.mkdir(parents=True, exist_ok=True)
DST.write_text(out, encoding="utf-8", newline="\n")
print(f"wrote {DST}: UI={n_ui} PRAYERS={n_pr} EVENTS={n_ev} keys")
