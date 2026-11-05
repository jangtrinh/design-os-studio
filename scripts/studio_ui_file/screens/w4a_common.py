"""The editor shell the Home and editor-shell screens place, and the spec every one of them is built through.

`shell()` is the six regions of `Screen/EditorDesign` at the rects the first capture gives them (a copy: a screen may drop or add an entry); `screen(case, place)` is the spec of one capture:
the regions then draw the state that capture shows (`gap_wiring.regions`: the right inspector, the layer tree, the active tool). Menus, popovers and the other overlays are the data modules'
(`gap_wiring.overlays`).
"""
from screens import editor_design as ed
from screens import gap_wiring


def shell():
    """The six regions of `Screen/EditorDesign` at the rects the first capture gives them (a copy: a screen may drop or add an entry)."""
    return [dict(p) for p in ed.screens()[0]["place"]]


def screen(case, place, flow="EditorDesign", canvas=True, **extra):
    """One state screen: the editor window with the canvas colour behind the document, or (canvas False) none."""
    s = {"name": f"Screen/{flow}/{case}", "w": 1440, "h": 900, "state": case, "aliases": ed.ALIASES, "canvas": (*ed.CANVAS, "#F5F5F5", "Canvas background") if canvas else None, "place": place}
    s.update(extra)
    return gap_wiring.regions(s)
