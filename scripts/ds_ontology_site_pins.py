"""Pinned absence sites for the design-system ontology gate (`check_ds_ontology_parity.py`, check P7).

Four patterns that the soul rules out or that debt still carries are pinned site by site. A site is the
triple (file, enclosing fn, item) with a count; the scan is a regex census over comment-stripped,
string-blanked, test-free source (`rust_literal_scan.clean`), the way the other ratchets here count.

    ns1  an upper-case literal on a painting line (.text( / label( / RichText::new( / badge / tag( / chip),
         outside the acronym allowlist
    ns2  `if <active|selected|current> { BG_BRAND`: the brand fill as a selected state
    ns4  `Mesh::default|with_texture|new` outside canvas/ and board/ (a gradient on chrome)
    ns5  `font_mono(` / `FontFamily::Monospace` (monospace text in chrome)

Each set of the scan must equal its pin exactly: a new key or a higher count is a new site, a lower count
or a missing key is a site that was removed and has to leave the pin too, so the pins only shrink.
`note` explains why a pinned site is allowed to stay; it is never compared.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from rust_literal_scan import clean

SetKey = Tuple[str, str, str]
SiteSets = Dict[str, Dict[SetKey, int]]
PinRow = Tuple[str, str, str, int, str]
Pins = Dict[str, List[PinRow]]

SET_IDS = ("ns1", "ns2", "ns4", "ns5")
SET_TITLES = {
    "ns1": "upper-case tag, badge or chip literal",
    "ns2": "brand fill as a selected state",
    "ns4": "gradient mesh outside canvas and board",
    "ns5": "monospace text in chrome",
}

ACRONYM_ALLOWLIST = frozenset(
    {"AI", "API", "CLI", "CSS", "MCP", "PRD", "SVG", "PNG", "JPG", "PDF", "URL", "ID", "UI", "AA", "AAA"}
)

_PAINT = re.compile(r"\.text\(|label\(|RichText::new\(|badge|tag\(|chip")
_UPPER_LITERAL = re.compile(r'"([A-Z][A-Z ]*[A-Z])"')
_NS2 = re.compile(
    r"if\s+[!(]*\s*(?:[a-z_.]*\b)?(active|selected|is_active|is_selected|current)\b[^{};]*\{\s*(?:theme::)?BG_BRAND\b"
)
_MESH = re.compile(r"Mesh::(default|with_texture|new)\(")
_MONO = re.compile(r"font_mono\(|FontFamily::Monospace")
_FN = re.compile(r"^\s*(?:pub(?:\([a-z]+\))? )?(?:async )?fn ([a-z_0-9]+)")


def _enclosing_fn(lines: List[str], i: int) -> str:
    for j in range(i, -1, -1):
        m = _FN.match(lines[j])
        if m:
            return m.group(1)
    return "(module)"


def _is_document_surface(rel: str) -> bool:
    return rel.startswith("canvas/") or rel.startswith("board/")


def scan_source(rel: str, src: str) -> SiteSets:
    """The pinned-site census of one file; `rel` is its path under `crates/designos-app/src/`."""
    sets: SiteSets = {k: {} for k in SET_IDS}

    def add(kind: str, fn: str, item: str) -> None:
        key = (rel, fn, item)
        sets[kind][key] = sets[kind].get(key, 0) + 1

    code = clean(src)
    code_lines, raw_lines = code.split("\n"), src.split("\n")
    document = _is_document_surface(rel)
    for i, (cl, rl) in enumerate(zip(code_lines, raw_lines)):
        if _PAINT.search(cl):
            for m in _UPPER_LITERAL.finditer(rl):
                if m.group(1) not in ACRONYM_ALLOWLIST:
                    add("ns1", _enclosing_fn(code_lines, i), m.group(1))
        if not document:
            for m in _MESH.finditer(cl):
                add("ns4", _enclosing_fn(code_lines, i), f"Mesh::{m.group(1)}()")
        for m in _MONO.finditer(cl):
            add("ns5", _enclosing_fn(code_lines, i), m.group(0).rstrip("("))
    if not document:
        for m in _NS2.finditer(code):
            add("ns2", _enclosing_fn(code_lines, code.count("\n", 0, m.start())), "BG_BRAND")
    return sets


def _is_test_source(rel: str) -> bool:
    parts = rel.split("/")
    return "tests" in parts[:-1] or parts[-1] == "tests.rs"


def scan_tree(app_src: Path) -> SiteSets:
    """The census of every non-test `.rs` file under `crates/designos-app/src/`."""
    total: SiteSets = {k: {} for k in SET_IDS}
    for path in sorted(app_src.rglob("*.rs")):
        rel = path.relative_to(app_src).as_posix()
        if _is_test_source(rel):
            continue
        for kind, found in scan_source(rel, path.read_text(encoding="utf-8", errors="replace")).items():
            for key, n in found.items():
                total[kind][key] = total[kind].get(key, 0) + n
    return total


def pins_as_sets(pins: Pins) -> SiteSets:
    return {kind: {(f, fn, item): n for f, fn, item, n, _note in rows} for kind, rows in pins.items()}


def compare(actual: SiteSets, pins: Optional[Pins] = None) -> List[str]:
    """Problems when a set differs from its pin; an empty list means every set equals its pin."""
    pinned = pins_as_sets(PINS if pins is None else pins)
    problems: List[str] = []
    for kind in SET_IDS:
        have, want = actual.get(kind, {}), pinned.get(kind, {})
        for key in sorted(set(have) | set(want)):
            file, fn, item = key
            where = f"{file} {fn}() {item!r}"
            if key not in want:
                problems.append(f"{kind} ({SET_TITLES[kind]}): new site {where} x{have[key]}; it is not in the pin")
            elif key not in have:
                problems.append(f"{kind} ({SET_TITLES[kind]}): {where} is gone; remove it from the pin (pins only shrink)")
            elif have[key] > want[key]:
                problems.append(f"{kind} ({SET_TITLES[kind]}): {where} rose from {want[key]} to {have[key]}")
            elif have[key] < want[key]:
                problems.append(
                    f"{kind} ({SET_TITLES[kind]}): {where} fell from {want[key]} to {have[key]}; lower the pin (pins only shrink)"
                )
    return problems


# (file, enclosing fn, item, count, note). Rescanned against the release head; see compare() for the rule.
PINS: Pins = {
    "ns1": [
        ('cmdk.rs', 'synthesize_audit_table', 'PASS', 2, 'document content written by the audit action'),
        ('showcase/kit.rs', 'avatar', 'AM', 1, 'showcase content (avatar initials)'),
        ('showcase/mobile.rs', 'welcome', 'ALEX MORGAN', 1, 'showcase content'),
        ('specs.rs', 'show_right', 'ON THIS PAGE', 1, 'debt (hidden reader)'),
    ],
    "ns2": [],
    "ns4": [
        ('home/main_view.rs', 'thumb_area', 'Mesh::default()', 1, 'orphan file, not in the module tree (home/mod.rs declares main_view inline)'),
        ('home/mod.rs', 'thumb_area', 'Mesh::default()', 1, 'content preview (board thumbnail dots)'),
        ('widgets/mod.rs', 'swatch_circle', 'Mesh::default()', 1, 'content preview (gradient swatch of a paint)'),
        ('widgets/picker.rs', 'gradient_bar', 'Mesh::default()', 1, 'colour picker content (gradients of the edited paint)'),
        ('widgets/picker.rs', 'hue_bar', 'Mesh::default()', 1, 'colour picker content (gradients of the edited paint)'),
        ('widgets/picker.rs', 'paint_swatch', 'Mesh::default()', 1, 'colour picker content (gradients of the edited paint)'),
        ('widgets/picker.rs', 'quad_mesh', 'Mesh::default()', 1, 'colour picker content (gradients of the edited paint)'),
        ('widgets/picker.rs', 'round_corners', 'Mesh::default()', 1, 'colour picker content (gradients of the edited paint)'),
        ('widgets/picker.rs', 'sv_area', 'Mesh::default()', 1, 'colour picker content (gradients of the edited paint)'),
    ],
    "ns5": [
        ('agent_approval_card.rs', 'paint_approval_card_for_tab', 'font_mono', 1, ''),
        ('agent_picker.rs', 'render_other_cli_inline_form', 'font_mono', 2, ''),
        ('agent_session_failure.rs', 'paint_banner', 'font_mono', 4, ''),
        ('agent_start_failure.rs', 'paint', 'font_mono', 1, ''),
        ('agent_turn_card.rs', 'draw_active_turn_card', 'font_mono', 2, ''),
        ('agent_turn_card.rs', 'draw_reverted_caption', 'font_mono', 1, ''),
        ('board/toolbar.rs', 'show_timer', 'font_mono', 1, ''),
        ('canvas/agent_changes_chip.rs', 'paint', 'font_mono', 1, ''),
        ('dev/code_view.rs', 'code_block', 'font_mono', 1, ''),
        ('dev/code_view.rs', 'mono_char_w', 'font_mono', 1, ''),
        ('dev/code_view.rs', 'prompt_box', 'font_mono', 1, ''),
        ('dev/list.rs', 'tabs', 'font_mono', 1, ''),
        ('home/dialog.rs', 'show', 'font_mono', 1, ''),
        ('image_jobs.rs', 'show_erase_popover', 'font_mono', 1, ''),
        ('image_jobs.rs', 'show_expand_popover', 'font_mono', 1, ''),
        ('panels/check_designs.rs', 'progress_row', 'FontFamily::Monospace', 1, ''),
        ('panels/inspect.rs', 'kv', 'font_mono', 1, ''),
        ('panels/left.rs', 'show', 'font_mono', 1, 'dev section count digits'),
        ('shell.rs', 'show_web_capture_report', 'font_mono', 1, ''),
        ('specs.rs', 'render_dev_view_tab', 'font_mono', 6, ''),
        ('specs.rs', 'render_gates_and_canvas_tab', 'font_mono', 3, ''),
        ('specs.rs', 'render_po_view_tab', 'font_mono', 9, ''),
        ('specs.rs', 'render_qa_coverage_tab', 'font_mono', 5, ''),
        ('specs.rs', 'render_wireframes_and_contract_tab', 'font_mono', 7, ''),
        ('specs.rs', 'show_left', 'font_mono', 2, ''),
        ('terminal.rs', 'paint_turn_receipt_row', 'font_mono', 1, ''),
        ('terminal.rs', 'render_vt100_viewport', 'FontFamily::Monospace', 1, ''),
        ('theme.rs', 'apply', 'font_mono', 1, ''),
        ('theme.rs', 'font_mono', 'FontFamily::Monospace', 1, ''),
        ('theme.rs', 'font_mono', 'font_mono', 1, ''),
        ('theme.rs', 'install_fonts', 'FontFamily::Monospace', 1, ''),
    ],
}
