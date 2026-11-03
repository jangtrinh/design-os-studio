"""Gate data of the W4B screens, the caps side: a cap is the editor-default cap of its region where the capture matches the painter, else the measured mean of the state pinned with its
cause; no state calibrates itself. Numbers are `evidence screens` against the 0.7.5 captures with designos-mcp 0.7.5 (renders are byte-identical across runs).

    _DEFAULT  the editor-default caps (screen_caps.DEFAULT_CAPS), copied because a gate module imports nothing of the package
    _PINS     {state: {region: measured mean}}: the regions over their editor-default cap, and each overlay region (it has no default)
    CAUSES    one cause per pinned region, by rule (the region, then the state when a state differs), and one for each worst-tile override
    GEOMETRY  the named widget rects that are off by more than 1 px, with the measured numbers and why
"""
_DEFAULT = {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8}
_DEV = ("g-dev-react", "g-dev-annotate", "g-dev-compare", "micro-dev-boxmodel")
_PINS = {
    "f-edit-style-color": {"tab bar": 0.3, "ruler top strip": 15.069, "ruler left strip": 13.242, "popover": 2.07},
    "f-effect-styles": {"screen": 4.99, "tab bar": 0.7, "left panel": 4.469, "right panel": 9.638, "ruler top strip": 14.614, "ruler left strip": 13.242, "menu": 6.04},
    "f-effects-menu": {"screen": 7.14, "left panel": 4.469, "right panel": 10.38, "ruler top strip": 15.069, "ruler left strip": 13.242, "menu": 3.24},
    "f-picker": {"screen": 4.72, "left panel": 9.008, "ruler top strip": 13.198, "ruler left strip": 13.721, "picker": 3.431},
    "f-picker-gradient": {"tab bar": 0.3, "ruler top strip": 15.069, "ruler left strip": 13.242, "picker": 3.01},
    "f-prototype-tab": {"left panel": 4.54, "ruler top strip": 13.582, "ruler left strip": 12.754, "popover": 3.05},
    "f-stroke-settings": {"ruler top strip": 15.069, "ruler left strip": 13.242, "popover": 2.12},
    "f-type-details": {"left panel": 8.92, "ruler top strip": 15.432, "ruler left strip": 13.169, "toolbar": 3.99, "popover": 3.18},
    "fill-d9": {"tab bar": 0.3, "ruler top strip": 15.069, "ruler left strip": 13.242},
    "edit-style-effect": {"tab bar": 0.3, "ruler top strip": 15.069, "ruler left strip": 13.242, "popover": 1.81},
    "edit-style-grid": {"tab bar": 0.3, "ruler top strip": 15.069, "ruler left strip": 13.242, "popover": 2.38},
    "edit-style-text": {"tab bar": 0.3, "screen": 4.86, "left panel": 8.92, "ruler top strip": 15.432, "ruler left strip": 13.233, "popover": 3.06},
    "effect-glass": {"tab bar": 0.3, "ruler top strip": 15.069, "ruler left strip": 13.242, "popover": 2.21},
    "effect-noise": {"tab bar": 0.3, "ruler top strip": 15.069, "ruler left strip": 13.242, "popover": 2.29},
    "effect-texture": {"tab bar": 0.3, "ruler top strip": 15.069, "ruler left strip": 13.242, "popover": 1.58},
    "grid-styles": {"tab bar": 0.3, "screen": 5.78, "left panel": 4.469, "right panel": 7.34, "ruler top strip": 15.069, "ruler left strip": 13.242, "menu": 3.28},
    "micro-inspector-math": {"screen": 7.81, "right panel": 10.57, "ruler top strip": 33.984, "ruler left strip": 24.78},
    "micro-typography-multiedit": {"ruler top strip": 38.05, "ruler left strip": 18.116},
    "micro-components-variables": {"screen": 7.25, "tab bar": 2.2, "rail": 7.35, "left panel": 10.11, "ruler top strip": 29.27, "ruler left strip": 32.83},
    "micro-dev-boxmodel": {"tab bar": 0.26, "screen": 11.78, "left panel": 5.55, "right panel": 15.0, "ruler top strip": 47.029, "ruler left strip": 37.913, "toolbar": 4.28},
    "g-dev-annotate": {"tab bar": 0.26, "ruler top strip": 13.198, "ruler left strip": 13.721, "toolbar": 3.95},
    "g-dev-compare": {"tab bar": 0.21, "screen": 44.22, "right panel": 8.22, "dialog": 84.11},
    "g-dev-react": {"tab bar": 0.26, "ruler top strip": 13.198, "ruler left strip": 13.721, "toolbar": 4.28},
}
_FLOAT = ("the floating Variables window (Window/Variables `Layout=Floating`, 642 x 380 at (0, 8)) covers part of this region, and its own mismatch counts here (its ink box is 613 x 352 "
          "against the capture's 622 x 364)")
