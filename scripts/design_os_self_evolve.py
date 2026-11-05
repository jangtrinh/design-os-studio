#!/usr/bin/env python3
"""
design:os studio — Self-Evolution Engine & Live Token Bridge (`scripts/design_os_self_evolve.py`)
==================================================================================================
Orchestrates the closed-loop self-evolving design verification and pulse for `design:os studio`:

  1. G01 · `check_rust_token_parity`:
     Parses `crates/designos-app/src/theme.rs` for the 15 core hex constants (`BG_PRIMARY`,
     `BG_SECONDARY`, `BG_TERTIARY`, `BG_HOVER`, `BG_PRESSED`, `BORDER`, `BORDER_SUBTLE`,
     `BORDER_STRONG`, `BG_BRAND`, `TEXT_COMPONENT`, `COMPONENT`, `CANVAS_COMPONENT`,
     `DEV_READY`, `DEV_COMPLETED`, `DEV_CHANGED`) and verifies:
       - Zero Purple Ban violations (`#9747FF`, `#8755E9`, `#C4A1FF`, `#A855F7` never used on active UI/component tokens).
       - Exact hex parity against `design/design.tokens.json` (`colors.bg`, `colors.surface`,
         `colors.secondary`, `colors.bg-hover`, `colors.bg-active`, `colors.border`,
         `colors.border-subtle`, `colors.border-strong`, `colors.primary`, `colors.component`,
         `colors.success`).
       - When `--record-gaps` is enabled, automatically records any detected drift via
         `ui memory record gap`.

     The same gate also compares the non-colour tokens (radius, spacing, size) with their
     `theme.rs` constants through `scripts/non_colour_token_parity.py`.

     Every `Color32` constant in `theme.rs` (188 names) must fall in exactly one class: mapped to a store key
     (`RUST_TO_DESIGN_TOKEN_MAP`), sharing an existing key with the same value and role
     (`RUST_COLOUR_SHARED_KEY`), pending a new key (`RUST_COLOUR_PENDING_STORE`: Studio path and full pinned
     value, alpha included; the count may only fall) or exempt by design (`RUST_COLOUR_EXEMPT`: canvas, Dev
     overlay, content, identity, board). `scripts/check_ds_ontology_parity.py` reuses these classes (check P3).

  2. G02 · `zero_fake_ui_gate`:
     Executes `python3 scripts/zero_fake_ui_gate.py --json` and verifies every invariant passes (`passed == total`).

  3. G03 · `ui_ds_status`:
     Executes `ui ds status --dir . --json` and verifies compiled manifest + token & component registry health.

  4. G04 · `ui_ds_a11y`:
     Report-only. Runs `ui ds a11y` (default and `design/a11y-chrome-pairs.txt`), prints the counts as supplementary evidence, and always returns passed.

  5. G05 · `ui_ds_soul_check`:
     Executes `ui ds soul check --dir . --json` and verifies `0` findings (`errorCount == 0`, `warningCount == 0`)
     and `status: ratified` in `design/soul.md`.

  6. G06 · `ui_agents_check`:
     Executes `ui agents check --dir . --json` and verifies all 3 stamped roster agents (`designer`, `curator`,
     `figma-hand`) under `.claude/agents/` + all 4 cross-runtime agents under `.agents/agents/` have `0` errors.

  7. G07 · `self_evolution_pulse` (`design-os harvest` + `design-os heartbeat` + `ui memory compile` + `design-os evolution`):
     In default / `--evolve` mode, executes the full harvest -> heartbeat -> memory compile pulse, then runs
     `design-os evolution --dir . --json` and verifies:
       - `verdict == "ALIVE"`
       - `proof.level in ("APPLIED", "IMPROVING")`
       - `len(proof.findings) == 0`
     In `--check` mode, skips state-mutating harvest/heartbeat/compile steps and evaluates `design-os evolution` read-only.

  8. G08 · `ui_gate_specimen`:
     Executes `ui gate design/specimen.html --tokens design/design.tokens.json --json` and
     `ui ds-usage-lint design/specimen.html --dir . --json`, verifying `exit_code == 0` and `0` errors
     across all 6 gate families (`layout`, `a11y`, `taste`, `tell`, `content`, `autofix`) plus `0`
     token-usage errors.

  9. G09 · `design_os_project_audit`:
     Executes `design-os audit` over every HTML file in the repository except the two exclusions below, plus one
     repo-level run (`--dir .`) for the DS specimen / DS a11y / flow-lint sections, plus
     `ui audit design/studio-ui-nodes.json` and `ui flow lint design/flow.json` when present, verifying `0`
     errors across everything audited.
     `design-os audit TARGET` takes one file or one directory and has no ignore option, so the gate walks the
     HTML itself with the CLI's own walk (`rglob("*.html")`, skipping node_modules / .git / dist / build / .next /
     out) and audits file by file; the DS/flow sections run exactly once. `toolsRun`, `toolsFailed`, `errors` and
     `warnings` are summed over the runs; the gate passes only when every run exits 0 and reports ok, with
     `toolsRun > 0`, `toolsFailed == 0` and `errors == 0`.
     Excluded (owner rulings 2026-10-22), and nothing else:
       - `skills/vendor/**`: SHA-pinned third-party upstream files that must stay byte-identical
         (`scripts/check_vendor_skills.py verify`), so lint findings there cannot be fixed in this repository.
       - `crates/designos-webcapture/tests/playwright/fixtures/stacking.html`: it tests capture of the maximum
         32-bit `z-index: 2147483647`, and its test (`paint_index_dense_and_stacking_aware`) asserts that value,
         which taste-lint reports as `z-index-inflation`. Changing the value would defeat the test.
     Every other HTML file stays audited, the other test fixtures included.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from non_colour_token_parity import check_non_colour_parity

REPO_ROOT = Path(__file__).resolve().parent.parent
THEME_RS = REPO_ROOT / "crates" / "designos-app" / "src" / "theme.rs"
TOKENS_JSON = REPO_ROOT / "design" / "design.tokens.json"
REGISTRY_JSON = REPO_ROOT / "design" / "component-registry.json"
SPECIMEN_HTML = REPO_ROOT / "design" / "specimen.html"
STUDIO_UI_NODES_JSON = REPO_ROOT / "design" / "studio-ui-nodes.json"
FLOW_JSON = REPO_ROOT / "design" / "flow.json"
SOUL_MD = REPO_ROOT / "design" / "soul.md"
ZERO_FAKE_GATE = REPO_ROOT / "scripts" / "zero_fake_ui_gate.py"

# G09 audit scope. Mirror of `design_os.commands.audit._EXCLUDED_SEGMENTS` (the CLI's own HTML walk).
AUDIT_WALK_EXCLUDED_SEGMENTS = frozenset({"node_modules", ".git", "dist", "build", ".next", "out"})
# Exclusion 1, a subtree: SHA-pinned MIT upstream, kept byte-identical (owner ruling 2026-10-22).
AUDIT_EXCLUDED_PREFIX: Tuple[str, ...] = ("skills", "vendor")
# Exclusion 2, one file (owner ruling 2026-10-22). It tests capture of the maximum 32-bit z-index
# (`z-index: 2147483647`) and its test asserts that value (`paint_index_dense_and_stacking_aware` in
# run_capture_tests.mjs), which taste-lint reports as `z-index-inflation`; lowering it would defeat the test.
AUDIT_EXCLUDED_FILES = frozenset({"crates/designos-webcapture/tests/playwright/fixtures/stacking.html"})
AUDIT_EXCLUSION_SUMMARY = " and ".join(
    [f"{'/'.join(AUDIT_EXCLUDED_PREFIX)}/**", *(f"{Path(f).parent.name}/{Path(f).name}" for f in sorted(AUDIT_EXCLUDED_FILES))]
)

BANNED_PURPLES = {
    "#9747ff",
    "#8755e9",
    "#c4a1ff",
    "#a855f7",
    "#a259ff",
    "#c792ea",
    "#7f56d9",
    "#9e77ed",
    "#b692f6",
    "#6941c6",
}

CORE_RUST_CONSTANTS = [
    "BG_PRIMARY",
    "BG_SECONDARY",
    "BG_TERTIARY",
    "BG_HOVER",
    "BG_PRESSED",
    "BORDER",
    "BORDER_SUBTLE",
    "BORDER_STRONG",
    "BG_BRAND",
    "TEXT_COMPONENT",
    "COMPONENT",
    "CANVAS_COMPONENT",
    "DEV_READY",
    "DEV_COMPLETED",
    "DEV_CHANGED",
    "AGENT_1",
    "AGENT_2",
    "AGENT_3",
    "AGENT_4",
    "INFO",
    "SUCCESS",
    "WARNING",
    "BG_SELECTED",
    "BG",
    "TEXT",
    "TEXT_TERTIARY",
    "BG_BRAND_HOVER",
    "BG_BRAND_PRESSED",
    "TEXT_ON_BRAND",
    "FOCUS_RING",
    "BORDER_SELECTED",
    "BG_TEXT_SELECTION",
    "TEXT_BRAND",
    "BG_MENU",
    "BG_ERROR_SOLID",
]

# Maps Rust `theme.rs` constants to canonical `design/design.tokens.json` paths under `colors.<key>`
RUST_TO_DESIGN_TOKEN_MAP: Dict[str, str] = {
    "BG_PRIMARY": "bg",
    "BG_SECONDARY": "surface",
    "BG_TERTIARY": "secondary",
    "BG_HOVER": "bg-hover",
    "BG_PRESSED": "secondary-active",
    "BORDER": "border",
    "BORDER_SUBTLE": "border-subtle",
    "BORDER_STRONG": "border-strong",
    "BG_BRAND": "primary",
    "COMPONENT": "component",
    "CANVAS_COMPONENT": "component",
    "DEV_COMPLETED": "component",
    "AGENT_1": "agent-1",
    "AGENT_2": "agent-2",
    "AGENT_3": "agent-3",
    "AGENT_4": "agent-4",
    "INFO": "info",
    "SUCCESS": "success",
    "WARNING": "warning",
    "BG_SELECTED": "bg-active",
    "BG": "canvas",
    "TEXT": "bg-foreground",
    "TEXT_TERTIARY": "muted-foreground",
    "BG_BRAND_HOVER": "primary-hover",
    "BG_BRAND_PRESSED": "primary-active",
    "TEXT_ON_BRAND": "primary-foreground",
    "FOCUS_RING": "ring",
    "BORDER_SELECTED": "border-focus",
    "BG_TEXT_SELECTION": "accent",
    "TEXT_BRAND": "accent-foreground",
    "BG_MENU": "popover",
    "BG_ERROR_SOLID": "destructive",
}

EXPECTED_STANDALONE_HEX: Dict[str, str] = {
    "TEXT_COMPONENT": "#22d3ee",
    "DEV_READY": "#14ae5c",
    "DEV_CHANGED": "#ffa629",
}

# Studio colour names that share a store key: the key already holds the same value and role (key under `colors`).
RUST_COLOUR_SHARED_KEY: Dict[str, str] = {
    "AGENT_LABEL_TEXT": "primary-foreground",
    "BG_MENU_HOVER": "bg-hover",
    "BG_MENU_SELECTED": "bg-active",
    "BG_SECONDARY_HOVER": "surface-hover",
    "BG_SELECTED_HOVER": "bg-active",
    "BG_SELECTED_SECONDARY": "surface",
    "BG_TOOLBAR": "bg",
    "BG_TOOLBAR_HOVER": "bg-hover",
    "BG_TOOLBAR_MODE_SWITCHER": "surface",
    "BG_TOOLBAR_SELECTED": "bg-active",
    "DROP_INDICATOR": "primary",
    "HOME_THUMB_BG": "secondary",
    "ICON_QUATERNARY": "neutral",
    "PRESENT_BG": "bg",
    "STATUS_INFO_TEXT": "info",
    "SWATCH_PLACEHOLDER": "secondary-active",
    "SYNTAX_PLAIN": "bg-foreground",
    "TEXT_ON_LIGHT_CANVAS": "primary-foreground",
    "TEXT_WHITE": "destructive-foreground",
    "TILE_SPEC_BG": "accent",
    "VAR_SCROLL_THUMB": "neutral",
}

# Colour names with no store key yet: name -> (Studio DTCG path, full expected value, #RRGGBB or #RRGGBBAA).
# The pending count may only fall; the target is zero.
PENDING_STORE_MAX = 68
RUST_COLOUR_PENDING_STORE: Dict[str, Tuple[str, str]] = {
    "BADGE_ERROR_BORDER": ("colors.badge-error-border", "#991B1B"),
    "BADGE_ERROR_TEXT": ("colors.badge-error-text", "#FCA5A5"),
    "BADGE_NEUTRAL_BG": ("colors.badge-neutral-bg", "#171717"),
    "BADGE_NEUTRAL_BORDER": ("colors.badge-neutral-border", "#404040"),
    "BADGE_NEUTRAL_TEXT": ("colors.badge-neutral-text", "#D4D4D4"),
    "BADGE_SUCCESS_BORDER": ("colors.badge-success-border", "#166534"),
    "BADGE_SUCCESS_TEXT": ("colors.badge-success-text", "#86EFAC"),
    "BADGE_WARNING_BORDER": ("colors.badge-warning-border", "#854D0E"),
    "BADGE_WARNING_TEXT": ("colors.badge-warning-text", "#FDE047"),
    "BG_ERROR_PRIMARY": ("colors.bg-error-primary", "#450A0A"),
    "BG_ERROR_SOLID_HOVER": ("colors.bg-error-solid-hover", "#B91C1C"),
    "BG_ERROR_SOLID_PRESSED": ("colors.bg-error-solid-pressed", "#991B1B"),
    "BG_OVERLAY": ("colors.bg-overlay", "#262626B3"),
    "BG_PRIMARY_ALT": ("colors.bg-primary-alt", "#171717"),
    "BG_PRIMARY_SOLID": ("colors.bg-primary-solid", "#262626"),
    "BG_SUCCESS_PRIMARY": ("colors.bg-success-primary", "#052E16"),
    "BG_TRANSPARENT_HOVER": ("colors.bg-transparent-hover", "#FFFFFF1A"),
    "BG_WARNING_PRIMARY": ("colors.bg-warning-primary", "#422006"),
    "BORDER_ERROR": ("colors.border-error", "#F87171"),
    "BORDER_ERROR_SUBTLE": ("colors.border-error-subtle", "#EF4444"),
    "BORDER_MENU": ("colors.border-menu", "#262626"),
    "BORDER_TRANSLUCENT": ("colors.border-translucent", "#FFFFFF1A"),
    "DANGER": ("colors.danger", "#EF4444"),
    "ELEVATION_400_RING": ("colors.elevation-400-ring", "#262626"),
    "ELEVATION_400_SHADOW": ("colors.elevation-400-shadow", "#00000000"),
    "FIND_HIGHLIGHT": ("colors.find-highlight", "#0D99FF59"),
    "FIND_HIGHLIGHT_SELECTED": ("colors.find-highlight-selected", "#7CC4F866"),
    "FOCUS_RING_ERROR": ("colors.focus-ring-error", "#EF4444"),
    "ICON": ("colors.icon", "#FFFFFF"),
    "ICON_BRAND_PRIMARY": ("colors.icon-brand-primary", "#F97316"),
    "ICON_QUATERNARY_HOVER": ("colors.icon-quaternary-hover", "#A3A3A3"),
    "ICON_SECONDARY": ("colors.icon-secondary", "#D4D4D4"),
    "ICON_SECONDARY_HOVER": ("colors.icon-secondary-hover", "#E5E5E5"),
    "ICON_SELECTED": ("colors.icon-selected", "#FFFFFF"),
    "ICON_TERTIARY": ("colors.icon-tertiary", "#A3A3A3"),
    "ICON_WHITE": ("colors.icon-white", "#FFFFFF"),
    "INNER_HIGHLIGHT": ("colors.inner-highlight", "#262626"),
    "MENU_SHORTCUT_HIGHLIGHT": ("colors.menu-shortcut-highlight", "#E5E5E5"),
    "SHADOW_COLOR": ("colors.shadow-color", "#00000000"),
    "SHADOW_COLOR_STRONG": ("colors.shadow-color-strong", "#00000000"),
    "STATUS_INFO_BG": ("colors.status-info-bg", "#0D99FF24"),
    "STATUS_INFO_BORDER": ("colors.status-info-border", "#0D99FF59"),
    "SYNTAX_COMMENT": ("colors.syntax-comment", "#A3A3A3"),
    "SYNTAX_FUNCTION": ("colors.syntax-function", "#60A5FA"),
    "SYNTAX_KEYWORD": ("colors.syntax-keyword", "#F472B6"),
    "SYNTAX_NUMBER": ("colors.syntax-number", "#FB923C"),
    "SYNTAX_PROPERTY": ("colors.syntax-property", "#7DD3FC"),
    "SYNTAX_PUNCT": ("colors.syntax-punct", "#D4D4D4"),
    "SYNTAX_STRING": ("colors.syntax-string", "#4ADE80"),
    "SYNTAX_TYPE": ("colors.syntax-type", "#FACC15"),
    "TEXT_COMPONENT": ("colors.text-component", "#22D3EE"),
    "TEXT_DISABLED": ("colors.text-disabled", "#D4D4D480"),
    "TEXT_ERROR": ("colors.text-error", "#F87171"),
    "TEXT_ERROR_HOVER": ("colors.text-error-hover", "#FCA5A5"),
    "TEXT_PLACEHOLDER": ("colors.text-placeholder", "#737373"),
    "TEXT_QUATERNARY": ("colors.text-quaternary", "#A3A3A3"),
    "TEXT_SECONDARY": ("colors.text-secondary", "#D4D4D4"),
    "TEXT_SECONDARY_HOVER": ("colors.text-secondary-hover", "#E5E5E5"),
    "TEXT_SUCCESS": ("colors.text-success", "#4ADE80"),
    "TEXT_TERTIARY_HOVER": ("colors.text-tertiary-hover", "#D4D4D4"),
    "TEXT_WARNING": ("colors.text-warning", "#FACC15"),
    "TILE_BOARD_BG": ("colors.tile-board-bg", "#451A03"),
    "TILE_BOARD_GLYPH": ("colors.tile-board-glyph", "#FBBF24"),
    "TILE_DECK_BG": ("colors.tile-deck-bg", "#052E16"),
    "TILE_DECK_GLYPH": ("colors.tile-deck-glyph", "#4ADE80"),
    "TILE_DESIGN_BG": ("colors.tile-design-bg", "#172554"),
    "TILE_DESIGN_GLYPH": ("colors.tile-design-glyph", "#60A5FA"),
    "TILE_SPEC_GLYPH": ("colors.tile-spec-glyph", "#FB923C"),
}

EXEMPT_CANVAS = "canvas, Dev overlay, content, identity, board"
EXEMPT_READER = "spec-reader light content"
EXEMPT_ALIAS = "alias of BG_BRAND"

# Colour names with no design-store token by design: name -> reason class.
RUST_COLOUR_EXEMPT: Dict[str, str] = {
    "AGENT_COLORS": EXEMPT_CANVAS,
    "ANNOTATION_BG": EXEMPT_CANVAS,
    "ANNOTATION_LINE": EXEMPT_CANVAS,
    "ANNOTATION_TEXT": EXEMPT_CANVAS,
    "AVATAR_COLORS": EXEMPT_CANVAS,
    "BG_SELECTED_AREA": EXEMPT_CANVAS,
    "BOARD_DOT": EXEMPT_CANVAS,
    "BOX_CONTENT": EXEMPT_CANVAS,
    "BOX_MARGIN": EXEMPT_CANVAS,
    "BOX_PADDING": EXEMPT_CANVAS,
    "BOX_TEXT": EXEMPT_CANVAS,
    "CANVAS_HANDLE_FILL": EXEMPT_CANVAS,
    "CANVAS_LABEL": EXEMPT_CANVAS,
    "CANVAS_LABEL_DARK": EXEMPT_CANVAS,
    "CANVAS_MARQUEE_FILL": EXEMPT_CANVAS,
    "CANVAS_SELECTION": EXEMPT_CANVAS,
    "CANVAS_SIZE_BADGE_TEXT": EXEMPT_CANVAS,
    "CHECKER_A": EXEMPT_CANVAS,
    "CHECKER_B": EXEMPT_CANVAS,
    "COMMENT_PIN_RESOLVED": EXEMPT_CANVAS,
    "CONTENT_HANDLE_AXIS": EXEMPT_CANVAS,
    "CONTENT_HANDLE_CASING": EXEMPT_CANVAS,
    "CONTENT_HANDLE_FILL": EXEMPT_CANVAS,
    "CONTENT_HANDLE_RING": EXEMPT_CANVAS,
    "CONTENT_HANDLE_STOP_IDLE": EXEMPT_CANVAS,
    "CONTENT_MARK_LIGHT": EXEMPT_CANVAS,
    "CONTENT_MARK_SHADOW": EXEMPT_CANVAS,
    "DEV_CHANGED": EXEMPT_CANVAS,
    "DEV_READY": EXEMPT_CANVAS,
    "DEV_READY_HOVER": EXEMPT_CANVAS,
    "ELEVATION_400_TOP": EXEMPT_CANVAS,
    "FIND_CANVAS_MATCH": EXEMPT_CANVAS,
    "FIND_CANVAS_MATCH_CURRENT": EXEMPT_ALIAS,
    "GUIDE_COLOR": EXEMPT_CANVAS,
    "GUIDE_TEXT": EXEMPT_CANVAS,
    "HUD_POINTER_FILL": EXEMPT_CANVAS,
    "MEASURE": EXEMPT_CANVAS,
    "PEN_POINT": EXEMPT_CANVAS,
    "PIXEL_GRID_DARK_LINES": EXEMPT_CANVAS,
    "PIXEL_GRID_LIGHT_LINES": EXEMPT_CANVAS,
    "PLAYGROUND_GRID_DOT": EXEMPT_CANVAS,
    "PROTO_BLUE": EXEMPT_ALIAS,
    "PROVIDER_CLAUDE": EXEMPT_CANVAS,
    "PROVIDER_CODEX": EXEMPT_CANVAS,
    "PROVIDER_GEMINI": EXEMPT_CANVAS,
    "PROVIDER_GROK": EXEMPT_CANVAS,
    "READER_LIGHT_BG_PAGE": EXEMPT_READER,
    "READER_LIGHT_BORDER": EXEMPT_READER,
    "READER_LIGHT_CODE_BG": EXEMPT_READER,
    "READER_LIGHT_DOC_SHEET": EXEMPT_READER,
    "READER_LIGHT_DOT_NEUTRAL": EXEMPT_READER,
    "READER_LIGHT_DOT_OK": EXEMPT_READER,
    "READER_LIGHT_DOT_WARN": EXEMPT_READER,
    "READER_LIGHT_HAIRLINE": EXEMPT_READER,
    "READER_LIGHT_HIGHLIGHT": EXEMPT_READER,
    "READER_LIGHT_SURFACE": EXEMPT_READER,
    "READER_LIGHT_SURFACE_2": EXEMPT_READER,
    "READER_LIGHT_SURFACE_3": EXEMPT_READER,
    "READER_LIGHT_TEXT": EXEMPT_READER,
    "READER_LIGHT_TEXT_MUTED": EXEMPT_READER,
    "READER_LIGHT_TEXT_SUBTLE": EXEMPT_READER,
    "RULER_GUIDE": EXEMPT_CANVAS,
    "RULER_SELECTION_FILL": EXEMPT_CANVAS,
    "RULER_TEXT": EXEMPT_CANVAS,
    "RULER_TICK": EXEMPT_CANVAS,
    "TEXT_SELECTION": EXEMPT_CANVAS,
    "TYPE_PREVIEW_BASELINE": EXEMPT_CANVAS,
}

CROSS_RUNTIME_AGENTS = [
    "designer-jang-designos-studio.md",
    "curator-jang-designos-studio.md",
    "figma-jang-designos-studio.md",
    "librarian-jang-designos-studio.md",
]


def build_env() -> Dict[str, str]:
    env = os.environ.copy()
    home = str(Path.home())
    extra_paths = [
        f"{home}/.local/bin",
        f"{home}/.cargo/bin",
        "/opt/homebrew/bin",
        "/usr/local/bin",
    ]
    current_path = env.get("PATH", "")
    env["PATH"] = ":".join(extra_paths + ([current_path] if current_path else []))
    return env


def run_cli(cmd: List[str], cwd: Path = REPO_ROOT) -> Tuple[int, str, str]:
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(cwd),
            env=build_env(),
            capture_output=True,
            text=True,
        )
    except OSError as exc:  # e.g. the `ui` CLI is not installed on a CI runner
        return 127, "", f"{cmd[0]} not runnable: {exc}"
    return proc.returncode, proc.stdout.strip(), proc.stderr.strip()


def run_cli_json(cmd: List[str], cwd: Path = REPO_ROOT) -> Tuple[int, Dict[str, Any], str]:
    code, out, err = run_cli(cmd, cwd=cwd)
    if not out:
        return code, {}, err
    try:
        return code, json.loads(out), err
    except json.JSONDecodeError:
        # Try parsing last JSON line if stdout had prefix logs
        for line in reversed(out.splitlines()):
            line = line.strip()
            if line.startswith("{") and line.endswith("}"):
                try:
                    return code, json.loads(line), err
                except json.JSONDecodeError:
                    pass
        return code, {}, err or out


def record_gap_event(message: str, target: str = "crates/designos-app/src/theme.rs") -> None:
    payload = json.dumps({"text": message, "target": target})
    run_cli(["ui", "memory", "record", "gap", "--dir", ".", "--data", payload])


def parse_rust_theme_constants(theme_src: str) -> Dict[str, str]:
    """Extracts `pub const <NAME>: Color32 = rgb(0xRRGGBB);` into lowercase `#rrggbb` strings."""
    pattern = re.compile(
        r"pub\s+const\s+([A-Z0-9_]+)\s*:\s*Color32\s*=\s*rgb\(\s*0x([0-9a-fA-F]{6})\s*\)\s*;"
    )
    extracted: Dict[str, str] = {}
    for match in pattern.finditer(theme_src):
        name = match.group(1)
        hex_val = f"#{match.group(2).lower()}"
        extracted[name] = hex_val
    return extracted


def extract_design_token_colors(tokens_data: Dict[str, Any]) -> Dict[str, str]:
    colors_obj = tokens_data.get("colors", {})
    out: Dict[str, str] = {}
    if isinstance(colors_obj, dict):
        for k, v in colors_obj.items():
            if isinstance(v, dict) and "$value" in v:
                out[k] = str(v["$value"]).strip().lower()
            elif isinstance(v, str):
                out[k] = v.strip().lower()
    return out


def purple_band_constants(theme_src: str) -> List[str]:
    """Every Color32 constant except BOARD_*: an rgb()/rgba() literal with hue 255-300 deg and HSL saturation > 0.25."""
    import colorsys
    hits: List[str] = []
    for name, hx in re.findall(r"^pub const ([A-Z0-9_]+): Color32 = rgba?\(0x([0-9A-Fa-f]{6})", theme_src, re.M):
        if name.startswith("BOARD_"):
            continue
        r, g, b = (int(hx[i:i + 2], 16) / 255 for i in (0, 2, 4))
        h, _, sat = colorsys.rgb_to_hls(r, g, b)
        if 255 <= h * 360 <= 300 and sat > 0.25:
            hits.append(f"{name} #{hx.upper()} hue {h * 360:.0f} deg, HSL S {sat:.2f}")
    return hits


_COLOUR_CONST_RE = re.compile(r"^pub const ([A-Z0-9_]+): (Color32|\[Color32; \d+\]) = (.+?);\s*$", re.M)
# Every colour constant the file declares, however it is written; the reader above parses only the one-line
# form with nothing after the `;`, so a declaration found here that the reader skipped is reported, not ignored.
_COLOUR_DECL_RE = re.compile(r"^pub const ([A-Z0-9_]+)\s*:\s*\[?Color32", re.M)
_RGB_RE = re.compile(r"rgb\(\s*0x([0-9A-Fa-f]{6})\s*\)")
_RGBA_RE = re.compile(r"rgba\(\s*0x([0-9A-Fa-f]{6})\s*,\s*0x([0-9A-Fa-f]{2})\s*\)")


def parse_theme_colour_values(theme_src: str) -> Dict[str, Optional[str]]:
    """Every `pub const NAME: Color32` (or `[Color32; N]`) in `theme.rs`, in file order, as its full value.

    `rgb(0xRRGGBB)` reads as `#RRGGBB`, `rgba(0xRRGGBB, 0xAA)` as `#RRGGBBAA`, and `= NAME` follows the alias
    to its target. An array or any other expression reads as None: the name is listed, its value is not compared.
    """
    exprs: Dict[str, Tuple[str, str]] = {m.group(1): (m.group(2), m.group(3).strip()) for m in _COLOUR_CONST_RE.finditer(theme_src)}

    def resolve(name: str, seen: Tuple[str, ...] = ()) -> Optional[str]:
        kind, expr = exprs[name]
        if kind != "Color32":
            return None
        if m := _RGB_RE.fullmatch(expr):
            return f"#{m.group(1).upper()}"
        if m := _RGBA_RE.fullmatch(expr):
            return f"#{m.group(1).upper()}{m.group(2).upper()}"
        if expr in exprs and name not in seen:
            return resolve(expr, seen + (name,))
        return None

    return {name: resolve(name) for name in exprs}


def check_colour_classes(theme_src: str, ds_colors: Dict[str, str]) -> Dict[str, Any]:
    """Every theme.rs colour name falls in exactly one declared class; mapped and shared names equal their
    store key, pending names equal their pinned full value (alpha included) while they wait for a store key.

    `ds_colors` maps a bare `colors.<key>` key to its value, as `extract_design_token_colors` returns it.
    """
    values = parse_theme_colour_values(theme_src)
    declared = set(_COLOUR_DECL_RE.findall(theme_src))
    classes: Dict[str, set] = {
        "mapped": set(RUST_TO_DESIGN_TOKEN_MAP),
        "shared-key": set(RUST_COLOUR_SHARED_KEY),
        "pending": set(RUST_COLOUR_PENDING_STORE),
        "exempt": set(RUST_COLOUR_EXEMPT),
    }
    problems: List[str] = []
    order = list(classes)
    for i, a in enumerate(order):
        for b in order[i + 1:]:
            for name in sorted(classes[a] & classes[b]):
                problems.append(f"{name} is declared in two colour classes ({a} and {b})")
    for name in values:
        if not any(name in members for members in classes.values()):
            problems.append(
                f"{name} is in no colour class: map it in RUST_TO_DESIGN_TOKEN_MAP, or declare it in "
                "RUST_COLOUR_SHARED_KEY, RUST_COLOUR_PENDING_STORE or RUST_COLOUR_EXEMPT"
            )
    for name in sorted(declared - set(values)):
        problems.append(
            f"{name} is unclassified: its declaration is not one line ending in `;` (a trailing comment or a wrapped "
            "value), so the colour reader skipped it and no class checks it; write `pub const NAME: Color32 = rgb(0x......);` on one line"
        )
    for cls, members in classes.items():
        for name in sorted(members - set(values) - declared):
            problems.append(f"{name} is declared {cls} but theme.rs has no such colour constant")

    def compare(name: str, key: str) -> None:
        store = ds_colors.get(key)
        theme = values.get(name)
        if store is None:
            problems.append(f"{name}: colors.{key} is missing in design.tokens.json")
        elif theme is None:
            problems.append(f"{name}: its theme.rs value is not a readable colour")
        elif store.upper() != theme:
            problems.append(f"{name} ({theme}) != colors.{key} ({store.upper()})")

    for name, key in RUST_TO_DESIGN_TOKEN_MAP.items():
        if name in values:
            compare(name, key)
    for name, key in RUST_COLOUR_SHARED_KEY.items():
        if name in values:
            compare(name, key)
    for name, (path, pin) in RUST_COLOUR_PENDING_STORE.items():
        if name not in values:
            continue
        if values[name] != pin:
            problems.append(f"{name}: theme.rs {values[name]} != pinned {pin} (alpha included); pending names cannot drift")
        if path.split(".", 1)[1] in ds_colors:
            problems.append(f"{name} now has the store key {path}: move it from pending to mapped")
    if len(RUST_COLOUR_PENDING_STORE) > PENDING_STORE_MAX:
        problems.append(f"pending count {len(RUST_COLOUR_PENDING_STORE)} rose above {PENDING_STORE_MAX}; it may only fall")
    known_reasons = {EXEMPT_CANVAS, EXEMPT_READER, EXEMPT_ALIAS}
    for name, reason in RUST_COLOUR_EXEMPT.items():
        if reason not in known_reasons:
            problems.append(f"{name}: unknown exempt reason {reason!r}")
    counts = {cls: len(members) for cls, members in classes.items()}
    return {"counts": counts, "total": len(values), "declared": len(declared), "problems": problems}


def check_rust_token_parity(record_gaps: bool = False) -> Tuple[bool, str, Dict[str, Any]]:
    if not THEME_RS.exists():
        msg = f"Missing Rust theme file: {THEME_RS}"
        if record_gaps:
            record_gap_event(msg)
        return False, msg, {}
    if not TOKENS_JSON.exists():
        msg = f"Missing design tokens file: {TOKENS_JSON}"
        if record_gaps:
            record_gap_event(msg, target="design/design.tokens.json")
        return False, msg, {}

    theme_src = THEME_RS.read_text(encoding="utf-8", errors="replace")
    rust_consts = parse_rust_theme_constants(theme_src)
    tokens_data = json.loads(TOKENS_JSON.read_text(encoding="utf-8"))
    ds_colors = extract_design_token_colors(tokens_data)

    missing_consts: List[str] = []
    purple_violations: List[str] = []
    token_drifts: List[str] = []
    verified_pairs: Dict[str, str] = {}

    for cname in CORE_RUST_CONSTANTS:
        actual_hex = rust_consts.get(cname)
        if not actual_hex:
            missing_consts.append(cname)
            continue
        if actual_hex in BANNED_PURPLES:
            purple_violations.append(f"{cname}={actual_hex}")
        if cname in RUST_TO_DESIGN_TOKEN_MAP:
            ds_key = RUST_TO_DESIGN_TOKEN_MAP[cname]
            expected_hex = ds_colors.get(ds_key)
            if not expected_hex:
                token_drifts.append(f"colors.{ds_key} missing in design.tokens.json")
            elif actual_hex != expected_hex:
                token_drifts.append(f"{cname} ({actual_hex}) != colors.{ds_key} ({expected_hex})")
            else:
                verified_pairs[cname] = f"{actual_hex} == colors.{ds_key}"
        elif cname in EXPECTED_STANDALONE_HEX:
            expected_hex = EXPECTED_STANDALONE_HEX[cname]
            if actual_hex != expected_hex:
                token_drifts.append(f"{cname} ({actual_hex}) != expected ({expected_hex})")
            else:
                verified_pairs[cname] = actual_hex

    band_hits = purple_band_constants(theme_src)
    for hit in band_hits:
        print(f"Purple Ban (hue scan): {hit}", file=sys.stderr)
        purple_violations.append(hit)
    print(f"purple-band hits: {len(band_hits)}", file=sys.stderr)

    # Also verify no banned purple exists anywhere in theme.rs or design/design.tokens.json
    for r_name, r_hex in rust_consts.items():
        if r_hex in BANNED_PURPLES and f"{r_name}={r_hex}" not in purple_violations:
            purple_violations.append(f"{r_name}={r_hex}")
    for ds_key, ds_hex in ds_colors.items():
        if ds_hex in BANNED_PURPLES:
            purple_violations.append(f"colors.{ds_key}={ds_hex}")

    non_colour = check_non_colour_parity(theme_src, tokens_data)
    non_colour_issues = (
        non_colour["drifts"] + non_colour["stale_exemptions"] + non_colour["problems"]
    )
    token_drifts.extend(non_colour_issues)

    colour_classes = check_colour_classes(theme_src, ds_colors)
    token_drifts.extend(colour_classes["problems"])

    ok = (not missing_consts) and (not purple_violations) and (not token_drifts)
    details = {
        "colour_classes": colour_classes,
        "non_colour": non_colour,
        "checked_constants": len(CORE_RUST_CONSTANTS),
        "verified_pairs": verified_pairs,
        "missing_constants": missing_consts,
        "purple_violations": purple_violations,
        "token_drifts": token_drifts,
    }
    if ok:
        summary = (
            f"{len(CORE_RUST_CONSTANTS)}/{len(CORE_RUST_CONSTANTS)} Rust theme.rs constants match "
            f"design.tokens.json; 0 Purple Ban violations; "
            f"purple-band hits: {len(band_hits)}; "
            f"{len(non_colour['verified'])} non-colour constants match; "
            f"colour classes cover {colour_classes['total']} theme.rs names "
            f"({colour_classes['counts']['mapped']} mapped, {colour_classes['counts']['shared-key']} shared-key, "
            f"{colour_classes['counts']['pending']} pending, {colour_classes['counts']['exempt']} exempt; "
            f"pending max {PENDING_STORE_MAX})"
        )
        if non_colour["known_drift"]:
            summary += f"; KNOWN DRIFT x{len(non_colour['known_drift'])}: " + " | ".join(
                non_colour["known_drift"]
            )
        return True, summary, details

    issues: List[str] = []
    if missing_consts:
        issues.append(f"missing={missing_consts}")
    if purple_violations:
        issues.append(f"purple_ban_violations={purple_violations}")
    if token_drifts:
        issues.append(f"drifts={token_drifts}")
    summary = "; ".join(issues)
    if record_gaps:
        record_gap_event(
            f"Rust theme.rs <-> design.tokens.json parity violation: {summary}",
            target="crates/designos-app/src/theme.rs",
        )
    return False, summary, details


def check_zero_fake_ui_gate() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json([sys.executable, str(ZERO_FAKE_GATE), "--json"])
    passed = payload.get("passed", 0)
    total = payload.get("total", 0)
    status = payload.get("status", "FAIL")
    ok = (code == 0) and (status == "PASS") and (total > 0) and (passed == total)
    summary = f"{passed}/{total} Zero-Fake-UI invariants PASS (DIR-28 / BR-31 / REQ-25)"
    if not ok and err:
        summary = f"{summary} ({err})"
    return ok, summary, payload


def check_ui_ds_status() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json(["ui", "ds", "status", "--dir", ".", "--json"])
    ok = (code == 0) and bool(payload.get("ok"))
    data = payload.get("data", {})
    if ok:
        comp_count = data.get("componentCount", 0)
        compiled_hash = str(data.get("compiledHash", ""))[:19]
        summary = f"DS manifest active ({comp_count} registered components, hash={compiled_hash}…)"
    else:
        err_msg = payload.get("error", {}).get("message") or err or "ui ds status failed"
        summary = f"DS status error: {err_msg}"
    return ok, summary, payload


def check_ui_ds_a11y() -> Tuple[bool, str, Dict[str, Any]]:
    """Report-only. Runs the default audit and the chrome pairs file, then always returns passed."""
    code, payload, err = run_cli_json(["ui", "ds", "a11y", "--dir", ".", "--json"])
    data = payload.get("data", {}) if isinstance(payload, dict) else {}
    failures = data.get("failures", []) or []
    unresolved = data.get("unresolved", []) or []
    pairs = data.get("checkedPairs", 0)
    state_pairs = data.get("checkedStatePairs", 0)

    pairs_rel = "design/a11y-chrome-pairs.txt"
    pairs_file = REPO_ROOT / pairs_rel
    chrome_pairs = 0
    chrome_failures: List[Any] = []
    chrome_unresolved: List[Any] = []
    notes: List[str] = []
    if err:
        notes.append(str(err))
    if code != 0:
        notes.append(f"default a11y exit {code}")
    if not pairs_file.exists():
        notes.append(f"missing {pairs_rel}")
    else:
        pairs_arg = pairs_file.read_text(encoding="utf-8").strip()
        if not pairs_arg:
            notes.append(f"empty {pairs_rel}")
        else:
            c_code, c_payload, c_err = run_cli_json(
                ["ui", "ds", "a11y", "--dir", ".", "--pairs", pairs_arg, "--json"]
            )
            c_data = c_payload.get("data", {}) if isinstance(c_payload, dict) else {}
            chrome_failures = c_data.get("failures", []) or []
            chrome_unresolved = c_data.get("unresolved", []) or []
            chrome_pairs = c_data.get("checkedPairs", 0)
            if isinstance(payload, dict):
                payload["chrome_pairs"] = c_data
            if c_err:
                notes.append(str(c_err))
            if c_code != 0:
                notes.append(f"chrome pairs exit {c_code}")

    summary = (
        f"supplementary: {pairs} foreground/surface pairs + {state_pairs} state pairs"
        f" + {chrome_pairs} explicit chrome pairs ({pairs_rel};"
        f" failures={len(failures) + len(chrome_failures)},"
        f" unresolved={len(unresolved) + len(chrome_unresolved)})"
    )
    if notes:
        summary = f"{summary}; " + "; ".join(notes)
    return True, summary, payload


def check_ui_ds_soul() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json(["ui", "ds", "soul", "check", "--dir", ".", "--json"])
    data = payload.get("data", {})
    findings = data.get("findings", [])
    err_count = data.get("errorCount", 0)
    soul_text = SOUL_MD.read_text(encoding="utf-8", errors="replace") if SOUL_MD.exists() else ""
    ratified = bool(re.search(r"(?m)^status:\s*ratified\b", soul_text))
    ok = (code == 0) and bool(payload.get("ok")) and len(findings) == 0 and err_count == 0 and ratified
    if ok:
        summary = "0 soul findings; design/soul.md status=ratified"
    else:
        summary = f"findings={len(findings)}, errorCount={err_count}, ratified={ratified} ({err})"
    return ok, summary, payload


def check_ui_agents() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json(["ui", "agents", "check", "--dir", ".", "--json"])
    data = payload.get("data", {})
    findings = data.get("findings", [])
    err_count = data.get("errorCount", 0)
    warn_count = data.get("warningCount", 0)

    claude_agents_dir = REPO_ROOT / ".claude" / "agents"
    claude_agent_files = sorted(p.name for p in claude_agents_dir.glob("*.md")) if claude_agents_dir.exists() else []

    cross_agents_dir = REPO_ROOT / ".agents" / "agents"
    missing_cross = [
        name for name in CROSS_RUNTIME_AGENTS if not (cross_agents_dir / name).exists()
    ]

    ok = (
        (code == 0)
        and bool(payload.get("ok"))
        and err_count == 0
        and warn_count == 0
        and len(claude_agent_files) >= 3
        and not missing_cross
    )
    if ok:
        summary = (
            f"{len(claude_agent_files)} stamped .claude/agents + "
            f"{len(CROSS_RUNTIME_AGENTS)} cross-runtime .agents/agents fresh (0 errors)"
        )
    else:
        summary = (
            f"errorCount={err_count}, warningCount={warn_count}, "
            f"claude_agents={len(claude_agent_files)}, missing_cross_runtime={missing_cross}"
        )
    return ok, summary, {
        "ui_agents_check": payload,
        "claude_agents": claude_agent_files,
        "cross_runtime_agents_missing": missing_cross,
    }


def run_self_evolution_pulse(check_only: bool = False) -> Tuple[bool, str, Dict[str, Any]]:
    pulse_steps: Dict[str, Any] = {}
    if not check_only:
        h_code, h_json, h_err = run_cli_json(["design-os", "harvest", "--dir", ".", "--emit-packet", "--json"])
        hb_code, hb_json, hb_err = run_cli_json(["design-os", "heartbeat", "--dir", ".", "--force", "--json"])
        mc_code, mc_out, mc_err = run_cli(["ui", "memory", "compile", "--dir", "."])
        pulse_steps = {
            "harvest": {"exit_code": h_code, "data": h_json, "stderr": h_err},
            "heartbeat": {"exit_code": hb_code, "data": hb_json, "stderr": hb_err},
            "memory_compile": {"exit_code": mc_code, "stdout": mc_out, "stderr": mc_err},
        }

    ev_code, ev_payload, ev_err = run_cli_json(["design-os", "evolution", "--dir", ".", "--json"])
    ev_data = ev_payload.get("data", {})
    verdict = ev_data.get("verdict")
    proof = ev_data.get("proof", {})
    proof_level = proof.get("level")
    proof_findings = proof.get("findings", [])
    ledger_total = ev_data.get("ledger", {}).get("total", 0)
    recurrent_insights = ev_data.get("graph", {}).get("insights_recurrent", 0)

    ok = (
        ev_code == 0
        and bool(ev_payload.get("ok"))
        and verdict == "ALIVE"
        and proof_level in ("APPLIED", "IMPROVING")
        and len(proof_findings) == 0
    )
    mode_label = "check-only" if check_only else "harvest+heartbeat+compile"
    if ok:
        summary = (
            f"verdict={verdict}, proof.level={proof_level}, findings=0, "
            f"ledger_events={ledger_total}, recurrent_insights={recurrent_insights} ({mode_label})"
        )
    else:
        summary = (
            f"verdict={verdict}, proof.level={proof_level}, findings={proof_findings} ({mode_label}; {ev_err})"
        )
    return ok, summary, {"pulse": pulse_steps, "evolution": ev_payload}


def check_ui_gate_specimen() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json(
        ["ui", "gate", "design/specimen.html", "--tokens", "design/design.tokens.json", "--json"]
    )
    data = payload.get("data", {})
    families = data.get("families", {})
    expected_families = ("layout", "a11y", "taste", "tell", "content", "autofix")
    family_errors: Dict[str, int] = {}
    family_warnings: Dict[str, int] = {}
    for fam in expected_families:
        fam_obj = families.get(fam, {})
        family_errors[fam] = int(fam_obj.get("errorCount", 0))
        family_warnings[fam] = int(fam_obj.get("warningCount", 0))

    total_errors = int(data.get("errorCount", sum(family_errors.values())))
    total_warnings = int(data.get("warningCount", sum(family_warnings.values())))
    gate_pass = bool(data.get("pass")) and (code == 0) and (total_errors == 0) and all(
        v == 0 for v in family_errors.values()
    )

    u_code, u_payload, u_err = run_cli_json(
        ["ui", "ds-usage-lint", "design/specimen.html", "--dir", ".", "--json"]
    )
    u_data = u_payload.get("data", {})
    u_errors = int(u_data.get("errorCount", 0))
    usage_ok = (u_code == 0) and bool(u_payload.get("ok")) and (u_errors == 0)

    ok = gate_pass and usage_ok
    if ok:
        summary = (
            f"6/6 families PASS (errors={total_errors}, warnings={total_warnings}); "
            f"ds-usage-lint errors={u_errors}"
        )
    else:
        summary = (
            f"gate_exit={code}, family_errors={family_errors}, warnings={total_warnings}, "
            f"ds_usage_errors={u_errors} ({err or u_err})"
        )
    return ok, summary, {"ui_gate": payload, "ds_usage_lint": u_payload}


def collect_audit_html_targets(root: Path = REPO_ROOT) -> Tuple[List[str], List[str]]:
    """HTML files `design-os audit .` would lint, split into (audited, excluded) repo-relative POSIX paths.

    Same walk as the CLI (`rglob("*.html")`, any path segment in AUDIT_WALK_EXCLUDED_SEGMENTS dropped,
    sorted); then only the two exclusions move to `excluded`: the pinned vendored subtree
    (AUDIT_EXCLUDED_PREFIX) and the files in AUDIT_EXCLUDED_FILES.
    """
    audited: List[str] = []
    excluded: List[str] = []
    for rel in sorted(path.relative_to(root) for path in root.rglob("*.html")):
        if not AUDIT_WALK_EXCLUDED_SEGMENTS.isdisjoint(rel.parts):
            continue
        is_excluded = (
            rel.parts[: len(AUDIT_EXCLUDED_PREFIX)] == AUDIT_EXCLUDED_PREFIX or rel.as_posix() in AUDIT_EXCLUDED_FILES
        )
        (excluded if is_excluded else audited).append(rel.as_posix())
    return audited, excluded


def audit_exclusion_reason(rel: str) -> str:
    """Why `rel` (an excluded path from `collect_audit_html_targets`) is not audited."""
    if tuple(Path(rel).parts[: len(AUDIT_EXCLUDED_PREFIX)]) == AUDIT_EXCLUDED_PREFIX:
        return "SHA-pinned upstream, byte-identical"
    return "tests capture of z-index 2147483647, its test asserts that value"


def run_design_os_audit(label: str, cmd: List[str]) -> Dict[str, Any]:
    """One `design-os audit` invocation, reduced to its summary counts and the sections that exited non-zero."""
    code, payload, err = run_cli_json(cmd)
    data = payload.get("data", {}) if isinstance(payload, dict) else {}
    summary = data.get("summary", {}) if isinstance(data, dict) else {}
    sections = data.get("sections", []) if isinstance(data, dict) else []
    return {
        "target": label,
        "exit_code": code,
        "ok": (code == 0) and isinstance(payload, dict) and bool(payload.get("ok")),
        "tools_run": int(summary.get("toolsRun", 0)),
        "tools_failed": int(summary.get("toolsFailed", 0)),
        "errors": int(summary.get("errors", 0)),
        "warnings": int(summary.get("warnings", 0)),
        "failed_sections": [
            {"tool": s.get("tool"), "target": s.get("target"), "exit_code": s.get("exitCode")}
            for s in sections
            if isinstance(s, dict) and s.get("exitCode") != 0
        ],
        "stderr": err,
    }


def check_design_os_project_audit() -> Tuple[bool, str, Dict[str, Any]]:
    audited, excluded = collect_audit_html_targets()
    # `design-os audit` adds its DS specimen / DS a11y / flow-lint sections for `--dir`. An empty scratch
    # target has no HTML, so this one run contributes exactly those sections and nothing else.
    with tempfile.TemporaryDirectory(prefix="g09-repo-level-") as empty_dir:
        runs = [
            run_design_os_audit("<repo-level DS/flow>", ["design-os", "audit", empty_dir, "--dir", ".", "--json"])
        ]
    runs.extend(run_design_os_audit(rel, ["design-os", "audit", rel, "--json"]) for rel in audited)

    tools_run = sum(r["tools_run"] for r in runs)
    tools_failed = sum(r["tools_failed"] for r in runs)
    audit_errors = sum(r["errors"] for r in runs)
    audit_warnings = sum(r["warnings"] for r in runs)
    runs_ok = all(r["ok"] for r in runs)
    # An empty walk must not pass on the strength of the repo-level run alone.
    audit_ok = (
        bool(audited)
        and runs_ok
        and (tools_run > 0)
        and (tools_failed == 0)
        and (audit_errors == 0)
    )
    a_err = "; ".join(f"{r['target']}: {r['stderr']}" for r in runs if r["stderr"] and not r["ok"])[:300]
    failing = [r for r in runs if (not r["ok"]) or r["errors"] or r["tools_failed"]]
    scope_lines = [
        f"audit scope: design-os audit . minus {AUDIT_EXCLUSION_SUMMARY} (owner rulings 2026-10-22)",
        f"audited html targets ({len(audited)}), plus 1 repo-level DS/flow run:",
        *(
            f"  {'FAIL' if (not r['ok'] or r['errors'] or r['tools_failed']) else 'ok  '} {r['target']} "
            f"(tools={r['tools_run']}, errors={r['errors']}, warnings={r['warnings']}, exit={r['exit_code']})"
            for r in runs[1:]
        ),
        f"repo-level DS/flow: tools={runs[0]['tools_run']}, errors={runs[0]['errors']}, exit={runs[0]['exit_code']}",
        f"excluded ({len(excluded)}):" + ("" if excluded else " none"),
        *(f"  {rel} ({audit_exclusion_reason(rel)})" for rel in excluded),
    ]
    a_payload: Dict[str, Any] = {
        "excluded_rule": AUDIT_EXCLUSION_SUMMARY,
        "audited_targets": audited,
        "excluded_targets": excluded,
        "runs": runs,
    }
    a_code = 0 if runs_ok else 1

    nodes_ok = True
    nodes_total = 0
    nodes_payload: Dict[str, Any] = {}
    if STUDIO_UI_NODES_JSON.exists():
        n_code, nodes_payload, _ = run_cli_json(
            [
                "ui",
                "audit",
                "design/studio-ui-nodes.json",
                "--tokens",
                "design/design.tokens.json",
                "--registry",
                "design/component-registry.json",
                "--grid",
                "4",
                "--json",
            ]
        )
        n_data = nodes_payload.get("data", {})
        nodes_total = int(n_data.get("total", len(n_data.get("violations", []))))
        nodes_ok = (n_code == 0) and bool(nodes_payload.get("ok")) and (nodes_total == 0)

    flow_ok = True
    flow_errors = 0
    flow_warnings = 0
    flow_payload: Dict[str, Any] = {}
    if FLOW_JSON.exists():
        f_code, flow_payload, _ = run_cli_json(["ui", "flow", "lint", "design/flow.json", "--json"])
        f_data = flow_payload.get("data", {})
        flow_errors = int(f_data.get("errorCount", 0))
        flow_warnings = int(f_data.get("warningCount", 0))
        flow_ok = (f_code == 0) and bool(flow_payload.get("ok")) and (flow_errors == 0)

    ok = audit_ok and nodes_ok and flow_ok
    if ok:
        summary = (
            f"design-os audit . minus {AUDIT_EXCLUSION_SUMMARY} ({len(audited)} html targets + repo-level, "
            f"{tools_run} tools, errors={audit_errors}, warnings={audit_warnings}, excluded={len(excluded)}); "
            f"nodes violations={nodes_total}; flow errors={flow_errors}"
        )
    else:
        shown = ", ".join(f"{r['target']}[errors={r['errors']}]" for r in failing[:3])
        more = f" +{len(failing) - 3} more" if len(failing) > 3 else ""
        summary = (
            f"audit_exit={a_code}, targets={len(audited)}, excluded={len(excluded)}, tools_run={tools_run}, "
            f"tools_failed={tools_failed}, audit_errors={audit_errors}, failing=[{shown}{more}], "
            f"nodes_violations={nodes_total}, flow_errors={flow_errors} ({a_err})"
        )
    return ok, summary, {
        "project_audit": a_payload,
        "studio_ui_nodes_audit": nodes_payload,
        "flow_lint": flow_payload,
        "report_lines": scope_lines,
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="design:os studio — Automated Self-Evolution Engine & Rust <-> design:os Token Bridge"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Read-only check mode (skips state-mutating harvest, heartbeat --force, and memory compile)",
    )
    parser.add_argument(
        "--evolve",
        action="store_true",
        help="Run full harvest + heartbeat --force + memory compile pulse (default unless --check is passed)",
    )
    parser.add_argument(
        "--record-gaps",
        action="store_true",
        help="Automatically record any detected Rust <-> design.tokens.json drift via `ui memory record gap`",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit machine-readable JSON summary report",
    )
    args = parser.parse_args()

    check_only = bool(args.check) and not bool(args.evolve)

    # Execute G08 and G09 before G07 so that in evolve mode G07 harvests the latest gate/audit events
    execution_order = [
        ("G01", "check_rust_token_parity", "Rust theme.rs <-> design.tokens.json Parity & Purple Ban", lambda: check_rust_token_parity(record_gaps=args.record_gaps)),
        ("G02", "zero_fake_ui_gate", "Zero Fake UI Deterministic Gate (all invariants)", check_zero_fake_ui_gate),
        ("G03", "ui_ds_status", "design:os Compiled Manifest & Component Registry", check_ui_ds_status),
        ("G04", "ui_ds_a11y", "WCAG Contrast Audit (ui ds a11y)", check_ui_ds_a11y),
        ("G05", "ui_ds_soul_check", "Ratified Soul Integrity (ui ds soul check)", check_ui_ds_soul),
        ("G06", "ui_agents_check", "Roster & Cross-Runtime Design Agents Parity", check_ui_agents),
        ("G08", "ui_gate_specimen", "6-Family Specimen Gate & DS Usage Lint (design/specimen.html)", check_ui_gate_specimen),
        ("G09", "design_os_project_audit", "Full Repository Audit, Node Tree & Flow Lint (design-os audit . minus skills/vendor/** and fixtures/stacking.html)", check_design_os_project_audit),
        ("G07", "design_os_evolution", "Self-Evolution Loop Proof (design-os evolution)", lambda: run_self_evolution_pulse(check_only=check_only)),
    ]

    results_by_id: Dict[str, Dict[str, Any]] = {}
    for gid, key, title, fn in execution_order:
        passed, summary, detail = fn()
        results_by_id[gid] = {
            "id": gid,
            "gate": key,
            "title": title,
            "passed": passed,
            "status": "PASS" if passed else "FAIL",
            "summary": summary,
            "detail": detail,
        }

    results: List[Dict[str, Any]] = [results_by_id[k] for k in sorted(results_by_id.keys())]

    passed_count = sum(1 for r in results if r["passed"])
    total_count = len(results)
    all_passed = passed_count == total_count

    if args.json:
        report = {
            "engine": "design_os_self_evolve",
            "mode": "check" if check_only else "evolve",
            "passed": passed_count,
            "total": total_count,
            "status": "PASS" if all_passed else "FAIL",
            "gates": results,
        }
        print(json.dumps(report, indent=2))
    else:
        print("=" * 116)
        print(
            f" design:os studio — Self-Evolution Engine & Live Token Parity Audit (mode={'CHECK' if check_only else 'EVOLVE'})"
        )
        print("=" * 116)
        print(f"{'ID':<4} | {'STATUS':<6} | {'GATE':<24} | {'EVIDENCE & VERDICT'}")
        print("-" * 116)
        for r in results:
            print(f"{r['id']:<4} | {r['status']:<6} | {r['gate']:<24} | {r['summary']}")
        print("-" * 116)
        for r in results:
            report_lines = r["detail"].get("report_lines") if isinstance(r["detail"], dict) else None
            if report_lines:
                print(f" {r['id']} detail:")
                for line in report_lines:
                    print(f"   {line}")
        print("-" * 116)
        verdict = "PASS" if all_passed else "FAIL"
        print(f" Overall Self-Evolution Verdict: {verdict} ({passed_count}/{total_count} gates passed)")
        print("=" * 116)

    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
