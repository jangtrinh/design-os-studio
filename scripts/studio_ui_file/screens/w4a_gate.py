"""Gate data of the W4a screens (Flow Home & editor shell), read by `studio_ui_gate_screen_data`: caps, causes, masks and pins per state. Plain data, no imports of the package.

A region's cap is the editor-default cap of the region of that name (_EDITOR below, the numbers of `screen_caps.DEFAULT_CAPS`), or a pinned measured value (rounded up to 0.01)
with its cause in CAUSES.
"""
import json
import pathlib

# the rects the wired data modules imply (gap_wired_texts.py writes the file): a state's kit strings the screen replaces with neutral ones, added to this module's own
_WIRED = {state: [(tuple(r[:4]), r[4], r[5]) for r in rects] for state, rects in json.loads(pathlib.Path(__file__).with_name("gap_wired_texts.json").read_text(encoding="utf-8")).items()}
_EDITOR = {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8}
_TOOLBAR, _CONTENT_RECT = (464, 840, 512, 48), (310, 52, 889, 848)
_EDITOR_REGIONS = {"tab bar": (0, 0, 1440, 32), "rail": (0, 32, 49, 868), "left panel": (49, 32, 241, 868), "right panel": (1199, 32, 241, 868), "ruler top strip": (310, 32, 889, 20),
                  "ruler left strip": (290, 52, 20, 848), "toolbar": _TOOLBAR}
_PAGES = ("Page 1", "Page 2", "——————", "SECTION", "↳ Page 3")
_SHELL_STATES = ("d-window-chrome", "e-no-selection", "d-menu-main", "d-menu-main-file", "d-menu-shape", "d-menu-zoom", "picker-page", "d-editor-selected", "e-layers-selected", "kit-editor",
                "micro-layer-picker", "micro-smart-selection", "micro-selection-corner", "micro-scale-tool", "micro-vector-edit", "micro-shape-controls", "micro-autolayout", "micro-css-grid")


def head_texts():
    """The kit strings every editor capture shows: the file name, the project tab title and the five page rows (Studio's own page labels such as `Pages` are never masked)."""
    # the project tab title stops where the dirty tab's dot starts: 185 = 301 - 116 (x of the layout's `tab-unsaved-dot-1`)
    return [((57, 40, 184, 24), "file name", "Design file"), ((116, 8, 185, 16), "project tab title", "Design file")] + [((61, 137 + 32 * i, 212, 32), "page name", n) for i, n in enumerate(_PAGES)]


def default_texts():
    names = ["Component 1", "Component 2"] + [f"Frame {i}" for i in range(1, 17)]
    return head_texts() + [((100, 345 + 32 * i, 82, 32), "layer name", n) for i, n in enumerate(names)] + [((1235, 281 + 160 * k, 110, 32), "text style folder", f"Heading {k + 1}") for k in range(4)]


def covered(texts, rect):
    """The texts the open overlay `rect` does not hide (a hidden text is not in the capture, so its rect stays in the comparison)."""
    x, y, w, h = rect
    return [t for t in texts if not (t[0][0] < x + w and x < t[0][0] + t[0][2] and t[0][1] < y + h and y < t[0][1] + t[0][3])]


_MENUS = {"d-menu-main": [(12, 44, 210, 581)], "d-menu-main-file": [(12, 44, 210, 581), (222, 142, 265, 434)], "d-menu-shape": [(619, 636, 246, 208)], "d-menu-zoom": [(1216, 44, 212, 453)]}
_OVERLAYS = {"picker-page": ("color picker", (960, 123, 240, 498)), "micro-layer-picker": ("layer picker", (680, 410, 220, 294)),
             "micro-vector-edit": ("vector tools", (618, 800, 252, 28))}  # the vector tools bar is the capture's `vtool:*` widgets (layout named rects), not an `overlays` entry


def texts(state):
    """The kit strings of the state: the same 29 rects for every editor capture (a selection capture's layer rows are other rows, but their name column is the kit's all the same),
    less the ones a menu hides."""
    base = default_texts()
    for rect in _MENUS.get(state, []):
        base = covered(base, rect)
    return base


_SWATCHES = ("the six swatch centres read (38, 38, 38), (250, 250, 250), (23, 23, 23), (240, 240, 240), (216, 0, 39) and (0, 82, 180) in the capture and (38, 38, 38), (10, 10, 10), "
             "(227, 99, 35), (239, 68, 68), (13, 153, 255) and (34, 197, 94) in the render (the part draws its own list under the `On this page` source)")
