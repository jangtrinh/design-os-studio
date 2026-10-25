"""Static scan of Rust UI source for literal geometry (stdlib only).

Used by `check_spacing_relationships.py`. Everything here is a regex scan over
comment-stripped, string-blanked, test-free source, so counts are a stable
census to ratchet against, not a type-checked analysis.

Categories counted per file:
  offset_literals    a numeric literal added to or subtracted from a coordinate
                     expression (`rect.min.x + 8.0`, `y += 24.0`)
  inset_literals     `add_space(N)`, `Margin::same/symmetric(N)`, `.shrink(N)`,
                     `.expand(N)` with a non-zero literal
  radius_literals    a non-zero literal corner radius (`CornerRadius::same(N)`,
                     `cr(N)`, `cr_each` / `CornerRadius { .. }` with literals,
                     or a literal radius argument of `rect_filled/rect_stroke/rect`)
  font_literals      `FontId::proportional/new/monospace(N)`, `.size(N)`,
                     `.size = N`
  nonconcentric_pairs  a rect derived with `.shrink(k)` / `.expand(-k)` or built
                     with `Rect::from_*(.. outer.left() + k ..)` from an outer rect, both painted with a resolvable radius, where
                     `inner != max(outer - k, 0)` and `k < outer`
"""

from __future__ import annotations

import re
from typing import Dict, List, Optional, Tuple

CATEGORIES = (
    "offset_literals",
    "inset_literals",
    "radius_literals",
    "font_literals",
    "nonconcentric_pairs",
)

NUM = r"(\d+(?:\.\d+)?)(?:_?(?:f32|f64|u8|i8|u16|i16|usize))?"
_NUM_FULL = re.compile(r"-?" + NUM + r"(?:\s+as\s+\w+)?")

_OFFSET_RE = re.compile(
    r"(?:\.(?:min|max)\.[xy]|\.(?:left|right|top|bottom)\(\)|\.center\(\)\.[xy]"
    r"|\.center_[xy]\(\)|\b[xy]\b|\.[xy]|\.width\(\)|\.height\(\))\s*([+-])\s*"
    + NUM
    + r"(?![\w.])"
)
_INCR_RE = re.compile(
    r"\b([xy]|cur_[xy]|[a-z_]*_[xy]|[a-z_]*top|[a-z_]*left|cursor)\s*([+-]=)\s*"
    + NUM
    + r"(?![\w.])"
)
_SIZE_FIELD_RE = re.compile(r"\.size\s*=\s*" + NUM + r"(?![\w.])")


def strip_code(src: str) -> str:
    """Blank comments and string/char literal bodies; keep every newline."""
    out: List[str] = []
    i, n = 0, len(src)
    while i < n:
        two = src[i : i + 2]
        c = src[i]
        if two == "//":
            while i < n and src[i] != "\n":
                i += 1
            continue
        if two == "/*":
            depth, i = 1, i + 2
            while i < n and depth:
                if src[i : i + 2] == "/*":
                    depth, i = depth + 1, i + 2
                elif src[i : i + 2] == "*/":
                    depth, i = depth - 1, i + 2
                else:
                    if src[i] == "\n":
                        out.append("\n")
                    i += 1
            continue
        raw = re.match(r'b?r(#*)"', src[i : i + 12])
        if raw and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            end = '"' + raw.group(1)
            j = src.find(end, i + raw.end())
            j = n if j < 0 else j
            out.append('""' + "\n" * src[i : j + len(end)].count("\n"))
            i = j + len(end)
            continue
        if c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append('""' + "\n" * src[i : j + 1].count("\n"))
            i = j + 1
            continue
        if c == "'":
            ch = re.match(r"'(\\.[^']*|[^\\'])'", src[i : i + 12])
            if ch:
                out.append("' '")
                i += ch.end()
                continue
        out.append(c)
        i += 1
    return "".join(out)


def drop_cfg_test(code: str) -> str:
    """Blank every item annotated `#[cfg(test)]`, keeping line numbers."""
    while True:
        m = re.search(r"#\[cfg\(test\)\]", code)
        if not m:
            return code
        depth, j, started = 0, m.end(), False
        while j < len(code):
            ch = code[j]
            if ch == "{":
                depth, started = depth + 1, True
            elif ch == "}":
                depth -= 1
                if started and depth == 0:
                    j += 1
                    break
            elif ch == ";" and not started:
                j += 1
                break
            j += 1
        code = code[: m.start()] + "\n" * code[m.start() : j].count("\n") + code[j:]