_SCRIM = ("the capture's scrim (black, alpha 140) dims this region; the screen draws a Scrim of the same colour, the differences are the content under it")
_NO_DATA = "no data covers this screen (`gap_right_panel_data`, `gap_left_dev_data`)"
_RULERS = "the capture's ruler is at another zoom and scroll offset (ticks, labels, selection band) than the 2 % view of Navigation/Rulers"
_SCROLLED = ("the capture's Layers list is scrolled: its first row's glyph centre sits at y 335.5 of the panel where a row at rest is centred at 328.5 (measured on e-layers-selected, f-picker, "
             "j-comments and j-cmdk); a slot cannot express a scroll offset, so the screen keeps the 18 default rows")
_NO_FRAMEWORK = ("the capture's frame has no auto layout, so its inspector draws a Copy to Slide Deck row, a collapsed Auto layout, the Clip content row and the variable-mode rows that "
                 "Panel/RightInspector `State=Layers` does not hold (as hidden alternates they would add 76,524 B to each of 8 instances): everything below the name row sits 32 px high")
_SWATCHES = ("the six swatch centres read (38, 38, 38), (250, 250, 250), (23, 23, 23), (240, 240, 240), (216, 0, 39) and (0, 82, 180) in the capture and (38, 38, 38), (10, 10, 10), "
             "(227, 99, 35), (239, 68, 68), (13, 153, 255) and (34, 197, 94) in the render (the part draws its own list under the `On this page` source)")
_EDGES = "the rows' text and glyph edges (egui against the renderer)"
_LIST_SHIFT = ("the tree is drawn (`gap_left_dev_data`) and scores 8.92, the mean of the 18 default rows (8.945): the capture's Layers list is scrolled about 7 px as the scrolled states' "
               "are (best vertical shift of the capture against the render, +7 px: mean difference 14.956 -> 5.966 over the list), and a slot cannot express a scroll offset")
_REGION = {
    "screen": "the mean over every counted pixel: it carries the region causes",
    "tab bar": "the effect style list (x 1134, y 2) covers the right end of the bar: its edge rows count here",
    "left panel": f"the capture's layer tree is another tree than the 18 default rows, and {_NO_DATA}: Panel/LeftLayers keeps the default rows",
    "right panel": f"the capture's inspector shows the selected frame (a menu covers part of it), and {_NO_DATA}: Panel/RightInspector draws the Page state",
    "ruler top strip": _RULERS, "ruler left strip": _RULERS,
    "popover": ("Popover/Panel draws the kind of this capture: the heat map shows its text and glyph edges, the numeric fields' icon prefixes (Input/PropertyField draws a letter prefix only) and "
                "the swatches' nearest theme tones, for the kinds that have them"),
    "menu": "the heat map shows " + _EDGES,
    "picker": ("Popover/ColorPicker draws the kind of this capture: the heat map shows the swatch row as filled squares: " + _SWATCHES + "; besides these it shows the text and glyph edges of its "
               "rows, the dotted contrast curve over the square (a recorded gap of the picker) and the rings of the handles"),
    "dialog": ("Dialog/Modal `Kind=Compare` draws the dialog's chrome (title, pane captions, summary, Close, Mark pill: the chrome mean is 2.508 without the two preview panes); the preview "
               "panes are the document's own render, which the screen does not draw"),
}
_STATE = {
    ("f-effect-styles", "menu"): (_EDGES + "; the capture's highlighted row carries a (23, 23, 23) fill where the render draws (10, 10, 10), read at (1384, 58) and (1146, 58), and its list "
                                  "draws a scroll thumb ((163, 163, 163) at (1432, 630)) where the render draws none; the group captions are not measured beyond the region mean"),
    ("f-picker-gradient", "picker"): ("Popover/ColorPicker `Kind=Gradient`: the heat map shows the swatch row as filled squares: " + _SWATCHES + "; besides these it shows the text and glyph "
                                      "edges of its rows, the rings of the handles and the Linear button's thin strokes (its box is 6 rows against the capture's 8)"),
    ("f-prototype-tab", "left panel"): ("the tree is drawn (`gap_left_dev_data`), and Tree/Row has no component look: the rows of components draw their names in the neutral text colour where "
                                        "production draws TEXT_COMPONENT"),
    ("f-prototype-tab", "popover"): ("Popover/Panel `Kind=Interaction` draws the details: the heat map shows its text and glyph edges"),
    ("f-type-details", "popover"): ("Popover/Panel `Kind=TypeSettings` draws the settings: the `Ag` preview glyph is 20 px where production draws 24 (no such text style); besides that tile the heat "
                                    "map shows text and glyph edges"),
    ("f-type-details", "toolbar"): "the type settings popover (x 888 to 1200, y 156 to 884) covers the right end of the toolbar: its bottom edge counts here",
    ("f-type-details", "left panel"): _LIST_SHIFT,
    ("edit-style-text", "left panel"): _LIST_SHIFT,
    ("f-picker", "left panel"): _SCROLLED,
    ("micro-inspector-math", "right panel"): _NO_FRAMEWORK + ", and 73 px high from the mode rows down",
    ("micro-dev-boxmodel", "right panel"): ("the component's playground preview is the document's own render, and the panel's strings are the file's own: the screen draws neutral names "
                                            "(`State=Component`)"),
    ("micro-dev-boxmodel", "left panel"): ("the Dev list draws neutral names and Studio's status text; its count in the tab, hover fills, scrollbar thumb, empty state and the Layers tab's own "
                                           "list are not drawn"),
    ("g-dev-compare", "right panel"): ("the capture's panel shows the Changed status (Compare changes, Mark ready) and no annotations or dev links; Panel/DevInspect `State=Inspect` draws the "
                                       "Ready for dev status, an annotation and a dev link, and no state holds the Changed status"),
}
_DEV_TOOLBAR = ("Navigation/BottomCanvasToolbar `State=Dev` draws the 266 px dock: its tool glyphs are 20 px where the capture's are 16 (#1427, the swapped glyph size) and the pill's two arrows "
                "(U+21A9, U+21E7) fall back to another font")
