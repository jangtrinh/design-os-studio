"""What a screen comparison leaves out, per capture (`state` of the screen spec), and the regions it reports by.

`CONTENT`    the document's own content, not chrome: the capture's canvas between the ruler strips, minus the strips and the floating toolbar. A kit's frames in the capture, nothing in the
             screen.
`TEXTS`      the texts the screen replaces with neutral content (a kit's names; the chrome rects stay 1:1): (rect, what, the neutral text the screen draws). The rect is the box that holds the
             text, wide enough for the capture's longer string. A string Studio itself draws (a type label, a weight, a size, `Pages`) is never masked.
`REGIONS`    the named rects whose numbers are reported next to the whole screen's.
`KEEP`       rects inside the content area that stay measured: a dialog or popover over the canvas is chrome, not the document's content.

The d-editor-default rows are here; every other state is declared by its batch (`screens/<batch>_gate.py`, studio_ui_gate_screen_data: TEXTS, CONTENT, REGIONS, KEEP), merged below. `CONTENT[state]`
is the (content rect, toolbar rect) pair, or None when the state has no canvas content; a state with no `TEXTS` or `REGIONS` row raises, so a screen is never compared with no mask decided.

    exclusions(state)    every rect the comparison leaves out for a capture: CONTENT minus KEEP + the replaced texts (a kept rect never unmasks a replaced text)
    regions(state)       {region name: rect}
"""
import pathlib
import sys

_SCRIPTS = str(pathlib.Path(__file__).resolve().parents[2])
if _SCRIPTS not in sys.path:
    sys.path.insert(0, _SCRIPTS)
import studio_ui_gate_screen_data as gate_data  # noqa: E402

STRIP = 20
CANVAS = (290, 32, 909, 868)
TOOLBAR = (464, 840, 512, 48)
PAGES = ("Page 1", "Page 2", "——————", "SECTION", "↳ Page 3")


def _texts_editor_default():
    # the project tab title stops where the dirty tab's dot starts: 185 = 301 - 116 (x of the layout's `tab-unsaved-dot-1`)
    out = [((57, 40, 184, 24), "file name", "Design file"), ((116, 8, 185, 16), "project tab title", "Design file")]
    out += [((61, 137 + 32 * i, 212, 32), "page name", name) for i, name in enumerate(PAGES)]
    names = ["Component 1", "Component 2"] + [f"Frame {i}" for i in range(1, 17)]
    out += [((100, 345 + 32 * i, 82, 32), "layer name", n) for i, n in enumerate(names)]  # the name column only: the kind icon, the chevron and the type label are Studio's own and count
    out += [((1235, 281 + 160 * k, 110, 32), "text style folder", f"Heading {k + 1}") for k in range(4)]  # the style names and sizes are generic (weights, a type ramp) and count
    return out


_BASE_TEXTS = {"d-editor-default": _texts_editor_default()}
_BASE_CONTENT = {"d-editor-default": ((CANVAS[0] + STRIP, CANVAS[1] + STRIP, CANVAS[2] - STRIP, CANVAS[3] - STRIP), TOOLBAR)}  # below and right of the ruler strips, and the toolbar
_BASE_REGIONS = {"d-editor-default": {"tab bar": (0, 0, 1440, 32), "rail": (0, 32, 49, 868), "left panel": (49, 32, 241, 868), "right panel": (1199, 32, 241, 868),
                                 "ruler top strip": (310, 32, 889, 20), "ruler left strip": (290, 52, 20, 848), "toolbar": TOOLBAR}}
_DATA = gate_data.load()
TEXTS, CONTENT = gate_data.merge(_BASE_TEXTS, _DATA, "TEXTS"), gate_data.merge(_BASE_CONTENT, _DATA, "CONTENT")
REGIONS, KEEP = gate_data.merge(_BASE_REGIONS, _DATA, "REGIONS"), _DATA["KEEP"]


def _minus(rect, keep):
    """`rect` without `keep`: up to four rects (above, below, left, right of the overlap); `rect` itself when they do not overlap."""
    (x, y, w, h), (kx, ky, kw, kh) = rect, keep
    x0, y0, x1, y1 = max(x, kx), max(y, ky), min(x + w, kx + kw), min(y + h, ky + kh)
    if x0 >= x1 or y0 >= y1:
        return [rect]
    parts = [(x, y, w, y0 - y), (x, y1, w, y + h - y1), (x, y0, x0 - x, y1 - y0), (x1, y0, x + w - x1, y1 - y0)]
    return [r for r in parts if r[2] > 0 and r[3] > 0]


def exclusions(state):
    """The rects left out of the comparison: the canvas content around the toolbar (the toolbar is chrome and stays in) minus the kept rects, and every replaced text."""
    content, around = CONTENT[state], []
    if content is not None:
        (x, y, w, h), (tx, ty, tw, th) = content
        around = [(x, y, tx - x, h), (tx + tw, y, x + w - (tx + tw), h), (tx, y, tw, ty - y), (tx, ty + th, tw, y + h - (ty + th))]
        for keep in KEEP.get(state, ()):
            around = [piece for r in around for piece in _minus(r, keep)]
    return around + [r for r, _, _ in TEXTS[state]]


def regions(state):
    return REGIONS[state]
