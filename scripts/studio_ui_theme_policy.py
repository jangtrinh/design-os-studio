"""Policy for reading theme.rs as design variables: which constants are not drawable, and which token key
(`design/design.tokens.json`) each mapped constant names. Shared by the generator and by the gate.
"""
from __future__ import annotations

import importlib.util
from pathlib import Path
from typing import Dict

# The structure the file generator builds and the gate checks (S1/S2). One copy: both import these.
PRIMITIVES, SEMANTIC, MODE_NAME = "Primitives", "Semantic", "Dark"
SEMANTIC_NAME = r"[a-z][a-z0-9]*/[a-z0-9]+(?:-[a-z0-9]+)*"  # group/kebab
FILE_REL, ALLOWLIST_REL = "design/studio-ui.designos", "design/studio-ui.gate-allowlist.json"

_HANDLE = "egui font-family handle name, not a design value"
_COUNT = "count (usize), not a drawable value"
_DEFAULT = "default of a new document layer, not chrome"
_BEHAVIOUR = "interaction threshold or unitless factor, nothing is painted with it"

# Constants with no drawable value, and why. Everything else the reader resolves is drawable and must have exactly
# one Semantic variable. Keys are whole constants (an array skips every element).
SKIPPED: Dict[str, str] = {
    "FAMILY_STRONG": _HANDLE, "FAMILY_MEDIUM": _HANDLE, "FAMILY_CANVAS_LABEL": _HANDLE,
    "APP_TITLE": "window title string, not a drawable value",
    "PAGES_MAX_VISIBLE_ROWS": _COUNT, "FIND_EXCERPT_LEAD": _COUNT, "MODE_SEGMENTS": _COUNT, "FIND_CANVAS_MATCH_MAX": _COUNT,
    "TOAST_SECONDS": "duration in seconds, no node property can bind it",
    "TOAST_REVEAL_SECONDS": "duration in seconds, no node property can bind it",
    "DEFAULT_WINDOW": "OS window size tuple, not chrome", "MIN_WINDOW": "OS window size tuple, not chrome",
    "AGENT_COLORS": "array of AGENT_1..AGENT_4, which each have a variable",
    "ZOOM_MIN": _BEHAVIOUR, "ZOOM_MAX": _BEHAVIOUR, "PIXEL_GRID_MIN_ZOOM": _BEHAVIOUR, "CANVAS_LABEL_MAX_ZOOM_HIDE": _BEHAVIOUR,
    "PRESENT_MAX_SCALE": _BEHAVIOUR, "SNAP_THRESHOLD_PX": _BEHAVIOUR, "DRAG_THRESHOLD_PX": _BEHAVIOUR, "NUDGE": _BEHAVIOUR,
    "NUDGE_BIG": _BEHAVIOUR, "PASTE_OFFSET": _BEHAVIOUR, "BOARD_DOT_MIN_PX": _BEHAVIOUR, "FIT_PADDING": _BEHAVIOUR,
    "LIGHT_BG_THRESHOLD": "luminance threshold, unitless",
    "UI_FONT_OPSZ": "font variation axis value, no node property binds it",
    "DEFAULT_PAGE_BG": _DEFAULT, "DEFAULT_STROKE": _DEFAULT, "DEFAULT_FRAME_FILL": _DEFAULT, "DEFAULT_SHAPE_FILL": _DEFAULT,
    "DEFAULT_SHAPE_SIZE": _DEFAULT, "DEFAULT_TEXT_SIZE": _DEFAULT, "AUTO_LAYOUT_SPACING": _DEFAULT, "AUTO_LAYOUT_PADDING": _DEFAULT,
}


def token_keys(scripts_dir: Path) -> Dict[str, str]:
    """const name -> token key, from G01 (`design_os_self_evolve`) and `non_colour_token_parity`."""
    import sys
    sys.path.insert(0, str(scripts_dir))
    spec = importlib.util.spec_from_file_location("studio_ui_g01", scripts_dir / "design_os_self_evolve.py")
    g01 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(g01)
    import non_colour_token_parity as nc
    keys = {k: f"colors.{v}" for k, v in {**g01.RUST_COLOUR_SHARED_KEY, **g01.RUST_TO_DESIGN_TOKEN_MAP}.items()}
    for path, names in nc.TOKEN_TO_RUST.items():
        for name in names:
            keys.setdefault(name, path)
    return keys
