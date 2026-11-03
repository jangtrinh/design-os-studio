"""A text placed in a box of the painter's rect, as one node where it can be one. Not a component module (no `component_sets`).

`regions_sidepanels_parts.label` wraps every text in a fixed frame so a painter's `Align2` can centre or right-align it: two nodes per text. A text that is left-aligned in a box
needs no frame: its line box sits at the box's top plus half of what the box has over the line (the frame's `center` alignment, written out), so the text is one node at the same
pixel. The dialog, install, publish and compare modules use it for every start-aligned text, and keep the frame for the centred and right-aligned ones (a hug text cannot align itself).
The page-03 size is the node count, so this halves those texts.
"""
from components.regions_sidepanels_parts import label
from rest_ir import T

LINE = {"Caption": "LINE_CAPTION", "Body": "LINE_BODY", "Mono": "LINE_CAPTION", "Large": "LINE_TEXT_SM", "Text sm": "LINE_TEXT_SM", "Text xs": "LINE_TEXT_XS", "Text md": "LINE_TEXT_MD",
        "Text xl": "LINE_TEXT_XL", "Title": "LINE_TEXT_XL"}  # text_styles_plan.ROLES: the line height variable of each role


def text_at(name, rect, content, style, colour, env, align=("start", "center"), **kw):
    """The text in `rect` (x, y, w, h): one T for a start-aligned line (centred vertically as the frame would), the `label` frame for any other alignment."""
    if align[0] != "start" or align[1] != "center":
        return label(name, rect, content, style, colour, env, align=align, **kw)
    x, y, w, h = rect
    line = env.num(LINE[style.split(" / ")[0]])
    return T(name, content, style, env.c(colour), abs=(x, y + (h - line) / 2, w, line))
