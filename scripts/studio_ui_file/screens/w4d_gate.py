"""Gate data of batch W4D (studio_ui_gate_screen_data): what each capture of the batch leaves out of the comparison and the cap of each region it reports.

Plain data, no import. A cap is the editor-default cap of its region where the render meets it (tab bar, toolbar, and on `ai-image-*` the rail), else the number the render measured, pinned with its
cause in CAUSES (no margin added, and no state calibrates itself). `_MEASURED` holds those numbers (`evidence screens`, scale 1); CAPS is derived from it, so a pin cannot drift from its
measurement without the gate saying so. Nothing here is a cap loosened to pass: each pin names what the region cannot draw yet. The tab bar of `ai-image-*`, which draws the dirty tab's dot,
is pinned the same way, under the editor default: its cap is its `_MEASURED` mean, so a missing dot goes over it.

The texts replaced with neutral ones are a capture's kit strings (the project tab title, the selection chip, the selected layer's name, on `ai-image-*` the file name, the pages and the layer tree);
the region draws its own neutral text, or none where the region has no such text (the right inspector draws the page, not a selected layer).
"""
import json
import pathlib

# the rects the wired data modules imply (gap_wired_texts.py writes the file): a state's kit strings the screen replaces with neutral ones, added to this module's own
_WIRED = {state: [(tuple(r[:4]), r[4], r[5]) for r in rects] for state, rects in json.loads(pathlib.Path(__file__).with_name("gap_wired_texts.json").read_text(encoding="utf-8")).items()}
_DEFAULT = {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8}  # screen_caps.DEFAULT_CAPS
_AGENTS = ["i-agents", "i-agents-chat", "i-agents-picker", "i-agents-start-failure", "i-agent-permissions", "i-agents-approval-command", "i-terminal", "agents-approval-open-tab",
           "agents-approval-queued", "agents-skills", "kit-agents"]
_AI_IMAGE = ["ai-image-menu", "ai-image-erase", "ai-image-expand"]
_CANVAS = {"agents": (450, 32, 749, 868), "layers": (290, 32, 909, 868)}
_TOOLBAR = (464, 840, 512, 48)
_PAGES = ("Page 1", "Page 2", "——————", "SECTION", "↳ Page 3")
_NONE = "none: the right inspector draws the page, not a selected layer"
_OVERLAY = {"i-agent-permissions": (540, 326, 360, 248), "agents-skills": (57, 608, 320, 192), "ai-image-menu": (1216, 668, 180, 161), "ai-image-erase": (944, 640, 248, 144),
            "ai-image-expand": (956, 640, 236, 156)}

# region means and the worst 40 px tile, measured by `evidence screens` (the whole screen, then each region; `overlay` where the state has one)
_MEASURED = {
    "i-agents": {"screen": 4.21, "tab bar": 0.293, "rail": 2.37, "left panel": 4.909, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493},
    "i-agents-chat": {"screen": 3.82, "tab bar": 0.293, "rail": 2.37, "left panel": 4.13, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493},
    "i-agents-picker": {"screen": 4.32, "tab bar": 0.293, "rail": 2.37, "left panel": 5.13, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493},
    "i-agents-start-failure": {"screen": 2.89, "tab bar": 0.293, "rail": 2.37, "left panel": 2.26, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493},
    "i-agent-permissions": {"screen": 5.51, "tab bar": 0.293, "rail": 2.37, "left panel": 6.838, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493, "overlay": 8.14},
    "i-agents-approval-command": {"screen": 3.96, "tab bar": 0.293, "rail": 2.37, "left panel": 4.41, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493},
    "i-terminal": {"screen": 4.32, "tab bar": 0.293, "rail": 2.37, "left panel": 5.13, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493},
    "agents-approval-open-tab": {"screen": 3.95, "tab bar": 0.293, "rail": 2.37, "left panel": 4.4, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493},
    "agents-approval-queued": {"screen": 3.93, "tab bar": 0.293, "rail": 2.37, "left panel": 4.34, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493},
    "agents-skills": {"screen": 4.87, "tab bar": 0.293, "rail": 2.37, "left panel": 6.35, "right panel": 2.96, "ruler top strip": 12.52, "ruler left strip": 13.576,
        "toolbar": 3.493, "overlay": 17.1},
    "kit-agents": {"screen": 6.08, "tab bar": 0.293, "rail": 2.37, "left panel": 4.85, "right panel": 3.98, "ruler top strip": 48.562, "ruler left strip": 47.645,
        "toolbar": 3.686},
    "ai-image-menu": {"screen": 3.13, "tab bar": 0.3, "rail": 2.385, "left panel": 1.96, "right panel": 2.59, "ruler top strip": 15.069, "ruler left strip": 13.242,
        "toolbar": 3.499, "overlay": 3.58},
    "ai-image-erase": {"screen": 3.41, "tab bar": 0.3, "rail": 2.385, "left panel": 1.96, "right panel": 2.92, "ruler top strip": 15.069, "ruler left strip": 13.242,
        "toolbar": 3.499, "overlay": 5.12},
    "ai-image-expand": {"screen": 3.25, "tab bar": 0.3, "rail": 2.385, "left panel": 1.96, "right panel": 2.85, "ruler top strip": 15.069, "ruler left strip": 13.242,
        "toolbar": 3.499, "overlay": 3.42},
}
_TILES = {
    "i-agents": 21.31,
    "i-agents-chat": 34.3,
    "i-agents-picker": 21.31,
    "i-agents-start-failure": 34.18,
    "i-agent-permissions": 43.05,
    "i-agents-approval-command": 34.3,
    "i-terminal": 21.31,
    "agents-approval-open-tab": 33.23,
    "agents-approval-queued": 34.3,
    "agents-skills": 41.42,
    "kit-agents": 36.78,
    "ai-image-menu": 18.96,
    "ai-image-erase": 18.96,
    "ai-image-expand": 18.96,
}