_TILE = "the worst tile lies in a region of this state that is pinned (its cause above)"
# the tab bar mean of the screens that draw the dirty tab's dot (`evidence screens`): the 13 editor captures, three Dev ones and the Compare dialog's, whose scrim dims the dot
_DOT_TAB = {**dict.fromkeys(("fill-d9", "f-picker-gradient", "f-edit-style-color", "edit-style-effect", "edit-style-grid", "edit-style-text", "effect-noise", "effect-glass", "effect-texture",
                             "grid-styles"), 0.3), **dict.fromkeys(("g-dev-react", "g-dev-annotate", "micro-dev-boxmodel"), 0.259), "g-dev-compare": 0.208}


def _cause(state, region):
    if region == "tab bar" and state in _DOT_TAB:
        dimmed = " (the scrim dims the dot)" if state == "g-dev-compare" else ""
        return (f"the dirty tab's dot is drawn (`Show unsaved dot`), so the tab bar mean is {_DOT_TAB[state]}{dimmed}: the cap is that mean rounded up to 0.01, with no margin, "
                "as a measured pin is, and a missing dot goes over it")
    if (state, region) in _STATE:
        return _STATE[(state, region)]
    if state == "micro-components-variables" and region in ("tab bar", "rail", "left panel", "ruler top strip", "ruler left strip"):
        return _FLOAT
    if state == "g-dev-compare" and region in ("rail", "ruler top strip", "ruler left strip", "toolbar", "tab bar"):
        return _SCRIM
    if state in _DEV and region == "toolbar":
        return _DEV_TOOLBAR
    return _REGION[region]


CAPS = {s: {**_DEFAULT, **pins} for s, pins in _PINS.items()}
WORST_TILE = {
    "f-effects-menu": 38.125,
    "f-picker": 39.17,
    "f-picker-gradient": 35.17,
    "f-prototype-tab": 36.7,
    "f-type-details": 38.2,
    "edit-style-text": 36.41,
    "grid-styles": 36.48,
    "micro-inspector-math": 40.06,
    "micro-typography-multiedit": 34.12,
    "micro-components-variables": 53.26,
    "micro-dev-boxmodel": 204.71,
    "g-dev-compare": 232.0,
}
CAUSES = {s: {**{r: _cause(s, r) for r in pins}, **({"worst tile": _TILE} if s in WORST_TILE else {})} for s, pins in _PINS.items()}
_SCROLL = "the named scroll area is the capture's own (the floating Variables window); no node of this screen has that rect"
_COMPARE_SCROLL = ("the compare dialog's list draws a scroll thumb, not the 8 x 160 track the capture names: the nearest node of that name is the Pages list's")
GEOMETRY = {
    "micro-components-variables": [
        {"key": "scrollbar-track", "layout": [634.0, 89.0, 8.0, 247.0], "node": [281.0, 137.0, 8.0, 160.0], "cause": _SCROLL},
    ],
    "g-dev-compare": [
        {"key": "scrollbar-track", "layout": [1102.0, 546.0, 8.0, 160.0], "node": [281.0, 137.0, 8.0, 160.0], "cause": _COMPARE_SCROLL},
    ],
}
