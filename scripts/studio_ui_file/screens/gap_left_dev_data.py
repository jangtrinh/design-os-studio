"""Per-screen data of the left panel's layer trees and of the Dev mode screens: plain dicts, no `screens()`: a screen module places the entries.

LEFT = {state: {"slots": {"Pages": [...], "Layers": [...]}, "overrides": [...]}} for every state of `LEFT_39` whose left panel draws a layer tree other than Panel/LeftLayers' default rows.
The entries are `place` entries (no x, y) of Tree/Row and List/Row. A tree is written as row codes, `<icon><depth>[>|v][*]`: the kind (`I` instance, `H` and `V` auto layout in columns and in rows,
`F` frame, `R` rectangle, `P` variant set, `C` component, `Y` polygon, `S` star, `E` ellipse, `G` grid), the depth, a closed (`>`) or open (`v`) chevron (none: no children) and `*` for the Selected
row. Names are neutral (`Frame 3`), the type label is the kind's own. Tree/Row has no component look (TEXT_COMPONENT), no look for a child of a selected row and no scroll offset, so a tree is
drawn from its first row at rest. States whose left panel is not a tree (`j-find`, `k-assets`, the open menu and modal) are not listed.
DEV = {state: {"patch": {component: {key: value}}, "add": [place entries]}}: what the Dev mode captures draw differently from their editor shell (the Dev toolbar and its place, the dev tab bar
parts, the right panel's state, the left list); `LEFT_TEXTS` the name rects the screens replace for the compare (window coordinates).
"""
from screens.gap_tree_text_fit import fit, name_left

ICONS = {"I": ("Instance", "Instance"), "H": ("AutoLayoutH", "Auto layout"), "V": ("AutoLayoutV", "Auto layout"), "F": ("Frame", "Frame"), "R": ("Rectangle", "Rect"),
         "P": ("Component", "Variant set"), "C": ("Component", "Component"), "Y": ("Polygon", "Polygon"), "S": ("Star", "Star"), "E": ("Ellipse", "Ellipse"), "G": ("AutoLayoutV", "Grid")}
NAMES = {"I": "Component", "H": "Frame", "V": "Frame", "F": "Frame", "R": "Rect", "P": "Variant set", "C": "State", "Y": "Shape", "S": "Shape", "E": "Shape", "G": "Grid"}
WINDOW_LAYERS = (49, 345)  # the Layers list's top-left in a window of the editor (screen coordinates)
FRAMES = ("VVHH" * 5)


def frames(pattern, n):
    return " ".join(f"{pattern[i % len(pattern)]}0>" for i in range(n))


TREES = {
    "default": "I0 I0 " + " ".join(f"{'V' if i in (3, 4, 7, 8) else 'H'}0>" for i in range(1, 17)),
    "selected": "I0 I0 H0> H0> V0> V0v V1v V2> V2> V2v V3v V4v H5> H5>* I4 V2> V2> V2>",
    "kit": "I0 P0>*",
    "frames": frames("VVHH", 18),
    "picker": frames("VHHV", 18),
    "text": frames("HHVV", 18),
    "prototype": "I0 P0> P0> P0v " + " ".join(f"C1>{'*' if i == 3 else ''}" for i in range(14)),
    "rects": "R0* R0* R0* I0 I0 " + frames("HHVV", 13),
    "scale": "F0>* I0 I0 " + frames("HHVV", 15),
    "polygon": "Y0* I0 I0 " + frames("HHVV", 15),
    "shapes": "S0 E0* I0 I0 " + frames("HHVV", 14),
    "autolayout": "H0>* I0 I0 " + frames("HHVV", 15),
    "grid": "G0v* R1 R1 R1 R1* I0 I0 " + frames("HHVV", 11),
    "math": "F0* I0 I0 " + frames("HHVV", 15),
    "typography": "F0v R1* F0v R1* F0v R1* I0 I0 " + frames("HHVV", 10),
    "history": "H0> F0> F0>",
}
STATE_TREE = {s: "selected" for s in ("d-editor-selected", "fill-d9", "f-picker-gradient", "f-edit-style-color", "edit-style-effect", "edit-style-grid", "f-stroke-settings", "effect-noise",
                                      "effect-glass", "effect-texture", "ai-image-menu", "ai-image-erase", "ai-image-expand", "j-check-designs", "micro-layer-picker")}
STATE_TREE.update({"kit-editor": "kit", "e-layers-selected": "frames", "f-picker": "picker", "j-comments": "picker", "j-cmdk": "picker", "edit-style-text": "text", "f-type-details": "text",
                   "f-prototype-tab": "prototype", "micro-smart-selection": "rects", "micro-selection-corner": "rects", "micro-scale-tool": "scale", "micro-vector-edit": "polygon",
                   "micro-shape-controls": "shapes", "micro-autolayout": "autolayout", "micro-css-grid": "grid", "micro-inspector-math": "math", "micro-typography-multiedit": "typography",
                   "j-version-history": "history", "k-variables-floating": "frames", "micro-components-variables": "frames"})