_RULERS = ("Navigation/Rulers draws the 2 % view with no selection band (W3a pinned the strips on that view at 6.4 and 6.6); this capture's rulers follow its own viewport and selection: other tick "
           "spacing and labels, the selection's blue band. The registry has one Rulers record with no variant for them")
_SCREEN = "the whole-screen mean carries the pinned regions of this state (the ruler strips and the regions pinned beside them)"
_TILE = "the worst 40 px tile sits on the pinned region of this state with the highest mean"
_EDGES = "the rows' text and glyph edges, the submenu chevrons and the panel's corner arcs (egui against the renderer), the only marks the heat maps of the main, File and shape menus show"
_MENU = "no editor-default region: pinned at the measured mean; "
_WHY = {"ruler top strip": _RULERS, "ruler left strip": _RULERS, "screen": _SCREEN, "menu 1": _MENU + _EDGES, "menu 2": _MENU + "the File submenu's " + _EDGES,
        "color picker": ("no editor-default region: Popover/ColorPicker `Kind=Page` (the page picker's Custom tab, 240 x 498): its crop rows pass; the heat map shows the swatch row as "
                         "filled squares: " + _SWATCHES + "; besides these it shows the text and glyph edges, the dotted contrast curve over the square (a recorded gap of the picker) "
                         "and the rings of the handles"),
        "vector tools": ("no editor-default region: the bar is Tabs/Segmented with its third cell shown; production draws a BG_TOOLBAR band 36 px high with 84 x 28 transparent cells "
                         "(BG_TOOLBAR_SELECTED for the active one) where Tabs/Segmented draws a BG_SECONDARY shell with its own label tones: the same bar, not the same painter"),
        "layer picker": ("no editor-default region: the layer picker is a second painter drawn as a Menu/Panel: its caption is Menu/Item Disabled (Body Medium) where production draws "
                         "Caption, every row sits 8 px low, the icons 4 px and the labels 6 px right of production's, and the 1 px separator under the caption is left out"),
        "home sidebar": "no editor-default region: Panel/HomeSidebar, pinned at the measured mean", "home main": "no editor-default region: Panel/HomeMain, pinned at the measured mean"}
_SCROLLED = ("the capture's Layers list is scrolled: its first row's glyph centre sits at y 335.5 of the panel where a row at rest is centred at 328.5 (measured on this capture, "
             "f-picker, j-comments and j-cmdk); a slot cannot express a scroll offset, so the screen keeps the 18 default rows (the name column is masked as the open file's own text)")
_SPECIFIC = {
    ("e-layers-selected", "left panel"): _SCROLLED,
    ("micro-css-grid", "left panel"): ("the tree of this capture has four rows under the selected grid row, and Tree/Row has no look for a child of a selected row (drawn from the first "
                                       "row at rest; the name column is masked as the open file's own text, the kind icons, chevrons and type labels count)"),
    ("micro-scale-tool", "right panel"): ("the capture's frame has no auto layout, so its inspector draws a Copy to Slide Deck row, a collapsed Auto layout, the Clip content row and the "
                                          "variable-mode rows that Panel/RightInspector `State=Layers` does not hold (as hidden alternates they would add 76,524 B to each of 8 "
                                          "instances): everything below the name row sits 32 px high, and 73 px high from the mode rows down"),
    ("micro-vector-edit", "right panel"): ("the capture's inspector opens with the Vertex section (3 vertices, corner radius, stroke cap, handles, Join / Split / Simplify), "
                                           "which no state of Panel/RightInspector holds: the screen keeps the Page state"),
    ("d-menu-main-file", "ruler left strip"): ("the File submenu (x 222 to 487, y 142 to 576) covers the left ruler strip (x 290 to 310): its text edges count in this region; the rulers are "
                                               "the 2 % view as drawn"),
    ("d-menu-shape", "menu 1"): _MENU + _EDGES + ", and the selected row's fill tone",
    ("d-menu-zoom", "right panel"): ("the zoom menu (x 1216 to 1428, y 44 to 497) covers the top of the right panel: the menu's own mismatch lies in this region; the panel under it is the "
                                     "Page state as drawn"),
    ("d-menu-zoom", "menu 1"): (_MENU + "the zoom menu is a second painter whose input row is 26 px high where Input/Field is 32, so its rows sit 2 px below production's"),
}
CAPS = {s: dict(_EDITOR) for s in _SHELL_STATES}
CAUSES = {s: {} for s in _SHELL_STATES}
REGIONS = {s: dict(_EDITOR_REGIONS) for s in _SHELL_STATES}
TEXTS = {s: texts(s) + _WIRED.get(s, []) for s in _SHELL_STATES}
CONTENT = {s: (_CONTENT_RECT, _TOOLBAR) for s in _SHELL_STATES}
KEEP = {s: list(_MENUS.get(s, [])) + ([_OVERLAYS[s][1]] if s in _OVERLAYS else []) for s in _SHELL_STATES}
for _s, _rects in _MENUS.items():
    for _i, _r in enumerate(_rects):
        REGIONS[_s][f"menu {_i + 1}"] = _r
