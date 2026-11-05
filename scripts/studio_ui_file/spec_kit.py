"""NodeSpec builders for apply_spec. Every colour, radius, gap, padding and stroke is a `var:` ref to a
Semantic variable by `Collection/name`; no literal value is passed for those properties.
"""
SEM = "Semantic"


def V(name):
    """`var:` reference to a Semantic variable, e.g. V("color/bg-secondary")."""
    return f"var:{SEM}/{name}"


def text(name, content, style="Mono / Value", color="color/text", width=None):
    s = {"kind": "text", "name": name, "text": content, "text_style": style, "text_color": V(color)}
    if width:
        s["width"] = width
    return s


def box(name, kind="frame", fill=None, radius=None, stroke=None, stroke_weight="size/border-width", width=None, height=None,
        direction=None, gap=None, pad=None, children=None, hug=False, align=None, **extra):
    """Frame/rectangle/section spec. fill=None -> transparent (explicit `none`, never the default white).

    pad: None | one variable name (all sides) | 4 entries [top, right, bottom, left]; an entry is a variable name, None (0) or a
    literal number (only for a recorded gate allow-list exception).
    """
    s = {"kind": kind, "name": name, "fill": V(fill) if fill else "none"}
    if radius:
        s["corner_radius"] = V(radius)
    if stroke:
        s["stroke"], s["stroke_weight"] = V(stroke), V(stroke_weight)
    if width is not None:
        s["width"] = width
    if height is not None:
        s["height"] = height
    if direction:
        layout = {"direction": direction, **({"counter_align": align} if align else {})}
        if gap:
            layout["gap"] = V(gap)
        if pad:
            layout["padding"] = [V(p) if isinstance(p, str) else (p or 0) for p in (pad if isinstance(pad, list) else [pad] * 4)]
        s["layout"] = layout
    if hug:
        s["width_sizing"], s["height_sizing"] = "hug", "hug"
    if children:
        s["children"] = children
    s.update(extra)
    return s


def column(name, children, gap="space/inline", pad=None, **kw):
    return box(name, direction="column", gap=gap, pad=pad, children=children, hug=True, **kw)


def row(name, children, gap="space/inline", pad=None, **kw):
    return box(name, direction="row", gap=gap, pad=pad, children=children, hug=True, **kw)


def section(name, content, x, y, width, height):
    """Dark section holding one content column at the 200 px section padding (VSF canvas rule)."""
    content = dict(content, x=200, y=200)
    return box(name, kind="section", fill="color/bg", width=width, height=height, x=x, y=y, children=[content])
