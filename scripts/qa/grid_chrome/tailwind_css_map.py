"""Tailwind utility class -> CSS declaration mapping for grid Chrome harness."""

from html.parser import HTMLParser
import re
from typing import Dict, List, Optional, Tuple

VOID_TAGS = frozenset(
    {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}
)


class TailwindChildExtractor(HTMLParser):
    """Extracts the root container's class attribute and direct child elements' class attributes."""

    def __init__(self) -> None:
        super().__init__()
        self.depth = 0
        self.root_class: Optional[str] = None
        self.child_classes: List[str] = []

    def handle_starttag(self, tag: str, attrs: List[Tuple[str, Optional[str]]]) -> None:
        cls = dict(attrs).get("class", "") or ""
        if self.depth == 0:
            self.root_class = cls
        elif self.depth == 1:
            self.child_classes.append(cls)
        if tag.lower() not in VOID_TAGS:
            self.depth += 1

    def handle_startendtag(self, tag: str, attrs: List[Tuple[str, Optional[str]]]) -> None:
        cls = dict(attrs).get("class", "") or ""
        if self.depth == 0:
            self.root_class = cls
        elif self.depth == 1:
            self.child_classes.append(cls)

    def handle_endtag(self, tag: str) -> None:
        if tag.lower() not in VOID_TAGS and self.depth > 0:
            self.depth -= 1


def _arbitrary_track_expr(raw: str) -> str:
    out: List[str] = []
    depth = 0
    for ch in raw:
        if ch == "(":
            depth += 1
            out.append(ch)
        elif ch == ")":
            depth = max(0, depth - 1)
            out.append(ch)
        elif ch == "_" and depth == 0:
            out.append(" ")
        elif ch == "," and depth > 0:
            out.append(", ")
        else:
            out.append(ch)
    return "".join(out)


def _scale_px(token_val: str) -> Optional[str]:
    if token_val.startswith("[") and token_val.endswith("]"):
        return token_val[1:-1]
    try:
        n = float(token_val)
    except ValueError:
        return None
    px = n * 4.0
    return f"{int(px)}px" if px.is_integer() else f"{px:g}px"


_SIMPLE_TOKENS: Dict[str, List[str]] = {
    "grid": ["display: grid;"],
    "flex": ["display: flex;"],
    "flex-col": ["flex-direction: column;"],
    "box-border": ["box-sizing: border-box;"],
    "border": ["border-width: 1px;", "border-style: solid;"],
    "border-dashed": ["border-style: dashed;"],
    "content-start": ["align-content: start;"],
    "items-start": ["align-items: flex-start;"],
    "items-center": ["align-items: center;"],
    "justify-self-start": ["justify-self: start;"],
    "self-start": ["align-self: start;"],
    "self-stretch": ["align-self: stretch;"],
}

_SCALE_PREFIXES: List[Tuple[str, Tuple[str, ...]]] = [
    ("gap-x-", ("column-gap",)),
    ("gap-y-", ("row-gap",)),
    ("gap-", ("gap",)),
    ("px-", ("padding-left", "padding-right")),
    ("py-", ("padding-top", "padding-bottom")),
    ("pt-", ("padding-top",)),
    ("pr-", ("padding-right",)),
    ("pb-", ("padding-bottom",)),
    ("pl-", ("padding-left",)),
    ("p-", ("padding",)),
    ("w-", ("width",)),
    ("h-", ("height",)),
    ("min-w-", ("min-width",)),
    ("max-w-", ("max-width",)),
    ("min-h-", ("min-height",)),
    ("max-h-", ("max-height",)),
]


def tailwind_token_to_decls(tok: str) -> List[str]:
    """Maps a single Tailwind utility token emitted by `tailwind_for` to CSS declarations."""
    if tok in _SIMPLE_TOKENS:
        return list(_SIMPLE_TOKENS[tok])

    m = re.fullmatch(r"border-(\d+)", tok)
    if m:
        return [f"border-width: {m.group(1)}px;", "border-style: solid;"]
    m = re.fullmatch(r"border-\[([0-9.]+px)\]", tok)
    if m:
        return [f"border-width: {m.group(1)};", "border-style: solid;"]
    m = re.fullmatch(r"border-\[(#[0-9A-Fa-f]+|rgba?\([^)]+\))\]", tok)
    if m:
        return [f"border-color: {m.group(1)};"]

    m = re.fullmatch(r"grid-cols-(\d+)", tok)
    if m:
        return [f"grid-template-columns: repeat({m.group(1)}, minmax(0, 1fr));"]
    m = re.fullmatch(r"grid-cols-\[(.+)\]", tok)
    if m:
        return [f"grid-template-columns: {_arbitrary_track_expr(m.group(1))};"]
    m = re.fullmatch(r"grid-rows-(\d+)", tok)
    if m:
        return [f"grid-template-rows: repeat({m.group(1)}, minmax(0, 1fr));"]
    m = re.fullmatch(r"grid-rows-\[(.+)\]", tok)
    if m:
        return [f"grid-template-rows: {_arbitrary_track_expr(m.group(1))};"]

    for prefix, css_prop in (("col", "grid-column"), ("row", "grid-row")):
        m = re.fullmatch(rf"{prefix}-start-(\d+)", tok)
        if m:
            return [f"{css_prop}-start: {m.group(1)};"]
        m = re.fullmatch(rf"{prefix}-start-\[(\d+)\]", tok)
        if m:
            return [f"{css_prop}-start: {m.group(1)};"]
        m = re.fullmatch(rf"{prefix}-span-(\d+)", tok)
        if m:
            n = m.group(1)
            return [f"{css_prop}-end: span {n};"]
        m = re.fullmatch(rf"{prefix}-span-\[span_(\d+)_/_span_\d+\]", tok)
        if m:
            return [f"{css_prop}-end: span {m.group(1)};"]

    for prefix, props in _SCALE_PREFIXES:
        if tok.startswith(prefix):
            px = _scale_px(tok[len(prefix) :])
            if px is not None:
                return [f"{prop}: {px};" for prop in props]

    raise ValueError(f"Unsupported Tailwind token in grid harness: {tok!r}")


def tailwind_classes_to_css(class_str: str) -> str:
    decls: List[str] = []
    for tok in class_str.split():
        decls.extend(tailwind_token_to_decls(tok))
    return " ".join(decls)


def parse_tailwind_html(html: str, expected_children: Optional[int] = None) -> Tuple[str, List[str]]:
    parser = TailwindChildExtractor()
    parser.feed(html)
    if parser.root_class is None:
        raise ValueError(f"Failed to parse root container from Tailwind HTML: {html!r}")
    if expected_children is not None and len(parser.child_classes) != expected_children:
        raise ValueError(
            f"Tailwind HTML child count mismatch: got {len(parser.child_classes)}, expected {expected_children}"
        )
    grid_decls = tailwind_classes_to_css(parser.root_class)
    item_decls = [tailwind_classes_to_css(c) for c in parser.child_classes]
    return grid_decls, item_decls
