#!/usr/bin/env python3
"""
UX-writing label scanner used by `scripts/zero_fake_ui_gate.py` invariant Z16.

Enforces the mechanical part of `design/ux-writing.md`:
  - rule 1: a visible action label (button, menu item) is at most 3 words;
  - rule 3: a count never sits in parentheses inside a label.

Message length (rule 5) and everything semantic stay review-only.

Studio paints most of its chrome by hand, so the scanner does not look for one egui
pattern. It reads three things, all from shipped code (comments and `#[cfg(test)]` items
stripped, literals normalized) as produced by the gate's `SourceSet`:
  1. label call sites (`LABEL_CALLS`): the helper that takes the visible label and the
     argument position of that label;
  2. a label bound to a `let` earlier in the same function and passed by name to such a helper;
  3. label-returning fns (`LABEL_FNS`, e.g. `fix_label`): every literal in the body.
Anything outside those tables (data tables such as the `(id, label, title, prompt)` tuples in
`terminal.rs`, a new helper not yet registered) is not read; register the helper in the same
change that introduces it.

Ratchet: `scripts/ux_writing_baseline.json` lists the violations that existed when the
standard was adopted (file, kind, literal, count; no line numbers). A listed violation
passes, a new one fails, and a listed one that disappeared fails with "fixed, remove from
baseline". The writer refuses to add or raise an entry (`growth_offenders`), so the list only
shrinks; a label moved to another file is accepted as a move.
"""

from __future__ import annotations

import json
import re
from collections import Counter
from pathlib import Path
from typing import Callable, Dict, Iterator, List, Optional, Tuple

BASELINE_PATH = Path(__file__).with_name("ux_writing_baseline.json")
MAX_ACTION_WORDS = 3
WORDS_KIND = "words"
COUNT_KIND = "count-in-parens"
COUNT_LABEL_KIND = "count-in-label"

# Exact product / app names count as one word (rule 1).
PRODUCT_NAMES = ("DESIGN:OS Studio", "Claude Code", "Google Fonts")

# Files that build document content (demo kits, prototypes) or tests, not app chrome.
SKIPPED_PREFIXES = ("showcase/", "shell/tests", "tests/")
SKIPPED_FILES = ("protovars.rs", "tests.rs")

# Label call sites: (regex matching the call head, path prefix filter or "", label argument index).
# A filter restricts a generic name such as `button` to the modules whose `button` takes a label there.
LABEL_CALLS: List[Tuple[str, str, int]] = [
    (r"(?<![\w.:])MenuItem::new", "", 0),
    (r"(?<![\w.:])leaf", "panels/menus.rs", 0),
    (r"(?<![\w.:])submenu", "panels/menus.rs", 0),
    (r"(?<![\w.:])stub_sub", "panels/menus.rs", 0),
    (r"(?<![\w.])text_button", "panels/vertex.rs", 3),
    (r"(?<![\w.])text_button", "find/panel.rs", 3),
    (r"(?<![\w.])primary_button", "dev/sections.rs", 3),
    (r"(?<![\w.])small_button", "panels/", 3),
    (r"(?<![\w.])slide_button", "present.rs", 3),
    (r"(?<![\w.])(?:ui::|buttons::|modal::)?button", "comments/", 3),
    (r"(?<![\w.])(?:ui::|buttons::|modal::)?button", "plugins/", 3),
    (r"(?<![\w.])(?:ui::|buttons::|modal::)?button", "libraries/", 3),
    (r"(?<![\w.])button", "home/", 4),
    (r"(?<![\w.])paint_card_button", "agent_approval_card.rs", 6),
    (r"(?<![\w.])paint_icon_btn", "agent_header.rs", 6),
    (r"(?<![\w.])paint_icon_button", "agent_composer_toolbar.rs", 5),
    (r"(?<![\w.])paint_text_button", "agent_composer_toolbar.rs", 5),
    (r"(?<![\w.])menu_row", "agent_composer_menus.rs", 3),
    (r"(?<![\w.])add_row", "panels/proto_actions.rs", 3),
    (r"(?<![\w.])add_row_indented", "panels/proto_actions.rs", 3),
]

# Label-returning fns: (fn name, path prefix). Every literal in the body is a visible label.
LABEL_FNS: List[Tuple[str, str]] = [
    ("fix_label", "agent_"),
    ("action_label", "agent_"),
    ("button_label", "agent_"),
    ("button_label", "terminal.rs"),
    ("mode_label", "agent_composer_toolbar.rs"),
]

# Row headings: only rule 3 applies (they are not actions), and only when the argument formats a `.len()` count.
COUNT_ONLY_CALLS: List[Tuple[str, str, int]] = [
    (r"(?<![\w.])draw_kv", "shell.rs", 1),
]

CALL_TABLE = [(h, f, i, False) for h, f, i in LABEL_CALLS] + [(h, f, i, True) for h, f, i in COUNT_ONLY_CALLS]