for _s, (_n, _r) in _OVERLAYS.items():
    REGIONS[_s][_n] = _r
CAPS["h-home"] = {"screen": 4.6, "tab bar": 0.4, "home sidebar": 0.0, "home main": 0.0}
CAUSES["h-home"] = {}
REGIONS["h-home"] = {"tab bar": (0, 0, 1440, 32), "home sidebar": (0, 32, 264, 868), "home main": (264, 32, 1176, 868)}
TEXTS["h-home"] = [((296, 176, 224, 140), "file thumbnail", "the file preview image: the document's own pixels")]
CONTENT["h-home"] = None
WORST_TILE, GEOMETRY = {}, {}
# PINS (measured by `evidence screens`, rounded up to 0.01): {state: {region: mean}}, and the worst tile per state
_PINS = {
    "h-home": {"home sidebar": 2.07, "home main": 0.4},
    "d-editor-selected": {"ruler top strip": 15.07, "ruler left strip": 13.25},
    "e-layers-selected": {"screen": 5.08, "left panel": 8.93, "ruler top strip": 10.57, "ruler left strip": 13.33},
    "kit-editor": {"screen": 5.55, "ruler top strip": 46.53, "ruler left strip": 47.94},
    "d-menu-main": {"menu 1": 2.09},
    "d-menu-main-file": {"ruler left strip": 7.3, "menu 1": 2.09, "menu 2": 2.91},
    "d-menu-shape": {"menu 1": 4.31},
    "d-menu-zoom": {"right panel": 7.05, "menu 1": 8.22},
    "picker-page": {"color picker": 3.67},
    "micro-layer-picker": {"ruler top strip": 26.36, "ruler left strip": 18.61, "layer picker": 8.35},
    "micro-smart-selection": {"ruler top strip": 39.12, "ruler left strip": 24.35},
    "micro-selection-corner": {"ruler top strip": 37.7, "ruler left strip": 23.99},
    "micro-scale-tool": {"screen": 7.44, "right panel": 10.34, "ruler top strip": 29.51, "ruler left strip": 24.08},
    "micro-vector-edit": {"screen": 8.48, "right panel": 11.68, "ruler top strip": 32.81, "ruler left strip": 27.05, "vector tools": 23.96},
    "micro-shape-controls": {"ruler top strip": 27.05, "ruler left strip": 27.88},
    "micro-autolayout": {"ruler top strip": 35.79, "ruler left strip": 23.0},
    "micro-css-grid": {"screen": 6.03, "left panel": 3.61, "ruler top strip": 39.57, "ruler left strip": 30.24},
}
_TILES = {
    "e-layers-selected": 34.61,
    "kit-editor": 50.37,
    "picker-page": 46.17,
    "micro-layer-picker": 41.2,
    "micro-smart-selection": 36.28,
    "micro-selection-corner": 34.5,
    "micro-scale-tool": 40.06,
    "micro-vector-edit": 40.77,
}
for _s, _pin in _PINS.items():
    for _r, _cap in _pin.items():
        CAPS[_s][_r] = _cap
        CAUSES[_s][_r] = _SPECIFIC[(_s, _r)] if (_s, _r) in _SPECIFIC else _WHY[_r]
for _s, _t in _TILES.items():
    WORST_TILE[_s] = _t
    CAUSES[_s]["worst tile"] = _TILE
