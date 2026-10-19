#!/usr/bin/env python3
"""
Zero Fake UI Deterministic Enforcement Gate (`scripts/zero_fake_ui_gate.py`)
============================================================================
Enforces `DIR-28` / `BR-31` / `REQ-25` / `D25` ("Zero Fake UI Law — Nghiêm Cấm Fake UI"):
Every visible button, input bar, scrollbar, tab, menu item, and agent CLI runtime in
`design:os studio` (`crates/designos-app/src/`) must execute real state mutation, real OS
clipboard/filesystem I/O, or real CLI subprocess execution.

Verifies 15 hard Zero-Fake-UI invariants (`Z01..Z15`):
  - Z01: `terminal.rs` has `runtime_binary_available` and real CLI invocation
         (`claude -p`, `codex exec`, `agy -p`, `grok -p`) in `submit_chat_prompt`.
  - Z02: `terminal.rs` `set_active_runtime` mutates `tab.preset` (not just renaming `tab.title`)
         without typing into the live PTY; `DESIGNOS_AGENT_RUNTIME` is set in `configure_pty_env`
         and `build_real_cli_command`.
  - Z03: `terminal.rs` `dispatch_figma_ai_prompt` does NOT prefix PTY commands with a `#` shell comment.
  - Z04: `terminal.rs` header & composer buttons (`agents-header-ai`, `agents-header-popout`,
         `agents-header-more`, `agents-composer-attach`, `agents-composer-context`) all have
         interactive click handlers (`Sense::click()`); `agents-composer-mic` stays removed.
  - Z05: `specs.rs` `specs-ask-agent-bar` hosts a real `TextEdit::singleline` (`ask_input`)
         instead of only sending a hardcoded prompt string.
  - Z06: `specs.rs` Right Inspector has dynamic `active_section_toc` and interactive
         `SpecComment` creation/resolution (`comment_draft` + `TextEdit::singleline`).
  - Z07: `specs.rs` `draw_wrapped_prose` does NOT truncate prose at `lines.len() >= 3`,
         and `render_tab_blueprints` renders `pkg.ui_contract`.
  - Z08: `panels/toolbar.rs` contextual buttons (`ctx-select-area`, `ctx-edit-prompt`) mutate
         `app.doc` / `app.tool`; the fake `ctx-crop` / `ctx-remove-bg` stubs and the cmdk
         "Remove background" entry stay removed (no occurrence in any shipped, non-test Rust code).
  - Z09: `home/mod.rs` `Act::ExportFig` writes `.fig` bytes to disk (`std::fs::write`),
         and `Nav::AllFolders` / `Nav::Templates` / `Nav::Libraries` filter distinctly.
  - Z10: `panels/right.rs` `"Share"` copies to clipboard (`copy_text`), and Dev Mode's 2nd tab
         renders the Dev Plugins inspector instead of `super::prototype::show`.
  - Z11: `panels/inspect.rs` (`dev-code-connect`, `dev-mcp-btn`, `dev-add-redlines`,
         `inspect_keyframes_section`) and `panels/properties.rs` (`copy-to-slides`) perform
         real clipboard/document mutations.
  - Z13: `panels/menus.rs` Zoom menu rows are exactly the allow-listed labels, each mapped to its
         real `Action`; a `checked:` value is `false` or read from `app.` state, never hard-coded true.
  - Z14: the in-app updater UI stays hidden (`UPDATER_UI_ENABLED = false`) while `updater.rs`
         still builds its candidate from `ReleaseManifest::sample_for_channel`; the guard must be
         CODE (comments stripped) in every entry point: the `tabs.rs` button, the Home sidebar card
         and the update modal.
  - Z15: `terminal.rs` never calls `synthesize_spec_from_prompt` or prints `Executed via` /
         `PRD Context:`; `synthesize_spec_from_prompt` is private to `cmdk.rs`.

All source matching runs on shipped code: `//` comments and `#[cfg(test)]` items are stripped first, then
every string literal is rewritten to the text the Rust compiler builds from it (escapes decoded, raw strings quoted,
`concat!` of literals and adjacent literals joined).
`scripts/zero_fake_ui_gate_selftest.py` re-introduces each fake-UI regression in memory and asserts
this gate fails.
  - Z12: `widgets/picker.rs` `Mode::Image` creates real `Paint::Image` / `app.doc.images`
         entries, and `panels/mod.rs::scrollbar` + `variables/view.rs::hscrollbar` support
         `Sense::click_and_drag()`.

Exit code:
  0 when 15/15 invariants pass, 1 if any invariant fails.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
APP_SRC = REPO_ROOT / "crates" / "designos-app" / "src"


def literal_end(src: str, i: int) -> int:
    """Returns the exclusive end of the string / raw string / char literal starting at `src[i]`, else 0."""
    n = len(src)
    ch = src[i]
    if ch == '"':
        j = i + 1
        while j < n:
            if src[j] == "\\":
                j += 2
            elif src[j] == '"':
                return j + 1
            else:
                j += 1
        return n
    if ch in "rb" and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
        j = i + 1
        if ch == "b" and j < n and src[j] == "r":
            j += 1
        elif ch == "b":
            return 0
        hashes = 0
        while j < n and src[j] == "#":
            hashes += 1
            j += 1
        if j < n and src[j] == '"':
            close = src.find('"' + "#" * hashes, j + 1)
            return n if close == -1 else close + 1 + hashes
        return 0
    if ch == "'":
        if i + 1 < n and src[i + 1] == "\\":
            close = src.find("'", i + 3)
            return n if close == -1 else close + 1
        if i + 2 < n and src[i + 2] == "'":
            return i + 3
    return 0


def strip_rust_comments(src: str) -> str:
    """Blanks `//` line comments (incl. doc comments) and nested `/* */` blocks, keeping literals and line numbers."""
    out: List[str] = []
    i, n = 0, len(src)
    while i < n:
        if src.startswith("//", i):
            nl = src.find("\n", i)
            i = n if nl == -1 else nl
            continue
        if src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            out.append(" " + "\n" * src.count("\n", i, j))
            i = j
            continue
        end = literal_end(src, i)
        if end:
            out.append(src[i:end])
            i = end
            continue
        out.append(src[i])
        i += 1
    return "".join(out)


def strip_cfg_test_items(src: str) -> str:
    """Removes every item annotated `#[cfg(test)]` (module, fn, use) so only shipped code remains."""
    out: List[str] = []
    pos = 0
    marker = re.compile(r"#\[cfg\(test\)\]")
    while True:
        m = marker.search(src, pos)
        if not m:
            out.append(src[pos:])
            return "".join(out)
        out.append(src[pos : m.start()])
        i, depth, n = m.end(), 0, len(src)
        while i < n:
            end = literal_end(src, i)
            if end:
                i = end
                continue
            ch = src[i]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth <= 0:
                    i += 1
                    break
            elif ch == ";" and depth == 0:
                i += 1
                break
            i += 1
        pos = i


_SIMPLE_ESCAPES = {"n": "\n", "r": "\r", "t": "\t", "0": "\0", "\\": "\\", "'": "'", '"': '"'}
_HEX_BYTE_RE = re.compile(r"[0-9a-fA-F]{2}")
_UNICODE_BODY_RE = re.compile(r"\{([0-9a-fA-F_]*)\}")
_RAW_STRING_OPEN_RE = re.compile(r'b?r(#*)"')
_LITERAL_START_RE = re.compile(r"[\"'rb]")
_CODE_TOKEN_RE = re.compile(r"[A-Za-z_]\w*|\d[\w.]*|\s+|.", re.S)

# One lexical token: (kind, source text, value). Kinds: "str" (any string literal; `value` is the text it holds),
# "chr" (char literal; `value` is the char), "id", "num", "ws", and "p" (any other single character).
RustToken = Tuple[str, str, str]


def decode_rust_escapes(body: str) -> str:
    """Value of a non-raw string / char literal body, as the compiler reads it.

    Decodes `\\n \\r \\t \\0 \\\\ \\' \\"`, `\\xHH`, `\\u{H..}` and the backslash-newline continuation (which also
    swallows the whitespace that opens the next line). An escape the compiler rejects stays verbatim.
    """
    out: List[str] = []
    i, n = 0, len(body)
    while i < n:
        ch = body[i]
        if ch != "\\" or i + 1 >= n:
            out.append(ch)
            i += 1
            continue
        nxt = body[i + 1]
        if nxt in _SIMPLE_ESCAPES:
            out.append(_SIMPLE_ESCAPES[nxt])
            i += 2
        elif nxt == "x" and _HEX_BYTE_RE.fullmatch(body[i + 2 : i + 4]):
            out.append(chr(int(body[i + 2 : i + 4], 16)))
            i += 4
        elif nxt == "u" and (unicode_body := _UNICODE_BODY_RE.match(body, i + 2)):
            digits = unicode_body.group(1).replace("_", "")
            codepoint = int(digits, 16) if 1 <= len(digits) <= 6 else -1
            if 0 <= codepoint <= 0x10FFFF and not 0xD800 <= codepoint <= 0xDFFF:
                out.append(chr(codepoint))
            else:
                out.append(body[i : unicode_body.end()])
            i = unicode_body.end()
        elif nxt in "\r\n":
            i += 2
            while i < n and body[i] in " \t\r\n":
                i += 1
        else:
            out.append(body[i : i + 2])
            i += 2
    return "".join(out)


def _literal_token(text: str) -> RustToken:
    """Token for a literal span returned by `literal_end`: normal string, raw string or char."""
    if text[0] in "'\"":
        closed = len(text) >= 2 and text.endswith(text[0])
        body = decode_rust_escapes(text[1:-1] if closed else text[1:])
        return ("chr" if text[0] == "'" else "str", text, body)
    opening = _RAW_STRING_OPEN_RE.match(text)
    if not opening:
        return ("str", text, text)
    body = text[opening.end() :]
    closing = '"' + opening.group(1)
    return ("str", text, body[: -len(closing)] if body.endswith(closing) else body)


def _code_tokens(text: str) -> List[RustToken]:
    tokens: List[RustToken] = []
    for m in _CODE_TOKEN_RE.finditer(text):
        piece = m.group(0)
        if piece[0].isspace():
            kind = "ws"
        elif piece[0].isalpha() or piece[0] == "_":
            kind = "id"
        elif piece[0].isdigit():
            kind = "num"
        else:
            kind = "p"
        tokens.append((kind, piece, ""))
    return tokens


def lex_rust(src: str) -> List[RustToken]:
    """Splits Rust source into literal tokens (found with `literal_end`, so a `'"'` char or a raw string cannot
    desynchronise string pairing) and code tokens."""
    tokens: List[RustToken] = []
    pending: List[str] = []
    i = 0
    while True:
        start = _LITERAL_START_RE.search(src, i)
        if not start:
            pending.append(src[i:])
            break
        at = start.start()
        end = literal_end(src, at)
        if end:
            pending.append(src[i:at])
            tokens.extend(_code_tokens("".join(pending)))
            pending.clear()
            tokens.append(_literal_token(src[at:end]))
            i = end
        else:
            pending.append(src[i : at + 1])
            i = at + 1
    tokens.extend(_code_tokens("".join(pending)))
    return tokens


def quote_rust_string(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _skip_whitespace(tokens: List[RustToken], i: int) -> int:
    while i < len(tokens) and tokens[i][0] == "ws":
        i += 1
    return i


def _is_punct(tokens: List[RustToken], i: int, ch: str) -> bool:
    return i < len(tokens) and tokens[i][0] == "p" and tokens[i][1] == ch


_MACRO_CLOSERS = {"(": ")", "[": "]", "{": "}"}


def _parse_concat_macro(tokens: List[RustToken], i: int) -> Optional[Tuple[str, int]]:
    """`concat!(...)`, `concat![...]` or `concat!{...}` starting at `tokens[i]` whose pieces are all literals the
    compiler can join (string, raw string, char, number, bool, nested `concat!`): returns (joined text, index
    after the closing delimiter). The closing delimiter must be the one that matches the opening one."""
    if tokens[i][0] != "id" or tokens[i][1] != "concat":
        return None
    j = _skip_whitespace(tokens, i + 1)
    if not _is_punct(tokens, j, "!"):
        return None
    j = _skip_whitespace(tokens, j + 1)
    closer = _MACRO_CLOSERS.get(tokens[j][1]) if j < len(tokens) and tokens[j][0] == "p" else None
    if closer is None:
        return None
    j += 1
    parts: List[str] = []
    while True:
        j = _skip_whitespace(tokens, j)
        if j >= len(tokens):
            return None
        kind, text, value = tokens[j]
        if kind in ("str", "chr"):
            parts.append(value)
            j += 1
        elif kind == "num" or (kind == "id" and text in ("true", "false")):
            parts.append(text)
            j += 1
        elif _is_punct(tokens, j, "-") and (k := _skip_whitespace(tokens, j + 1)) < len(tokens) and tokens[k][0] == "num":
            parts.append("-" + tokens[k][1])
            j = k + 1
        elif kind == "id" and text == "concat":
            nested = _parse_concat_macro(tokens, j)
            if nested is None:
                return None
            parts.append(nested[0])
            j = nested[1]
        else:
            return None
        j = _skip_whitespace(tokens, j)
        if _is_punct(tokens, j, closer):
            return "".join(parts), j + 1
        if not _is_punct(tokens, j, ","):
            return None
        j = _skip_whitespace(tokens, j + 1)
        if _is_punct(tokens, j, closer):
            return "".join(parts), j + 1


def fold_compile_time_strings(tokens: List[RustToken]) -> List[RustToken]:
    """Joins what the compiler joins into one `&'static str`: `concat!` of literal pieces (nested too) and string
    literals that sit next to each other (valid Rust only as macro input, e.g. `join!("a" "b")`)."""
    changed = True
    while changed:
        changed = False
        out: List[RustToken] = []
        i = 0
        while i < len(tokens):
            joined = _parse_concat_macro(tokens, i)
            if joined is not None:
                out.append(("str", "", joined[0]))
                i = joined[1]
                changed = True
                continue
            if tokens[i][0] == "str":
                value = tokens[i][2]
                j = _skip_whitespace(tokens, i + 1)
                while j < len(tokens) and tokens[j][0] == "str":
                    value += tokens[j][2]
                    i = j
                    j = _skip_whitespace(tokens, j + 1)
                    changed = True
                out.append(("str", "", value))
                i += 1
                continue
            out.append(tokens[i])
            i += 1
        tokens = out
    return tokens


def normalize_rust_literals(src: str) -> str:
    """Source with every string literal rewritten to the text it holds: escapes decoded, raw strings quoted,
    `concat!` of literals and adjacent literals folded into one `"..."`."""
    return "".join(
        quote_rust_string(value) if kind == "str" else text
        for kind, text, value in fold_compile_time_strings(lex_rust(src))
    )


def shipped_code(src: str) -> str:
    """Rust source as the binary sees it: no comments, no `#[cfg(test)]` items, with string literals normalized."""
    return normalize_rust_literals(strip_cfg_test_items(strip_rust_comments(src)))


class SourceSet:
    """In-memory view of `crates/designos-app/src/**/*.rs`; the self-test mutates copies of it."""

    def __init__(self, files: Dict[str, str]) -> None:
        self.files = dict(files)
        self._code: Dict[str, str] = {}
        self._unnormalized_code: Dict[str, str] = {}

    @classmethod
    def from_disk(cls) -> "SourceSet":
        files = {
            p.relative_to(APP_SRC).as_posix(): p.read_text(encoding="utf-8", errors="replace")
            for p in sorted(APP_SRC.rglob("*.rs"))
        }
        return cls(files)

    def with_replacement(self, rel_path: str, old: str, new: str) -> "SourceSet":
        """Copy of this set with `old` -> `new` in one file; fails loudly if `old` is absent (a vacuous mutation)."""
        text = self.files.get(rel_path, "")
        if old not in text:
            raise AssertionError(f"mutation anchor not found in {rel_path}: {old!r}")
        files = dict(self.files)
        files[rel_path] = text.replace(old, new, 1)
        mutated = SourceSet(files)
        # Only `rel_path` changed, so every other file's already-computed views stay valid.
        mutated._code = {rel: code for rel, code in self._code.items() if rel != rel_path}
        mutated._unnormalized_code = {rel: code for rel, code in self._unnormalized_code.items() if rel != rel_path}
        return mutated

    def raw(self, rel_path: str) -> str:
        return self.files.get(rel_path, "")

    def unnormalized_code(self, rel_path: str) -> str:
        if rel_path not in self._unnormalized_code:
            self._unnormalized_code[rel_path] = strip_cfg_test_items(strip_rust_comments(self.raw(rel_path)))
        return self._unnormalized_code[rel_path]

    def code(self, rel_path: str) -> str:
        if rel_path not in self._code:
            self._code[rel_path] = shipped_code(self.raw(rel_path))
        return self._code[rel_path]

    def code_hits(self, pattern: str, only: str = "") -> List[str]:
        """Files whose shipped code (comments and tests excluded, literals normalized) matches `pattern`."""
        rx = re.compile(pattern)
        hits = []
        for rel in sorted(self.files):
            if only and rel != only:
                continue
            if rx.search(self.code(rel)):
                hits.append(rel)
        return hits


def extract_fn_body(src: str, fn_name: str) -> str:
    """Extracts the body of `fn <fn_name>(...)` up to its matching closing brace."""
    pattern = re.compile(rf"\b(?:pub(?:\([^)]*\))?\s+)?fn\s+{re.escape(fn_name)}\b")
    match = pattern.search(src)
    if not match:
        return ""
    brace_start = src.find("{", match.end())
    if brace_start == -1:
        return ""
    depth = 0
    in_str = False
    escape = False
    for idx in range(brace_start, len(src)):
        ch = src[idx]
        if in_str:
            if escape:
                escape = False
            elif ch == "\\":
                escape = True
            elif ch == '"':
                in_str = False
            continue
        if ch == '"':
            in_str = True
        elif ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return src[brace_start : idx + 1]
    return src[brace_start:]


def check_z01(terminal_rs: str) -> Tuple[bool, str]:
    has_avail = "fn runtime_binary_available" in terminal_rs
    submit_body = extract_fn_body(terminal_rs, "submit_chat_prompt")
    helper_body = extract_fn_body(terminal_rs, "build_real_cli_command")
    combined = submit_body + "\n" + helper_body
    cli_checks = {
        "claude": ("claude --dangerously-skip-permissions -p" in combined or "claude -p" in combined),
        "codex": ("codex --dangerously-bypass-approvals-and-sandbox exec" in combined or "codex exec" in combined),
        "agy": ("agy --dangerously-skip-permissions -p" in combined or "agy -p" in combined),
        "grok": ("grok --always-approve -p" in combined or "grok -p" in combined),
    }
    missing = [name for name, present in cli_checks.items() if not present]
    calls_avail = "runtime_binary_available" in submit_body
    ok = has_avail and calls_avail and not missing
    if ok:
        return True, "runtime_binary_available + real CLI invocations (claude, codex, agy, grok with skip-permissions) verified in submit_chat_prompt"
    reasons = []
    if not has_avail:
        reasons.append("missing `fn runtime_binary_available`")
    if not calls_avail:
        reasons.append("`submit_chat_prompt` does not call `runtime_binary_available`")
    if missing:
        reasons.append(f"missing CLI invocations: {missing}")
    return False, "; ".join(reasons)


def check_z02(terminal_rs: str) -> Tuple[bool, str]:
    body = extract_fn_body(terminal_rs, "set_active_runtime")
    has_preset = bool(re.search(r"tab\.preset\s*=\s*runtime", body))
    # Typing into the tab lands as a chat prompt when the tab hosts an agent TUI.
    no_pty_write = "send_line" not in body and "write_all" not in body
    spawn_env = "DESIGNOS_AGENT_RUNTIME" in extract_fn_body(terminal_rs, "configure_pty_env")
    cmd_env = "DESIGNOS_AGENT_RUNTIME" in extract_fn_body(terminal_rs, "build_real_cli_command")
    ok = bool(body) and has_preset and no_pty_write and spawn_env and cmd_env
    if ok:
        return True, "`set_active_runtime` mutates `tab.preset = runtime` without PTY writes; `DESIGNOS_AGENT_RUNTIME` set at spawn + per CLI command"
    return False, (
        f"tab.preset mutation={has_preset}, no PTY write in set_active_runtime={no_pty_write}, "
        f"env in configure_pty_env={spawn_env}, env in build_real_cli_command={cmd_env}"
    )


def check_z03(terminal_rs: str) -> Tuple[bool, str]:
    body = extract_fn_body(terminal_rs, "dispatch_figma_ai_prompt")
    if not body:
        return False, "`fn dispatch_figma_ai_prompt` not found in terminal.rs"
    # Must not format or send a `#`-prefixed shell comment line as the PTY command
    comment_noop = bool(re.search(r'format!\(\s*"#\s*\[', body)) or bool(
        re.search(r'send_line\(\s*"#', body)
    )
    has_send = ("send_line" in body) or ("submit_chat_prompt" in body)
    ok = (not comment_noop) and has_send
    if ok:
        return True, "`dispatch_figma_ai_prompt` dispatches executable PTY/CLI command without `#` comment prefix"
    return False, f"dispatch_figma_ai_prompt comment_noop={comment_noop}, has_send={has_send}"


def check_z04(sources: "SourceSet") -> Tuple[bool, str]:
    terminal_rs = sources.code("terminal.rs")
    required_ids = [
        "agents-header-ai",
        "agents-header-popout",
        "agents-header-more",
        "agents-composer-attach",
        "agents-composer-context",
    ]
    missing = []
    for wid in required_ids:
        # Verify the ID appears in an interactive Sense::click() block with a .clicked() handler
        pattern = re.compile(
            rf'"{re.escape(wid)}"[\s\S]{{0,520}}?(?:Sense::click\(\)|\.clicked\(\))|Sense::click\(\)[\s\S]{{0,520}}?"{re.escape(wid)}"'
        )
        if not pattern.search(terminal_rs):
            missing.append(wid)
    has_fake_mic = "agents-composer-mic" in terminal_rs
    if not missing and not has_fake_mic:
        return True, f"All 5 header/composer controls ({', '.join(required_ids)}) have interactive Sense::click() handlers; fake mic removed"
    return False, f"Missing interactive Sense::click() handler for: {missing}, has_fake_mic={has_fake_mic}"


def check_z05(specs_rs: str) -> Tuple[bool, str]:
    has_bar = "specs-ask-agent-bar" in specs_rs
    has_input_field = "ask_input" in specs_rs
    has_text_edit = bool(
        re.search(r"TextEdit::singleline\s*\(\s*&mut\s+[a-zA-Z0-9_.]*ask_input\b", specs_rs)
    )
    ok = has_bar and has_input_field and has_text_edit
    if ok:
        return True, "`specs-ask-agent-bar` hosts editable `TextEdit::singleline(&mut ...ask_input)`"
    return (
        False,
        f"specs-ask-agent-bar={has_bar}, ask_input={has_input_field}, TextEdit::singleline(ask_input)={has_text_edit}",
    )


def check_z06(specs_rs: str) -> Tuple[bool, str]:
    has_toc = "active_section_toc" in specs_rs
    has_comment_struct = "SpecComment" in specs_rs
    has_draft = "comment_draft" in specs_rs
    has_text_edit = bool(
        re.search(r"TextEdit::singleline\s*\(\s*&mut\s+[a-zA-Z0-9_.]*comment_draft\b", specs_rs)
    )
    ok = has_toc and has_comment_struct and has_draft and has_text_edit
    if ok:
        return True, "Right Spec Inspector has dynamic `active_section_toc` + interactive `SpecComment` (`comment_draft` + `TextEdit::singleline`)"
    return (
        False,
        f"active_section_toc={has_toc}, SpecComment={has_comment_struct}, comment_draft={has_draft}, TextEdit(comment_draft)={has_text_edit}",
    )


def check_z07(specs_rs: str) -> Tuple[bool, str]:
    prose_body = extract_fn_body(specs_rs, "draw_wrapped_prose")
    if not prose_body:
        return False, "`fn draw_wrapped_prose` not found in specs.rs"
    truncates_at_3 = bool(re.search(r"lines\.len\(\)\s*>=\s*3", prose_body))
    blueprints_body = (
        extract_fn_body(specs_rs, "render_tab_blueprints")
        + "\n"
        + extract_fn_body(specs_rs, "render_wireframes_and_contract_tab")
    )
    renders_ui_contract = bool(
        re.search(r"(?:&pkg\.ui_contract|pkg\.ui_contract\.iter\(\))", blueprints_body)
    )
    ok = (not truncates_at_3) and bool(blueprints_body.strip()) and renders_ui_contract
    if ok:
        return True, "`draw_wrapped_prose` does not truncate at `lines.len() >= 3` and `render_tab_blueprints` renders `pkg.ui_contract`"
    return (
        False,
        f"truncates_at_3={truncates_at_3}, has_blueprints_fn={bool(blueprints_body.strip())}, renders_ui_contract={renders_ui_contract}",
    )


RETIRED_ACTION_PATTERN = r"ctx-crop|ctx-remove-bg"
RETIRED_CMDK_PATTERN = r"(?i)remove[\s_-]*background"
RETIRED_MULTIPLAYER_PATTERN = r"(?i)multiplayer\s+cursors"
# A `concat!` invocation in any of its spellings: `(...)`, `[...]` or `{...}`, with optional space around the bang.
CONCAT_MACRO_CALL = r"\bconcat\s*!\s*[(\[{]"
# Labels where any `\u{` / `\xHH` escape is banned (cmdk titles, zoom rows) ...
UI_LABEL_ANY_ESCAPE_PATTERN = re.compile(CONCAT_MACRO_CALL + r"|\\u\{|\\x[0-9a-fA-F]{2}")
# ... and the toolbar, which legitimately spells arrow glyphs as long `\u{21A9}` escapes: only short ASCII-range ones are banned.
UI_LABEL_OBFUSCATION_PATTERN = re.compile(CONCAT_MACRO_CALL + r"|\\u\{0*[0-7]?[0-9a-fA-F]\}|\\x[0-9a-fA-F]{2}")


def check_z08(sources: "SourceSet") -> Tuple[bool, str]:
    toolbar_rs = sources.code("panels/toolbar.rs")
    has_select = "ctx-select-area" in toolbar_rs
    has_edit_img = ("ctx-edit-image" in toolbar_rs) or ("ctx-edit-prompt" in toolbar_rs)
    # Crop / Remove-background had no real implementation (a 5% shrink and "delete first fill").
    # They may only come back with an honest implementation, which must update this check.
    # Scan all shipped, non-test sources (not only cmdk.rs / toolbar.rs) and reject concat! / \u{ in UI labels.
    retired_actions = sources.code_hits(RETIRED_ACTION_PATTERN)
    retired_cmdk = sources.code_hits(RETIRED_CMDK_PATTERN)
    cmdk_unnorm = sources.unnormalized_code("cmdk.rs")
    toolbar_unnorm = sources.unnormalized_code("panels/toolbar.rs")
    obfuscated_ui_labels = []
    if UI_LABEL_ANY_ESCAPE_PATTERN.search(cmdk_unnorm):
        obfuscated_ui_labels.append("cmdk.rs")
    if UI_LABEL_OBFUSCATION_PATTERN.search(toolbar_unnorm):
        obfuscated_ui_labels.append("panels/toolbar.rs")
    stubs_gone = not retired_actions and not retired_cmdk and not obfuscated_ui_labels
    mutates_state = (
        ("app.doc" in toolbar_rs or "doc." in toolbar_rs)
        and ("app.tool" in toolbar_rs)
        and ("apply_contextual_action" in toolbar_rs or "clips_content" in toolbar_rs)
    )
    ok = has_select and has_edit_img and stubs_gone and mutates_state
    if ok:
        return True, "`panels/toolbar.rs` contextual buttons (`ctx-select-area`, `ctx-edit-prompt`) mutate `app.doc` / `app.tool`; no ctx-crop / ctx-remove-bg / Remove background in any shipped source"
    return (
        False,
        f"select={has_select}, edit_img={has_edit_img}, mutates_state={mutates_state}, "
        f"retired ctx-crop/ctx-remove-bg in shipped code: {retired_actions}, retired cmdk Remove background: {retired_cmdk}, "
        f"obfuscated UI labels: {obfuscated_ui_labels}",
    )


def check_z09(home_rs: str) -> Tuple[bool, str]:
    has_export_fig = ("Act::ExportFig" in home_rs) or ("ExportFig" in home_rs)
    has_fs_write = ("std::fs::write" in home_rs) or ("fs::write" in home_rs)
    collapsed_sections = bool(
        re.search(
            r"Section::Recents\s*\|\s*Section::AllProjects\s*\|\s*Section::Templates\s*\|\s*Section::Resources",
            home_rs,
        )
    )
    has_distinct_nav = (not collapsed_sections) and (
        ("Nav::AllFolders" in home_rs and "Nav::Templates" in home_rs and "Nav::Libraries" in home_rs)
        or (
            "Section::AllProjects =>" in home_rs
            and "Section::Templates =>" in home_rs
            and "Section::Resources =>" in home_rs
        )
    )
    ok = has_export_fig and has_fs_write and has_distinct_nav
    if ok:
        return True, "`home/mod.rs` `Act::ExportFig` writes `.fig` bytes (`std::fs::write`) and `AllFolders`/`Templates`/`Libraries` filter distinctly"
    return (
        False,
        f"ExportFig={has_export_fig}, fs::write={has_fs_write}, distinct_nav={has_distinct_nav}",
    )


def check_z10(right_rs: str) -> Tuple[bool, str]:
    has_share_copy = ("\"Share\"" in right_rs) and ("copy_text" in right_rs)
    # Dev Mode's 2nd tab (`RightTab::Prototype` when `app.ui.dev_mode` is true) must NOT unconditionally call `super::prototype::show`
    unconditional_proto = bool(
        re.search(r"if\s+active\s*==\s*RightTab::Prototype\s*\{\s*super::prototype::show", right_rs)
    )
    has_dev_plugins = (not unconditional_proto) and (
        "plugins" in right_rs.lower() or "inspect::show_plugins" in right_rs or "dev_plugins" in right_rs
    )
    ok = has_share_copy and has_dev_plugins
    if ok:
        return True, "`panels/right.rs` `Share` copies to OS clipboard (`copy_text`) and Dev Mode 2nd tab renders Dev Plugins inspector"
    return False, f"has_share_copy={has_share_copy}, has_dev_plugins={has_dev_plugins} (unconditional_proto={unconditional_proto})"


def check_z11(inspect_rs: str, properties_rs: str, dev_sections_rs: str) -> Tuple[bool, str]:
    combined_inspect = inspect_rs + "\n" + dev_sections_rs
    req_inspect_tokens = [
        "dev-code-connect",
        "dev-mcp-btn",
        "dev-add-redlines",
        "inspect_keyframes_section",
    ]
    missing_inspect = [t for t in req_inspect_tokens if t not in combined_inspect]
    has_slides_id = ("copy-to-slides" in properties_rs) or ("copy-to-figma-slides" in properties_rs)
    # Verify `copy-to-slides` / `copy-to-figma-slides` performs real clipboard/doc mutation (not just show_toast)
    slides_match = re.search(
        r"copy-to-(?:figma-)?slides[\s\S]{0,900}?(copy_text|app\.doc|slides|export_svg|NodeSpec)",
        properties_rs,
    )
    has_slides_mutation = bool(slides_match)
    ok = (not missing_inspect) and has_slides_id and has_slides_mutation
    if ok:
        return True, "`panels/inspect.rs` (`dev-code-connect`, `dev-mcp-btn`, `dev-add-redlines`, `inspect_keyframes_section`) and `panels/properties.rs` (`copy-to-slides`) execute real clipboard/document mutations"
    return (
        False,
        f"missing_inspect={missing_inspect}, has_slides_id={has_slides_id}, has_slides_mutation={has_slides_mutation}",
    )


def check_z12(picker_rs: str, panels_mod_rs: str, vars_view_rs: str, properties_rs: str) -> Tuple[bool, str]:
    combined_picker = picker_rs + "\n" + properties_rs
    has_mode_image = "Mode::Image" in picker_rs
    has_image_paint = (
        ("Paint::Image" in combined_picker or "PaintKind::Image" in combined_picker)
        and ("images.insert" in combined_picker or "doc.images" in combined_picker or "checkerboard_image" in combined_picker or "default_image" in combined_picker)
    )
    scrollbar_body = extract_fn_body(panels_mod_rs, "scrollbar")
    hscrollbar_body = extract_fn_body(vars_view_rs, "hscrollbar")
    has_v_drag = "Sense::click_and_drag()" in scrollbar_body or "Sense::click_and_drag()" in panels_mod_rs
    has_h_drag = "Sense::click_and_drag()" in hscrollbar_body
    ok = has_mode_image and has_image_paint and has_v_drag and has_h_drag
    if ok:
        return True, "`widgets/picker.rs` `Mode::Image` creates real `Paint::Image` / `doc.images` entries and `panels::scrollbar` + `variables::view::hscrollbar` support `Sense::click_and_drag()`"
    return (
        False,
        f"Mode::Image={has_mode_image}, image_paint={has_image_paint}, panels::scrollbar drag={has_v_drag}, hscrollbar drag={has_h_drag}",
    )


# Exact zoom-menu rows (label -> real action). A new row needs a real feature behind it, so it needs a deliberate edit here.
ZOOM_MENU_ALLOWED_ROWS: Dict[str, str] = {
    "Zoom in": "Action::ZoomIn",
    "Zoom out": "Action::ZoomOut",
    "Zoom to fit": "Action::FitAll",
    "Zoom to 50%": "Action::ZoomTo(0.5)",
    "Zoom to 100%": "Action::Zoom100",
    "Zoom to 200%": "Action::ZoomTo(2.0)",
    "Pixel grid": "Action::TogglePixelGrid",
    "Snap to pixel grid": "Action::ToggleSnapPixel",
    "Layout guides": "Action::ToggleLayoutGrids",
    "Rulers": "Action::ToggleRulers",
    "Comments": "Action::Tool(Tool::Comment)",
}
ZOOM_ROW_PATTERN = re.compile(
    r'ZoomRowSpec\s*\{\s*label:\s*"(?P<label>[^"]*)"\s*,\s*shortcut:\s*"[^"]*"\s*,\s*'
    r"action:\s*(?P<action>.+?)\s*,\s*checked:\s*(?P<checked>.+?)\s*,\s*sep_after:",
    re.S,
)


def check_z13(sources: "SourceSet") -> Tuple[bool, str]:
    menus_rs = sources.code("panels/menus.rs")
    menus_unnorm = sources.unnormalized_code("panels/menus.rs")
    struct_at = menus_rs.find("struct ZoomRowSpec")
    zoom_src = menus_rs[struct_at : menus_rs.find("fn show_zoom_menu", struct_at)] if struct_at != -1 else ""
    rows_body = extract_fn_body(menus_rs, "zoom_rows")
    raw_rows_body = extract_fn_body(menus_unnorm, "zoom_rows")
    dead_rows = len(re.findall(r"action:\s*None", zoom_src))
    fake_check = bool(sources.code_hits(RETIRED_MULTIPLAYER_PATTERN))
    optional_action = bool(re.search(r"action:\s*Option<", zoom_src))
    obfuscated_zoom_labels = bool(UI_LABEL_ANY_ESCAPE_PATTERN.search(raw_rows_body))

    rows = [m.groupdict() for m in ZOOM_ROW_PATTERN.finditer(rows_body)]
    constructed = len(re.findall(r"ZoomRowSpec\s*\{", rows_body))
    unparsed = constructed - len(rows)  # a row written in an unexpected shape cannot be vetted, so it fails
    unlisted = [r["label"] for r in rows if r["label"] not in ZOOM_MENU_ALLOWED_ROWS]
    wrong_action = [
        r["label"] for r in rows if r["label"] in ZOOM_MENU_ALLOWED_ROWS and r["action"].strip() != ZOOM_MENU_ALLOWED_ROWS[r["label"]]
    ]
    missing = [label for label in ZOOM_MENU_ALLOWED_ROWS if label not in {r["label"] for r in rows}]
    hardcoded_checked = [r["label"] for r in rows if r["checked"].strip() != "false" and "app." not in r["checked"]]

    ok = (
        struct_at != -1
        and bool(rows_body)
        and dead_rows == 0
        and not fake_check
        and not optional_action
        and not obfuscated_zoom_labels
        and unparsed == 0
        and not unlisted
        and not wrong_action
        and not missing
        and not hardcoded_checked
    )
    if ok:
        return True, f"Zoom menu = the {len(rows)} allow-listed rows, each on its real `Action`; every `checked:` is `false` or read from `app.` state"
    return False, (
        f"found_struct={struct_at != -1}, zoom_rows_fn={bool(rows_body)}, action_none_rows={dead_rows}, multiplayer_cursors={fake_check}, "
        f"optional_action={optional_action}, obfuscated_zoom_labels={obfuscated_zoom_labels}, unparsed_rows={unparsed}, unlisted_labels={unlisted}, "
        f"wrong_action={wrong_action}, missing_rows={missing}, hardcoded_checked={hardcoded_checked}"
    )


# A guard may name the flag only by the full crate path, or bare inside updater.rs where the constant is defined
# (a local `let` there is a compile error because the name resolves to the constant). Any other spelling
# (`allow::UPDATER_UI_ENABLED`, or a bare name in tabs.rs that a local `let` can supply) could be an unrelated `true`.
UPDATER_FLAG_CRATE_PATH = r"crate\s*::\s*home\s*::\s*updater\s*::\s*UPDATER_UI_ENABLED"
UPDATER_FLAG_IN_UPDATER_RS = rf"(?:crate\s*::\s*home\s*::\s*updater\s*::\s*)?UPDATER_UI_ENABLED"
UPDATER_GUARD_FIRST_STATEMENT = re.compile(rf"^\{{\s*if\s+!\s*{UPDATER_FLAG_IN_UPDATER_RS}\s*\{{\s*return\s*;")
UPDATER_GUARD_BLOCK = re.compile(rf"\bif\s+{UPDATER_FLAG_CRATE_PATH}\s*\{{")
# Anything that binds the name `UPDATER_UI_ENABLED`: a const / static, or `use ... as UPDATER_UI_ENABLED`.
UPDATER_FLAG_BINDING = re.compile(r"\b(?:const|static)\s+UPDATER_UI_ENABLED\b|\bas\s+UPDATER_UI_ENABLED\b")


def matching_brace_end(src: str, open_idx: int) -> int:
    """Index just past the `}` matching the `{` at `open_idx` (literal-aware)."""
    depth, i, n = 0, open_idx, len(src)
    while i < n:
        end = literal_end(src, i)
        if end:
            i = end
            continue
        if src[i] == "{":
            depth += 1
        elif src[i] == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return n


def check_z14(sources: "SourceSet") -> Tuple[bool, str]:
    # Shipped code only: comments and #[cfg(test)] items are already stripped.
    updater_rs = sources.code("updater.rs")
    tabs_rs = sources.code("tabs.rs")
    uses_sample = "sample_for_channel" in updater_rs
    hidden = bool(re.search(r"pub const UPDATER_UI_ENABLED:\s*bool\s*=\s*false\s*;", updater_rs))
    # A guard is only as good as the binding its name resolves to: exactly one, the `false` constant in updater.rs.
    flag_bindings = {
        rel: len(UPDATER_FLAG_BINDING.findall(sources.code(rel))) for rel in sorted(sources.files)
    }
    flag_bindings = {rel: count for rel, count in flag_bindings.items() if count}
    single_flag_binding = flag_bindings == {"updater.rs": 1}

    # Tab-bar entry: every updater symbol / alias / button use must sit inside `if ...UPDATER_UI_ENABLED { ... }` CODE.
    guarded_spans = [(m.start(), matching_brace_end(tabs_rs, m.end() - 1)) for m in UPDATER_GUARD_BLOCK.finditer(tabs_rs)]
    entry_uses = [
        m.start()
        for m in re.finditer(
            r"top-tab-update-btn|\bUpdaterState\b|\bshow_update_modal\b|\bshow_sidebar_card(?:_in_rect)?\b|\bupdater\s+as\s+\w+|\.check_for_updates\(",
            tabs_rs,
        )
    ]
    unguarded_tab_uses = [at for at in entry_uses if not any(lo <= at < hi for lo, hi in guarded_spans)]
    tab_entry_gated = not unguarded_tab_uses

    # Home sidebar card + update modal: the guard is the exact first statement `if !...UPDATER_UI_ENABLED { return; }`.
    card_guarded = bool(UPDATER_GUARD_FIRST_STATEMENT.match(extract_fn_body(updater_rs, "show_sidebar_card_in_rect")))
    modal_guarded = bool(UPDATER_GUARD_FIRST_STATEMENT.match(extract_fn_body(updater_rs, "show_update_modal")))

    ok = (not uses_sample or hidden) and single_flag_binding and tab_entry_gated and card_guarded and modal_guarded
    if ok:
        return True, f"updater UI hidden={hidden}, sample manifest used={uses_sample}; guard is code in tabs.rs button, sidebar card and update modal"
    return False, (
        f"updater builds a sample manifest but its UI is reachable: hidden={hidden}, uses_sample={uses_sample}, "
        f"UPDATER_UI_ENABLED bindings (want only updater.rs once)={flag_bindings}, "
        f"tabs.rs unguarded updater uses at offsets {unguarded_tab_uses}, sidebar card guard first={card_guarded}, modal guard first={modal_guarded}"
    )


def check_z15(sources: "SourceSet") -> Tuple[bool, str]:
    terminal_code = sources.code("terminal.rs")
    cmdk_code = sources.code("cmdk.rs")
    forbidden_terminal = [
        tok
        for tok in (
            "synthesize_spec_from_prompt",
            "is_design_creation_prompt",
            "Executed via",
            "streamed to Canvas",
            "PRD Context:",
            "agents-cat-prd",
            "agents-composer-mic",
        )
        if tok in terminal_code
    ]
    synth_callers = [
        rel
        for rel in sources.code_hits(r"\bsynthesize_spec_from_prompt\b")
        if rel != "cmdk.rs"
    ]
    pub_synth_in_cmdk = bool(
        re.search(r"\bpub\s+fn\s+synthesize_spec_from_prompt\b", cmdk_code)
    )
    ok = not forbidden_terminal and not synth_callers and not pub_synth_in_cmdk
    if ok:
        return (
            True,
            "`terminal.rs` never calls `synthesize_spec_from_prompt` or prints `Executed via` / `PRD Context:`; `synthesize_spec_from_prompt` is private to `cmdk.rs`",
        )
    return (
        False,
        f"forbidden_terminal={forbidden_terminal}, synth_callers_outside_cmdk={synth_callers}, pub_synth_in_cmdk={pub_synth_in_cmdk}",
    )


def run_all_checks(sources: Optional["SourceSet"] = None) -> List[Dict[str, Any]]:
    sources = sources or SourceSet.from_disk()
    read_src = sources.raw
    terminal_rs = read_src("terminal.rs")
    specs_rs = read_src("specs.rs")
    home_rs = read_src("home/mod.rs")
    right_rs = read_src("panels/right.rs")
    inspect_rs = read_src("panels/inspect.rs")
    dev_sections_rs = read_src("dev/sections.rs")
    properties_rs = read_src("panels/properties.rs")
    picker_rs = read_src("widgets/picker.rs")
    panels_mod_rs = read_src("panels/mod.rs")
    vars_view_rs = read_src("variables/view.rs")

    specs = [
        ("Z01", "terminal.rs", "Real CLI binary detection & invocation in submit_chat_prompt", lambda: check_z01(terminal_rs)),
        ("Z02", "terminal.rs", "set_active_runtime mutates tab.preset; DESIGNOS_AGENT_RUNTIME set at spawn, never typed", lambda: check_z02(terminal_rs)),
        ("Z03", "terminal.rs", "dispatch_figma_ai_prompt sends executable command without # comment prefix", lambda: check_z03(terminal_rs)),
        ("Z04", "terminal.rs", "All 5 Agents header & composer buttons have Sense::click() handlers; fake mic removed", lambda: check_z04(sources)),
        ("Z05", "specs.rs", "specs-ask-agent-bar hosts editable TextEdit::singleline(ask_input)", lambda: check_z05(specs_rs)),
        ("Z06", "specs.rs", "Right Inspector dynamic active_section_toc & interactive SpecComment", lambda: check_z06(specs_rs)),
        ("Z07", "specs.rs", "Unwrapped draw_wrapped_prose & render_tab_blueprints renders pkg.ui_contract", lambda: check_z07(specs_rs)),
        ("Z08", "panels/toolbar.rs + cmdk.rs", "Contextual sub-toolbar buttons mutate app.doc / app.tool; ctx-crop / ctx-remove-bg / cmdk Remove background stay removed", lambda: check_z08(sources)),
        ("Z09", "home/mod.rs", "Act::ExportFig writes .fig to disk & Nav::AllFolders/Templates/Libraries filter distinctly", lambda: check_z09(home_rs)),
        ("Z10", "panels/right.rs", "Share copies to clipboard & Dev Mode 2nd tab renders Dev Plugins inspector", lambda: check_z10(right_rs)),
        ("Z11", "panels/inspect.rs + properties.rs", "Dev Mode Code Connect/MCP/Redlines/Keyframes & Copy to Slides mutate state/clipboard", lambda: check_z11(inspect_rs, properties_rs, dev_sections_rs)),
        ("Z12", "widgets/picker.rs + panels/mod.rs + variables/view.rs", "Mode::Image creates Paint::Image/doc.images & scrollbars support Sense::click_and_drag()", lambda: check_z12(picker_rs, panels_mod_rs, vars_view_rs, properties_rs)),
        ("Z13", "panels/menus.rs", "Zoom menu rows are the allow-listed set on real Actions; no hard-coded checked state", lambda: check_z13(sources)),
        ("Z14", "updater.rs + tabs.rs", "Sample-manifest updater UI stays hidden (guard is code in tabs, sidebar card, modal) until a real manifest fetch exists", lambda: check_z14(sources)),
        ("Z15", "terminal.rs + cmdk.rs", "No fake template synthesis or canned Executed via transcript in Agents panel; synthesize_spec_from_prompt stays private to cmdk.rs", lambda: check_z15(sources)),
    ]

    results: List[Dict[str, Any]] = []
    for cid, module, title, fn in specs:
        passed, detail = fn()
        results.append(
            {
                "id": cid,
                "module": module,
                "title": title,
                "passed": passed,
                "status": "PASS" if passed else "FAIL",
                "detail": detail,
            }
        )
    return results


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Zero Fake UI Deterministic Gate (DIR-28 / BR-31 / REQ-25 / D25)"
    )
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON report")
    args = parser.parse_args()

    results = run_all_checks()
    passed_count = sum(1 for r in results if r["passed"])
    total_count = len(results)
    all_passed = passed_count == total_count

    if args.json:
        payload = {
            "gate": "zero_fake_ui_gate",
            "directive": "DIR-28",
            "business_rule": "BR-31",
            "requirement": "REQ-25",
            "decision": "D25",
            "passed": passed_count,
            "total": total_count,
            "status": "PASS" if all_passed else "FAIL",
            "checks": results,
        }
        print(json.dumps(payload, indent=2))
    else:
        print("=" * 108)
        print(f" design:os studio — Zero Fake UI Enforcement Gate (DIR-28 · BR-31 · REQ-25 · D25)")
        print("=" * 108)
        print(f"{'ID':<5} | {'STATUS':<6} | {'MODULE':<32} | {'INVARIANT & EVIDENCE'}")
        print("-" * 108)
        for r in results:
            print(f"{r['id']:<5} | {r['status']:<6} | {r['module']:<32} | {r['title']} — {r['detail']}")
        print("-" * 108)
        verdict = "PASS" if all_passed else "FAIL"
        print(f" Gate Verdict: {verdict} ({passed_count}/{total_count} invariants satisfied)")
        print("=" * 108)

    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