PLACEHOLDER = re.compile(r"\{[^{}]*\}")
COUNT_IN_PARENS = re.compile(r"\(\{.*\}\)|\(\d+\)")
BARE_IDENT = re.compile(r"^&?([A-Za-z_]\w*)$")
DIGIT_TOKEN = re.compile(r"(?<![\w.])\d+(?![\w.%])")


def _literal_end(src: str, i: int) -> int:
    """Exclusive end of the double-quoted literal starting at `src[i]`, else 0 (normalized code only has plain ones)."""
    if src[i] != '"':
        return 0
    j, n = i + 1, len(src)
    while j < n:
        if src[j] == "\\":
            j += 2
        elif src[j] == '"':
            return j + 1
        else:
            j += 1
    return n


def _skip_char_literal(src: str, i: int) -> int:
    """If `src[i]` opens a char literal like `'('`, return its exclusive end, else 0."""
    n = len(src)
    if i + 2 < n and src[i + 1] != "\\" and src[i + 2] == "'":
        return i + 3
    if i + 3 < n and src[i + 1] == "\\":
        close = src.find("'", i + 3)
        return close + 1 if close != -1 else 0
    return 0


def split_call_args(src: str, open_idx: int) -> List[str]:
    """Top-level arguments of the call whose `(` is at `open_idx`; string and char literals are opaque."""
    args: List[str] = []
    depth, start, i, n = 0, open_idx + 1, open_idx, len(src)
    while i < n:
        ch = src[i]
        if ch == '"':
            i = _literal_end(src, i)
            continue
        if ch == "'":
            end = _skip_char_literal(src, i)
            if end:
                i = end
                continue
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
            if depth == 0:
                tail = src[start:i].strip()
                if tail:
                    args.append(tail)
                return args
        elif ch == "," and depth == 1:
            args.append(src[start:i].strip())
            start = i + 1
        i += 1
    return args


def literals_in(arg: str) -> List[str]:
    """Decoded string literals inside one argument expression, in source order."""
    found: List[str] = []
    i, n = 0, len(arg)
    while i < n:
        if arg[i] == '"':
            end = _literal_end(arg, i)
            body = arg[i + 1 : end - 1]
            found.append(body.replace('\\"', '"').replace("\\\\", "\\"))
            i = end
        else:
            i += 1
    return found


def word_count(label: str) -> int:
    """Whitespace words that carry a letter or digit. Placeholders are ignored, product names count once."""
    text = PLACEHOLDER.sub(" ", label)
    for name in PRODUCT_NAMES:
        text = text.replace(name, "Product")
    return sum(1 for tok in text.split() if any(c.isalnum() for c in tok))


def violations_of(label: str, expr: str = "") -> List[str]:
    """`expr` is the source expression the literal came from; a `.len()` in it means a placeholder holds a count."""
    kinds: List[str] = []
    if word_count(label) > MAX_ACTION_WORDS:
        kinds.append(WORDS_KIND)
    if COUNT_IN_PARENS.search(label):
        kinds.append(COUNT_KIND)
    elif DIGIT_TOKEN.search(PLACEHOLDER.sub(" ", label)) or ("{" in label and ".len()" in expr):
        kinds.append(COUNT_LABEL_KIND)
    return kinds


def _statement_end(src: str, i: int) -> int:
    """Index of the `;` that ends the statement starting at `i` (depth 0, literals opaque), else end of text."""
    depth, n = 0, len(src)
    while i < n:
        ch = src[i]
        if ch == '"':
            i = _literal_end(src, i)
            continue
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
            if depth < 0:
                return i
        elif ch == ";" and depth == 0:
            return i
        i += 1
    return n


def let_bound_expr(code: str, use_idx: int, ident: str) -> str:
    """Initializer of the nearest `let ident = ...;` before `use_idx` in the same fn, else ''."""
    fns = list(re.finditer(r"\bfn\s", code[:use_idx]))
    fn_start = fns[-1].start() if fns else 0
    lets = list(re.finditer(r"\blet\s+(?:mut\s+)?" + re.escape(ident) + r"\b[^=;]*=(?!=)", code[fn_start:use_idx]))
    if not lets:
        return ""
    start = fn_start + lets[-1].end()
    return code[start : _statement_end(code, start)]


def fn_bodies(code: str, name: str) -> Iterator[str]:
    for m in re.finditer(r"\bfn\s+" + re.escape(name) + r"\b[^{;]*\{", code):
        depth, i, n = 1, m.end(), len(code)
        while i < n and depth:
            if code[i] == '"':
                i = _literal_end(code, i)
                continue
            depth += {"{": 1, "}": -1}.get(code[i], 0)
            i += 1
        yield code[m.end() : i]