def clean(src: str) -> str:
    return drop_cfg_test(strip_code(src))


def balanced_args(s: str, open_idx: int) -> Tuple[List[str], int]:
    """`s[open_idx] == '('`: top-level comma split of the call and its end index."""
    depth, args, cur = 0, [], []
    for i in range(open_idx, len(s)):
        c = s[i]
        if c in "([{":
            depth += 1
            if depth > 1:
                cur.append(c)
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                args.append("".join(cur).strip())
                return args, i
            cur.append(c)
        elif c == "," and depth == 1:
            args.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
    return args, len(s)


def literal_value(expr: str) -> Optional[float]:
    m = _NUM_FULL.fullmatch(expr.strip())
    return float(m.group(1)) if m else None


def _line(code: str, pos: int) -> int:
    return code.count("\n", 0, pos) + 1


def parse_theme_consts(theme_src: str) -> Dict[str, float]:
    """Numeric `pub const NAME: f32|u8|... = N;` values by name."""
    consts: Dict[str, float] = {}
    for m in re.finditer(
        r"pub\s+const\s+([A-Z0-9_]+)\s*:\s*(?:f32|f64|u8|i8|u16|usize)\s*=\s*([^;]+);",
        strip_code(theme_src),
    ):
        v = literal_value(m.group(2))
        if v is not None:
            consts[m.group(1)] = v
    return consts


def resolve_radius(expr: str, consts: Dict[str, float]) -> Optional[float]:
    """Radius value of an expression made of a literal or theme constant."""
    e = expr.strip().replace("egui::", "").replace("theme::", "")
    wrap = re.fullmatch(r"(?:CornerRadius::same|cr)\((.+)\)", e)
    if wrap:
        e = wrap.group(1).strip()
    v = literal_value(e)
    if v is not None:
        return v
    return consts.get(e)


def scan_counts(code: str) -> Dict[str, List[int]]:
    """Line numbers of every literal site, by category (no pair detection)."""
    sites: Dict[str, List[int]] = {c: [] for c in CATEGORIES}
    for ln, text in enumerate(code.split("\n"), 1):
        sites["offset_literals"] += [ln] * len(_OFFSET_RE.findall(text))
        sites["offset_literals"] += [ln] * len(_INCR_RE.findall(text))
        sites["font_literals"] += [ln] * len(_SIZE_FIELD_RE.findall(text))
    for m in re.finditer(r"add_space\(", code):
        a, _ = balanced_args(code, m.end() - 1)
        if a and (literal_value(a[0]) or 0) > 0:
            sites["inset_literals"].append(_line(code, m.start()))
    for m in re.finditer(r"Margin::(?:same|symmetric)\(", code):
        a, _ = balanced_args(code, m.end() - 1)
        for arg in a:
            if (literal_value(arg) or 0) > 0:
                sites["inset_literals"].append(_line(code, m.start()))
    for m in re.finditer(r"\.(?:shrink|expand)\(", code):
        a, _ = balanced_args(code, m.end() - 1)
        if a and (literal_value(a[0].lstrip("-")) or 0) > 0:
            sites["inset_literals"].append(_line(code, m.start()))
    for m in re.finditer(r"CornerRadius::same\(|(?<![A-Za-z_:])cr\(|theme::cr\(", code):
        a, _ = balanced_args(code, code.index("(", m.start()))
        if a and (literal_value(a[0]) or 0) > 0:
            sites["radius_literals"].append(_line(code, m.start()))
    for m in re.finditer(r"cr_each\(", code):
        a, _ = balanced_args(code, m.end() - 1)
        if any((literal_value(x) or 0) > 0 for x in a):
            sites["radius_literals"].append(_line(code, m.start()))
    for m in re.finditer(r"CornerRadius\s*\{([^}]*)\}", code):
        if re.search(r"\b(?:nw|ne|se|sw)\s*:\s*[1-9]", m.group(1)):
            sites["radius_literals"].append(_line(code, m.start()))
    for m in re.finditer(r"\.(?:rect_filled|rect_stroke|rect)\(", code):
        a, _ = balanced_args(code, m.end() - 1)
        if len(a) >= 2 and (literal_value(a[1]) or 0) > 0:
            sites["radius_literals"].append(_line(code, m.start()))
    for m in re.finditer(r"FontId::(?:proportional|new|monospace)\(", code):
        a, _ = balanced_args(code, m.end() - 1)
        if a and literal_value(a[0]) is not None:
            sites["font_literals"].append(_line(code, m.start()))
    for m in re.finditer(r"\.size\(", code):
        a, _ = balanced_args(code, m.end() - 1)
        if a and literal_value(a[0]) is not None:
            sites["font_literals"].append(_line(code, m.start()))
    return sites


