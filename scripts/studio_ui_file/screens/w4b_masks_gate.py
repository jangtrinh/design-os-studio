"""Gate data of the W4B screens, the comparison side: per state (a capture of Inspector, styles and Dev mode), what the screen gate leaves out, keeps and reports by.

    TEXTS    the kit strings the screen replaces with neutral ones: the file name, the tab title, the five page names and, outside Dev mode, the eighteen layer names of the editor shell
             (a Dev mode list has other rows: its one kit row, the section, is masked instead), plus the style names of `grid-styles` and `f-effect-styles`
    CONTENT  the canvas content (the document's, not chrome) and the toolbar rect that stays in: the Dev mode toolbar is its own 266 px rect
    REGIONS  the editor regions at the editor-default rects, the Dev toolbar where the capture draws it, and the capture's overlay (`popover`, `menu`, `picker`, `dialog`)
    KEEP     the overlay rect stays measured (a popover over the canvas is chrome); in Dev mode so does the floating re-centre pill, which no record draws (a measured gap)
    SCRIMS   `g-dev-compare` draws the app's scrim: dev/compare.rs:97 `Color32::from_black_alpha(140)`, a literal no theme constant holds
"""
import json
import pathlib

# the rects the wired data modules imply (gap_wired_texts.py writes the file): a state's kit strings the screen replaces with neutral ones, added to this module's own
_WIRED = {state: [(tuple(r[:4]), r[4], r[5]) for r in rects] for state, rects in json.loads(pathlib.Path(__file__).with_name("gap_wired_texts.json").read_text(encoding="utf-8")).items()}
_STRIP, _CANVAS, _TOOLBAR = 20, (290, 32, 909, 868), (464, 840, 512, 48)
_DEV_TOOLBAR, _PILL = (587, 840, 266, 48), (655, 800, 131, 32)  # g-dev-react: `dev-mode-toggle` (809, 848, 36, 32) sits inside the green toolbar; `dev-recenter`
_CONTENT = (_CANVAS[0] + _STRIP, _CANVAS[1] + _STRIP, _CANVAS[2] - _STRIP, _CANVAS[3] - _STRIP)
_BASE = {"tab bar": (0, 0, 1440, 32), "rail": (0, 32, 49, 868), "left panel": (49, 32, 241, 868), "right panel": (1199, 32, 241, 868),
         "ruler top strip": (310, 32, 889, 20), "ruler left strip": (290, 52, 20, 848), "toolbar": _TOOLBAR}
_DEV = ("g-dev-react", "g-dev-annotate", "g-dev-compare", "micro-dev-boxmodel")
_OVERLAYS = {
    "f-edit-style-color": {"popover": (960, 412, 240, 184)},
    "f-effect-styles": {"menu": (1134, 2, 304, 896)},
    "f-effects-menu": {"menu": (1264, 571, 168, 289)},
    "f-picker": {"picker": (960, 346, 240, 538)},
    "f-picker-gradient": {"picker": (960, 332, 240, 552)},
    "f-prototype-tab": {"popover": (952, 105, 240, 306)},
    "f-stroke-settings": {"popover": (960, 668, 240, 216)},
    "f-type-details": {"popover": (888, 156, 312, 728)},
    "edit-style-effect": {"popover": (960, 412, 240, 344)},
    "edit-style-grid": {"popover": (960, 412, 240, 344)},
    "edit-style-text": {"popover": (960, 412, 240, 280)},
    "effect-glass": {"popover": (960, 636, 240, 248)},
    "effect-noise": {"popover": (960, 668, 240, 216)},
    "effect-texture": {"popover": (960, 732, 240, 152)},
    "grid-styles": {"menu": (1220, 417, 184, 481)},
    "g-dev-compare": {"dialog": (310, 130, 818, 638)},
}
_STATES = (
    'f-edit-style-color',
    'f-effect-styles',
    'f-effects-menu',
    'f-picker',
    'f-picker-gradient',
    'f-prototype-tab',
    'f-stroke-settings',
    'f-type-details',
    'fill-d9',
    'edit-style-effect',
    'edit-style-grid',
    'edit-style-text',
    'effect-glass',
    'effect-noise',
    'effect-texture',
    'grid-styles',
    'micro-inspector-math',
    'micro-typography-multiedit',
    'micro-components-variables',
    'micro-dev-boxmodel',
    'g-dev-annotate',
    'g-dev-compare',
    'g-dev-react',
)
_PAGES = ("Page 1", "Page 2", "——————", "SECTION", "↳ Page 3")
# the project tab title stops where the dirty tab's dot starts: 185 = 301 - 116 (x of the layout's `tab-unsaved-dot-1`)
_SHELL = [((57, 40, 184, 24), "file name", "Design file"), ((116, 8, 185, 16), "project tab title", "Design file")]
_SHELL += [((61, 137 + 32 * i, 212, 32), "page name", n) for i, n in enumerate(_PAGES)]
_LAYERS = [((100, 345 + 32 * i, 82, 32), "layer name", n) for i, n in enumerate(["Component 1", "Component 2"] + [f"Frame {i}" for i in range(1, 17)])]
_SECTION = [((80, 405, 118, 22), "kit section name in the Dev mode list", "Section")]
_STYLES = {"grid-styles": [((1250, 425 + 32 * i, 130, 28), "grid style name", f"Grid style {i + 1}") for i in range(3)],
           "f-effect-styles": [((1160, 10 + 32 * i, 260, 28), "effect style or group name", "Style / Group") for i in range(28)]}

TEXTS = {s: _SHELL + (_SECTION if s in _DEV else _LAYERS) + _STYLES.get(s, []) + _WIRED.get(s, []) for s in _STATES}
CONTENT = {s: (_CONTENT, _DEV_TOOLBAR if s in _DEV else _TOOLBAR) for s in _STATES}
REGIONS = {s: {**_BASE, **({"toolbar": _DEV_TOOLBAR} if s in _DEV else {}), **_OVERLAYS.get(s, {})} for s in _STATES}
KEEP = {s: ([_PILL] if s in _DEV else []) + list(_OVERLAYS.get(s, {}).values()) for s in _STATES if s in _DEV or s in _OVERLAYS}
SCRIMS = {"g-dev-compare": {"variable": None, "reason": "dev/compare.rs:97 `Color32::from_black_alpha(140)`: black at 140 / 255, no theme constant"}}