def _skipped(rel: str) -> bool:
    return rel.startswith(SKIPPED_PREFIXES) or "/tests/" in rel or rel.endswith(SKIPPED_FILES)


def scan_labels(files: Dict[str, str], code_of: Callable[[str], str]) -> Counter:
    """Counter of (file, kind, literal) for every label literal that breaks a mechanical rule."""
    found: Counter = Counter()
    for rel in sorted(files):
        if _skipped(rel):
            continue
        code = code_of(rel)
        for head, path_filter, arg_index, count_only in CALL_TABLE:
            if path_filter and not rel.startswith(path_filter):
                continue
            for m in re.finditer(head + r"\s*\(", code):
                if re.search(r"\bfn\s+$", code[max(0, m.start() - 4) : m.start()]):
                    continue
                args = split_call_args(code, m.end() - 1)
                if len(args) <= arg_index:
                    continue
                if count_only and ".len()" not in args[arg_index]:
                    continue
                arg = args[arg_index]
                bound = BARE_IDENT.match(arg)
                if bound and not count_only:
                    arg = let_bound_expr(code, m.start(), bound.group(1)) or arg
                for literal in literals_in(arg):
                    for kind in violations_of(literal, arg):
                        if count_only and kind != COUNT_KIND:
                            continue
                        found[(rel, kind, literal)] += 1
        for fn_name, prefix in LABEL_FNS:
            if rel.startswith(prefix):
                for body in fn_bodies(code, fn_name):
                    for literal in literals_in(body):
                        for kind in violations_of(literal):
                            found[(rel, kind, literal)] += 1
    return found


def load_baseline(path: Path = BASELINE_PATH) -> Counter:
    if not path.exists():
        return Counter()
    data = json.loads(path.read_text(encoding="utf-8"))
    return Counter({(e["file"], e["kind"], e["literal"]): e["count"] for e in data["entries"]})


def baseline_json(found: Counter) -> str:
    entries = [
        {"file": f, "kind": k, "literal": lit, "count": c}
        for (f, k, lit), c in sorted(found.items())
    ]
    return json.dumps({"version": 1, "entries": entries}, indent=2, ensure_ascii=False) + "\n"


def compare_to_baseline(found: Counter, baseline: Counter) -> Tuple[List[str], List[str], List[str]]:
    """(new, fixed, moved) as printable lines. A label that left one file and appeared in another is one `moved` line."""
    extra: Counter = Counter()
    gone: Counter = Counter()
    for key in set(found) | set(baseline):
        have, allowed = found.get(key, 0), baseline.get(key, 0)
        if have > allowed:
            extra[key] = have - allowed
        elif have < allowed:
            gone[key] = allowed - have
    moved: List[str] = []
    for key in sorted(extra):
        f, k, lit = key
        for src in sorted(g for g in gone if g[1:] == (k, lit) and g[0] != f):
            take = min(extra[key], gone[src])
            if take:
                extra[key] -= take
                gone[src] -= take
                moved.append(f"{src[0]} -> {f} [{k}] {lit!r}")
    new = [f"{f} [{k}] {lit!r}" for (f, k, lit), c in sorted(extra.items()) if c > 0]
    fixed = [f"{f} [{k}] {lit!r}" for (f, k, lit), c in sorted(gone.items()) if c > 0]
    return new, fixed, moved


def growth_offenders(found: Counter, baseline: Counter) -> List[str]:
    """Entries a baseline rewrite would add or raise. A move to another file with the same kind and literal is not growth."""
    return compare_to_baseline(found, baseline)[0]


def check_ux_writing(files: Dict[str, str], code_of: Callable[[str], str], baseline: Optional[Counter] = None) -> Tuple[bool, str]:
    found = scan_labels(files, code_of)
    base = load_baseline() if baseline is None else baseline
    new, fixed, moved = compare_to_baseline(found, base)
    total = sum(found.values())
    if not new and not fixed and not moved:
        return True, f"{total} labelled violation(s) all in scripts/ux_writing_baseline.json; none new (design/ux-writing.md rules 1 and 3)"
    parts = []
    if new:
        parts.append(f"NEW (rewrite the label: <=3 words, no count in it; the baseline cannot absorb it): {new[:6]}{' ...' if len(new) > 6 else ''}")
    if moved:
        parts.append(f"moved, baseline names the old file (--write-ux-baseline accepts a pure move): {moved[:6]}{' ...' if len(moved) > 6 else ''}")
    if fixed:
        parts.append(f"fixed, remove from baseline (python3 scripts/zero_fake_ui_gate.py --write-ux-baseline; it only shrinks): {fixed[:6]}{' ...' if len(fixed) > 6 else ''}")
    return False, "; ".join(parts)


def iter_summary(found: Counter) -> Iterator[Tuple[str, int]]:
    per_file: Counter = Counter()
    for (f, _k, _l), c in found.items():
        per_file[f] += c
    yield from per_file.most_common()