def _fn_bodies(code: str) -> List[Tuple[int, str]]:
    """(start offset, body text) of every `fn` body."""
    bodies = []
    for m in re.finditer(r"\bfn\s+\w+", code):
        paren = code.find("(", m.end())
        if paren < 0:
            continue
        _, close = balanced_args(code, paren)
        brace = code.find("{", close)
        semi = code.find(";", close)
        if brace < 0 or (0 <= semi < brace):
            continue
        depth = 0
        for j in range(brace, len(code)):
            if code[j] == "{":
                depth += 1
            elif code[j] == "}":
                depth -= 1
                if depth == 0:
                    bodies.append((brace, code[brace : j + 1]))
                    break
    return bodies


_DERIVE_RE = re.compile(
    r"let\s+(?:mut\s+)?(\w+)\s*=\s*(\w+)\.(shrink|expand)\(\s*(-?)\s*" + NUM + r"\s*\)"
)
_RECT_LET_RE = re.compile(r"let\s+(?:mut\s+)?(\w+)\s*=\s*([^;]*Rect::from_[a-z_]+[^;]*);")
_EDGE_OFFSET_RE = re.compile(
    r"\b(\w+)\s*\.\s*(?:(?:left|right|top|bottom)\(\)|(?:min|max)\.[xy])\s*\+\s*" + NUM + r"(?![\w.])"
)
_INLINE_DERIVE_RE = re.compile(r"(\w+)\.(shrink|expand)\(\s*(-?)\s*" + NUM + r"\s*\)")


def nonconcentric_pair_lines(code: str, consts: Dict[str, float]) -> List[int]:
    """Lines of derived-rect pairs whose radii are not `outer - inset`."""
    lines: List[int] = []
    for start, body in _fn_bodies(code):
        radii: Dict[str, List[float]] = {}
        derived: List[Tuple[str, str, float, int]] = []  # inner, outer, inset, line
        for m in _DERIVE_RE.finditer(body):
            k = float(m.group(5))
            if (m.group(3) == "expand") != (m.group(4) == "-"):
                k = -k  # expand(+k) grows the rect: a negative inset
            derived.append((m.group(1), m.group(2), k, _line(code, start + m.start())))
        for m in _RECT_LET_RE.finditer(body):
            # `Rect::from_min_size(Pos2::new(outer.left() + 8.0, ..))`: the smallest
            # literal added to an edge of one outer rect is taken as the inset.
            per_outer: Dict[str, float] = {}
            for e in _EDGE_OFFSET_RE.finditer(m.group(2)):
                if e.group(1) != m.group(1):
                    per_outer[e.group(1)] = min(per_outer.get(e.group(1), 1e9), float(e.group(2)))
            for outer, k in per_outer.items():
                derived.append((m.group(1), outer, k, _line(code, start + m.start())))
        for m in re.finditer(r"\.(?:rect_filled|rect_stroke|rect)\(", body):
            a, _ = balanced_args(body, m.end() - 1)
            if len(a) < 2:
                continue
            r = resolve_radius(a[1], consts)
            if r is None:
                continue
            inline = _INLINE_DERIVE_RE.fullmatch(a[0].strip())
            if inline:
                k = float(inline.group(4))
                if (inline.group(2) == "expand") != (inline.group(3) == "-"):
                    k = -k
                name = f"{inline.group(1)}.inline@{m.start()}"
                derived.append((name, inline.group(1), k, _line(code, start + m.start())))
                radii.setdefault(name, []).append(r)
            elif re.fullmatch(r"\w+", a[0].strip()):
                radii.setdefault(a[0].strip(), []).append(r)
        for inner, outer, inset, ln in derived:
            if inner not in radii or outer not in radii or inset <= 0:
                continue
            r_outer, r_inner = radii[outer][0], radii[inner][0]
            if inset >= r_outer:
                continue  # a straight inner edge sits clear of the corner arc
            if abs(r_inner - max(r_outer - inset, 0.0)) > 0.5:
                lines.append(ln)
    return lines


def scan_file(src: str, consts: Dict[str, float]) -> Dict[str, List[int]]:
    code = clean(src)
    sites = scan_counts(code)
    sites["nonconcentric_pairs"] = nonconcentric_pair_lines(code, consts)
    return sites