def parse(code):
    icon_key, rest = code[0], code[1:]
    selected, rest = rest.endswith("*"), rest.rstrip("*")
    chevron = rest[-1] if rest[-1] in ">v" else None
    return icon_key, int(rest.rstrip(">v")), chevron, selected


def tree_entries(tree):
    out = []
    for i, code in enumerate(TREES[tree].split()):
        key, depth, chevron, selected = parse(code)
        icon, label = ICONS[key]
        name, label = fit(f"{NAMES[key]} {i + 1}", label, depth)  # production's row rule: the label dropped where it does not fit, the name elided
        variant = {"Size": "Xs", "State": "Selected" if selected else "Default", "Depth": str(depth)}
        props = {"Name": name, "Type label": label, "Has Children": chevron is not None, "Icon": f"Icon/{icon}"}
        if chevron == "v":
            props["Chevron"] = "Icon/ChevronDown"
        out.append({"name": f"layer-{i}", "component": "Tree/Row", "variant": variant, "props": props})
    return out


def name_rects(tree, top=WINDOW_LAYERS[1]):
    """The layer-name rects of a tree in window coordinates: from 3 px left of where the name starts (the row's left edge is x 57) to the end of its room, one 32 px row each."""
    out = []
    for i, code in enumerate(TREES[tree].split()):
        key, depth, _chevron, _sel = parse(code)
        shown = fit(f"{NAMES[key]} {i + 1}", ICONS[key][1], depth)
        left = 57 + name_left(depth) - 3
        out.append(((int(left), top + 32 * i, int(57 + 176 - left) if not shown[1] else 82, 32), "layer name", shown[0]))
    return out


HISTORY_PAGES = [{"name": "page-0", "component": "List/Row", "variant": {"Size": "Xs", "State": "Selected"}, "props": {"Label": "Page 1"}},
                 {"name": "page-1", "component": "List/Row", "variant": {"Size": "Xs", "State": "Default"}, "props": {"Label": "Page 2"}}]
# The captures of these states are scrolled by a part of a row (the first row's centre sits at y 336 of the panel where a row at rest is centred at 329), so their rows do not line up with the
# slot's rows at rest: drawn from the top, a tree scores worse than the default rows (measured), and the state keeps them.
SCROLLED = ("e-layers-selected", "f-picker", "j-comments", "j-cmdk", "k-variables-floating", "micro-components-variables")
LEFT = {state: {"slots": {"Layers": tree_entries(tree)}} for state, tree in STATE_TREE.items() if tree != "default" and state not in SCROLLED}
LEFT["j-version-history"] = {"slots": {"Pages": HISTORY_PAGES, "Layers": tree_entries("history")}, "overrides": [{"path": "scrollbar-track", "visible": False}]}
LEFT_TEXTS = {state: name_rects(STATE_TREE[state]) for state in LEFT}


# ---- Dev mode: the four captures draw the editor shell with the Dev toolbar, the dev tab bar parts, the right panel's state and the left list.
# `patch` {component: {key: value}} edits the shell's placed instance of that record; `add` entries are placed right after Panel/LeftLayers (the list covers its Layers part).
LIST_AT = (49, 305)  # the left panel below the Pages list and its separator (panels/left.rs:135)
DEV_TOOLBAR_AT = (587, 800)  # centred on the window: the 266 px dock 48 high and the pill 40 above it (toolbar.rs:380-398)
TAB_PROPS = {"Show dev tile": True, "Show unsaved dot": True}
COUNT_ONE = [{"path": "left-dev-ready-section/Ready count/Count", "text": "1"}, {"path": "Page row/Page count", "text": "1"}]  # the three captures read 1; the boxmodel's list reads 78
LIST_ONE = COUNT_ONE + [{"path": "Selected row", "visible": True}] + [{"path": f"Row {i}", "visible": False} for i in range(2, 16)]
LIST_ENTRY = {"name": "Panel/DevInspect List", "component": "Panel/DevInspect", "variant": {"State": "List"}, "x": LIST_AT[0], "y": LIST_AT[1]}


def dev_patch(inspector, toolbar="Dev", list_overrides=()):
    return {"patch": {"Navigation/TopTabBar": {"props": dict(TAB_PROPS)},
                      "Navigation/BottomCanvasToolbar": {"variant": {"State": toolbar}, "x": DEV_TOOLBAR_AT[0], "y": DEV_TOOLBAR_AT[1]},
                      "Panel/DevInspect": {"variant": {"State": inspector}}},
            "add": [dict(LIST_ENTRY, overrides=list(list_overrides))]}


DEV = {"g-dev-react": dev_patch("Inspect", list_overrides=LIST_ONE), "g-dev-annotate": dev_patch("Annotate", "DevAnnotate", LIST_ONE),
       "g-dev-compare": dev_patch("Inspect", list_overrides=LIST_ONE), "micro-dev-boxmodel": dev_patch("Component")}