_SCREEN = "the pixel-weighted mean over the regions of this state and the unmasked rest of the window; it follows the pinned regions, each at the default cap or pinned with its cause"
_RULERS = "the capture's rulers sit at the document's own scroll position and zoom; Navigation/Rulers draws the d-editor-default view, so labels and ticks differ"
_RULERS_KIT = "the capture is at 100 % zoom (a 50 unit scale with dense labels); Navigation/Rulers draws the 2 % view of d-editor-default"
_LEFT_AGENTS = ("Panel/AgentTerminal is the 0.7.5 painter of the state i-agents: text anti-aliasing and the panel fill BG (10, 10, 10) against the capture's (14, 14, 18) (the terminal "
                "colours have no variable)")
_CARD = ("Panel/AgentTerminal `State=Approval` draws the card, its `Needs approval` pill and the 184 px chip: the pill and the status text use the nearest theme tone of STATUS_WARN_TEXT "
         "(no variable holds it), and the terminal surface is the nearest tone of the capture's")
_LEFT = {
    "i-agents": _LEFT_AGENTS,
    "i-agents-picker": _LEFT_AGENTS,
    "i-terminal": _LEFT_AGENTS,
    "kit-agents": _LEFT_AGENTS,
    "i-agents-chat": ("Panel/AgentTerminal `State=Chat` draws the PTY view with its cursor, the session tab and the notice: the composer sits at y 786 in the capture and in the render "
                      "(layout `agents-composer-box`, node `Composer`), and the terminal surface is the nearest tone of the capture's: (17, 18, 22) against (14, 14, 18) at (200, 600)"),
    "i-agents-start-failure": "Panel/AgentTerminal `State=Failure` draws the start-failure card, its message, last output and Open a shell button",
    "i-agent-permissions": ("the capture's shell PTY view and its session tab are not drawn (no component), and its composer sits at y 786 where the render's `Composer` node is at 808; "
                            "the picker body is hidden"),
    "i-agents-approval-command": _CARD,
    "agents-approval-open-tab": _CARD,
    "agents-approval-queued": _CARD,
    "agents-skills": _LEFT_AGENTS + "; the skills popover sits over this panel (its own region below)",
    "ai-image-menu": ("the capture's layer tree is the document's: the screen draws the neutral tree of `gap_left_dev_data` and masks the name column (the kind icons, the selection row and the "
                      "type labels count)"),
}
_OVERLAY_CAUSE = {
    "i-agent-permissions": "Popover/Panel `Kind=Permissions` draws the modal (360 x 248, its two Radio/Card rows and the footer note): what is left is text and glyph edges",
    "agents-skills": ("Popover/Panel `Kind=Skills` draws the picker (320 x 192): the skill rows' selected fills and the count pills use the nearest theme tones (the pills' fill equals the "
                      "surface and vanishes), and the names and descriptions are masked as the machine's own skills"),
    "ai-image-menu": ("Menu/Panel with four Menu/Item rows (`Size=Xs`, 32 px as `popup_menu` draws) and one Menu/Separator: the heat map shows the rows' text and glyph edges"),
    "ai-image-erase": "Popover/Panel `Kind=Erase` draws the popover (248 x 144, instruction field, caption, Cancel, Erase): what is left is text and glyph edges and the Erase button's tone",
    "ai-image-expand": ("Popover/Panel `Kind=Expand` draws the popover (236 x 156, the four insets, the preset pill, Cancel, Expand): the heat map shows text and glyph edges"),
}
_HEADER_TILE = "(80, 40) is the panel's header row: the session tab and the top of the selection chip"
_TILE_CAUSE = {
    "i-agents-chat": _HEADER_TILE,
    "i-agents-start-failure": _HEADER_TILE,
    "i-agent-permissions": "(120, 840) is the composer's chip row: the capture's composer sits 22 px higher (y 786, the notice below it)",
    "i-agents-approval-command": _HEADER_TILE,
    "agents-approval-queued": _HEADER_TILE,
    "agents-skills": "(200, 40) is the panel's header row: the session tab, the Skills control and the chip",
    "kit-agents": "(440, 360) is the left ruler strip of the agents canvas, a pinned region of this state",
}


