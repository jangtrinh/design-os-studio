"""Read every `pub const` of `crates/designos-app/src/theme.rs` and resolve it to a plain value.

Shared by the generator that draws `design/studio-ui.designos` and by the gate `check_studio_ui_file.py`. Resolution is
strict: a constant whose right-hand side `studio_ui_rust_expr` cannot evaluate raises `ThemeError` listing every failure. So
does an indented or nested `pub const` (not read) and a `u32` that is neither a colour nor skipped. A `pub(crate) const` is read.

Values: colour -> {"r","g","b","a"} (0..255, straight alpha); number -> int or float; text -> str; array -> list;
tuple -> tuple. `rgba(c, a)` is stored straight; `from_rgba_premultiplied` is un-premultiplied and rounded.
"""
from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

from studio_ui_rust_expr import Evaluator, ThemeError
from studio_ui_theme_policy import SKIPPED

__all__ = ["THEME_REL", "ThemeError", "SKIPPED", "Const", "read_theme_constants", "is_colour", "colour_of", "colour_hex", "flatten",
           "drawable_rows"]
THEME_REL = "crates/designos-app/src/theme.rs"
_COLOUR_DOC = re.compile(r"0xRRGGBB|colou?r", re.I)
_COLOUR_NAME = re.compile(r"(?:^|_)(?:BG|FILL|STROKE|PALETTE|INK|COLOU?R)(?:_|$)")


@dataclass
class Const:
    name: str
    rust_type: str
    expr: str
    line: int
    doc: str = ""
    value: Any = None
    resolved: bool = False


def _strip_comments(src: str) -> str:
    """Blank `//` and `/* */` comments (string-aware), keeping newlines so line numbers hold."""
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        if c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append(src[i:j + 1])
            i = j + 1
        elif src.startswith("//", i):
            while i < n and src[i] != "\n":
                i += 1
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append("\n" * src.count("\n", i, j))
            i = j
        else:
            out.append(c)
            i += 1
    return "".join(out)


def _doc_above(raw_lines: List[str], line: int) -> str:
    """The `///` and `//` comment block directly above a constant (attributes skipped)."""
    i, doc = line - 2, []
    while i >= 0 and raw_lines[i].lstrip().startswith(("#[", "//")):
        if raw_lines[i].lstrip().startswith("//"):
            doc.append(raw_lines[i].lstrip().lstrip("/").strip())
        i -= 1
    return " ".join(reversed(doc))


def _scan_const_blocks(code: str, raw: str) -> List[Const]:
    nested = re.findall(r"^[ \t]+pub(?:\([a-z]+\))?\s+const\s+([A-Z][A-Z0-9_]*)\s*:", code, re.M)
    if nested:
        raise ThemeError(f"pub const inside a block or indented, not read: {nested}")
    consts, raw_lines = [], raw.split("\n")
    for m in re.finditer(r"^pub(?:\([a-z]+\))?\s+const\s+([A-Z][A-Z0-9_]*)\s*:", code, re.M):
        i, depth = m.end(), 0
        while i < len(code) and not (code[i] == "=" and depth == 0):
            depth += code[i] in "([" and 1 or code[i] in ")]" and -1 or 0
            i += 1
        j, depth = i + 1, 0
        while j < len(code) and not (code[j] == ";" and depth == 0):
            if code[j] == '"':
                j = code.index('"', j + 1)
            depth += code[j] in "([" and 1 or code[j] in ")]" and -1 or 0
            j += 1
        line = code.count("\n", 0, m.start()) + 1
        consts.append(Const(m.group(1), code[m.end():i].strip(), code[i + 1:j].strip(), line, _doc_above(raw_lines, line)))
    return consts


def is_colour(c: Const) -> bool:
    """Color32 constants and arrays; a u32 (or u32 array) only when its doc comment or name says it is a colour."""
    if "Color32" in c.rust_type:
        return True
    return "u32" in c.rust_type and bool(_COLOUR_DOC.search(c.doc) or _COLOUR_NAME.search(c.name))


def read_theme_constants(src: str) -> Dict[str, Const]:
    """Every `pub const` of theme.rs resolved to a value; raises ThemeError listing all failures."""
    consts = {c.name: c for c in _scan_const_blocks(_strip_comments(src), src)}
    ev, failures = Evaluator(consts), []
    for name, c in consts.items():
        try:
            ev.stack = []
            ev.const(name)
            if "u32" in c.rust_type and name not in SKIPPED and not is_colour(c):
                raise ThemeError("u32 that is neither a colour (doc comment or name) nor in SKIPPED")
        except (ThemeError, ValueError, TypeError, ZeroDivisionError) as e:
            failures.append(f"{name} (line {c.line}): {e} in `{c.expr[:60]}`")
    if failures:
        raise ThemeError("unresolved theme constants:\n  " + "\n  ".join(failures))
    return consts


def colour_of(v: Any) -> Dict[str, int]:
    """Colour dict for a resolved colour constant or a raw 0xRRGGBB u32."""
    if isinstance(v, dict):
        return v
    n = int(v)
    return {"r": n >> 16 & 255, "g": n >> 8 & 255, "b": n & 255, "a": 255}


def colour_hex(c: Dict[str, int]) -> str:
    base = "#{r:02X}{g:02X}{b:02X}".format(**c)
    return base if c["a"] == 255 else base + f"{c['a']:02X}"


def flatten(consts: Dict[str, Const]) -> List[dict]:
    """One row per scalar: arrays become `NAME[i]` (0-based). Row: name, const, index, rust_type, kind, value, line."""
    rows: List[dict] = []
    for c in consts.values():
        compound = isinstance(c.value, (list, tuple))
        items = [(f"{c.name}[{i}]", i, v) for i, v in enumerate(c.value)] if compound else [(c.name, None, c.value)]
        for name, idx, v in items:
            if is_colour(c) and not isinstance(v, str):
                kind, val = "colour", colour_of(v)
            elif isinstance(v, str):
                kind, val = "text", v
            else:
                kind, val = "number", float(v)
            rows.append({"name": name, "const": c.name, "index": idx, "rust_type": c.rust_type, "kind": kind, "value": val,
                         "line": c.line, "compound": compound})
    return rows


def drawable_rows(rows: List[dict], skipped: Optional[Dict[str, str]] = None) -> List[dict]:
    skipped = SKIPPED if skipped is None else skipped
    return [r for r in rows if r["name"] not in skipped and r["const"] not in skipped]