def _regions(state):
    cx, cy, cw, ch = _CANVAS["layers" if state in _AI_IMAGE else "agents"]
    out = {"tab bar": (0, 0, 1440, 32), "rail": (0, 32, 49, 868), "left panel": (49, 32, cx - 49, 868), "right panel": (1199, 32, 241, 868), "ruler top strip": (cx + 20, 32, cw - 20, 20),
           "ruler left strip": (cx, 52, 20, 848), "toolbar": _TOOLBAR}
    if state in _OVERLAY:
        out["overlay"] = _OVERLAY[state]
    return out


def _texts(state):
    # the project tab title stops where the dirty tab's dot starts: 185 = 301 - 116 (x of the layout's `tab-unsaved-dot-1`)
    out = [((116, 8, 185, 16), "project tab title", "Design file"), ((1215, 130, 60 if state in _AI_IMAGE else 130, 22), "selected layer name", _NONE)]
    if state in _AGENTS:
        w = 208 if state == "kit-agents" else 170 if "approval" in state else 226
        return out + [((65, 70, w, 20), "selection chip text", "Page 1 · 1 selected · Frame 1")]
    out += [((57, 40, 184, 24), "file name", "Design file")] + [((61, 137 + 32 * i, 212, 32), "page name", n) for i, n in enumerate(_PAGES)]
    rows = [(100, 345 + 32 * i, 82, min(32, 900 - 345 - 32 * i)) for i in range(18)]  # the name column of the capture's own tree, as d-editor-default masks it (the last row is cut by the window)
    return out + [(r, "layer name", f"Frame {i + 1}") for i, r in enumerate(rows)]


def _content(kind):
    """The content rect (the canvas below and right of the ruler strips) and the toolbar clipped to it: the framework builds the pieces above and below the toolbar from the
      toolbar's own x, so a toolbar that starts left of the content (x 464 against x 470 on the agents canvas) would leave out 6 columns of the left ruler strip."""
    cx, cy, cw, ch = _CANVAS[kind]
    x, tx = cx + 20, max(_TOOLBAR[0], cx + 20)
    return (x, cy + 20, cw - 20, ch - 20), (tx, _TOOLBAR[1], _TOOLBAR[0] + _TOOLBAR[2] - tx, _TOOLBAR[3])


def _cap(region, value):
    d = _DEFAULT.get(region)
    return d if d is not None and value <= d else value


def _causes(state, measured):
    out = {}
    for region, value in measured.items():
        if _cap(region, value) == _DEFAULT.get(region):
            continue
        out[region] = {"screen": _SCREEN, "ruler top strip": _RULERS_KIT if state == "kit-agents" else _RULERS, "ruler left strip": _RULERS_KIT if state == "kit-agents" else _RULERS,
                       "left panel": _LEFT.get(state), "overlay": _OVERLAY_CAUSE.get(state)}[region]
    if state in _TILES and _TILES[state] > 34.0:
        out["worst tile"] = _TILE_CAUSE[state]
    return out


CAPS = {s: {r: _cap(r, v) for r, v in m.items()} for s, m in _MEASURED.items()}
CAUSES = {s: _causes(s, m) for s, m in _MEASURED.items()}
WORST_TILE = {s: v for s, v in _TILES.items() if v > 34.0}
# `ai-image-*` draw the dirty tab's dot: the tab bar measures 0.3 (`_MEASURED`) and its cap is that mean, not the editor default (`_cap` keeps the default under it), so a missing dot goes over it
for _s in _AI_IMAGE:
    CAPS[_s]["tab bar"] = _MEASURED[_s]["tab bar"]
    CAUSES[_s]["tab bar"] = ("the dirty tab's dot is drawn (`Show unsaved dot`), so the tab bar mean is 0.3: the cap is that mean rounded up to 0.01, with no margin, as a measured pin is, and a "
                             "missing dot goes over it")
TEXTS = {s: _texts(s) + _WIRED.get(s, []) for s in _AGENTS + _AI_IMAGE}
CONTENT = {s: _content(k) for k, ss in (("agents", _AGENTS), ("layers", _AI_IMAGE)) for s in ss}
REGIONS = {s: _regions(s) for s in _AGENTS + _AI_IMAGE}
KEEP = {"i-agent-permissions": [(540, 326, 360, 248)], "ai-image-erase": [(944, 640, 248, 144)], "ai-image-expand": [(956, 640, 236, 156)]}
