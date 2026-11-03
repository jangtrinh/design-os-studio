"""Component IR: plain dicts that name Semantic variables instead of holding values. `rest_lower` turns one into Figma REST
JSON plus the bind intents the native post-pass applies; a component module never writes a literal for a bound property.

    F(name, kids, dir="H"|"V"|None, gap=, pad=, fill=, stroke=, sw=, radius=, w=, h=, align=("center", "center"),
      sx="hug", sy="hug", abs=(x, y, w, h), alpha=1.0, visible_prop=, slot=)
                                                                        frame (auto layout when dir is set); `slot` = a SLOT property: the frame is that property's
                                                                        container (its children are the default content)
    T(name, content, style, color, prop="Label", alpha=1.0)             text; `style` is a text style name, `prop` a TEXT property
    I(name, icon, size, color, swap="Icon", visible_prop=, alpha=1.0)   nested `Icon/<icon>` instance, glyph fill = `color`
    Rest of the keywords are in the docstring of each function. Names of variables are `group/name` Semantic names.

    E(name, w, h, fill=, stroke=, sw=)                                  ellipse (a disc or a ring, fixed size)
    F(..., sa="inside"|"center"|"outside")                              stroke alignment (default inside; a centred stroke is egui's `circle_stroke`)
    T(..., wrap=True, w=)                                               a text that wraps at width `w` (textAutoResize HEIGHT); size it with sx="fill"/"fixed"
    T(..., align_h="left"|"center"|"right"|"justified")                 the lines' horizontal alignment inside the text box (textAlignHorizontal); none means left
    T(..., rot=-90, abs=(x, y, w, h))                                   a quarter turn counter-clockwise about the text box's top-left corner (egui `with_angle(-FRAC_PI_2)`): the
                                                                        text reads up the page from (x, y), its tops facing left; `w`, `h` are the unrotated box. REST `relativeTransform`;
                                                                        the text must be an absolutely placed child (`abs`)
    X(..., radius=lit(...))                                             the ring instance's own corner radius: host radius + SPACE_FACT, default the main's
    V(name, path, w, h, fill)                                           vector from an SVG path in a w x h box, solid fill
    V(name, path, w, h, None, stroke=, sw=, cap=, join=, sa=)           an open path drawn as a stroke (probed on 0.7.5: no fill, round or butt caps, miter, bevel or round joins), the stroke centred
                                                                        on the path unless `sa` says otherwise: egui's `line_segment`s of a chevron or an arrow
    F(..., stroke=, sw=[top, right, bottom, left])                      a stroke on some sides only: each entry a Semantic name, a lit(...) or None (no stroke there); binds `stroke<Side>Weight`
    X(name, set, variant, abs=(x, y, w, h))                             nested INSTANCE of a childless variant (focus ring)
    X(name, set, variant, props={...}, sx=, sy=, w=, h=, abs=None)      nested INSTANCE of a variant that has children, in auto-layout flow (or absolute with
                                                                        `abs`). The importer expands it from its main; `props` sets the nested set's TEXT (str),
                                                                        BOOLEAN (bool) and INSTANCE_SWAP (an Icon name) values by property name.
                                                                        `sx` / `sy` (hug, fixed, fill) and `w` / `h` (a fixed size in px, which implies fixed) size the COPY, as the editor's
                                                                        `set_sizing` does: Fill is layout_child Fill with the copy's own sizing Fixed, Fixed both Fixed,
                                                                        Hug layout_child Fixed with own Hug; a copy that asks for its main's own modes changes nothing.
                                                                        `slots=` (content for the copy's Slot) is refused: 0.7.5 has no working route (see the error).
                                                                        Nest no variant that holds a Focus ring; draw focus on the composite itself:
                                                                        the importer clones the main's ring at its estimated size and nothing resizes the copy.
    F(..., alpha=lit(0.22, reason))                                     a paint opacity of its own on the one `fill` (`DEV_READY.gamma_multiply(0.22)`): the fill stays bound, the
                                                                        opacity is a literal the gate allow-lists as `fills/0/opacity`; a plain 0.5 is the Disabled half alpha
    lit(value, reason)                                                  a literal for a bound property that no Semantic variable
                                                                        owns: the build records it in the gate allow-list with `reason`
    F(..., fills=[grad(stops, start=(0, .5), end=(1, .5)), ...])        gradient paints, bottom first, instead of `fill`: a linear gradient between two points
                                                                        of the node's unit box; each stop is `(position, (r, g, b, a), reason)`, a literal (0..255
                                                                        channels) with the reason no variable holds it, recorded per stop in the allow-list
    F(..., radius=[tl, tr, br, bl])                                     per-corner radii: each entry a Semantic name, a lit(...) or None (a square corner, bound
                                                                        and recorded as nothing); written like a uniform radius, one binding or entry per rounded corner

`pad` is a variable name (all four sides) or four entries [top, right, bottom, left]; an entry is a name or None (0).
"""
import copy
import math

from rest_values import check_alpha, grad, lit, segments_path  # noqa: F401  (re-exported: the value constructors a module imports with F, T, X)

SIZING = ("hug", "fixed", "fill")
STROKE_ALIGN = ("inside", "center", "outside")
STROKE_ALIGN_REST = {"inside": "INSIDE", "center": "CENTER", "outside": "OUTSIDE"}
CAPS, JOINS = ("none", "round", "square"), ("miter", "bevel", "round")


def F(name, kids=(), **kw):
    for k in ("sx", "sy"):
        if kw.get(k, "hug") not in SIZING:
            raise ValueError(f"{name}: {k} must be one of {SIZING}")
    if isinstance(kw.get("radius"), list):
        _corners(name, kw["radius"])
    if kw.get("fills") and kw.get("fill"):
        raise ValueError(f"{name}: give `fill` (one Semantic colour) or `fills` (gradient paints), not both")
    if kw.get("sa", "inside") not in STROKE_ALIGN:
        raise ValueError(f"{name}: sa (stroke alignment) must be one of {STROKE_ALIGN}")
    check_alpha(name, kw)
    if isinstance(kw.get("sw"), list):
        _sides(name, kw["sw"], kw.get("stroke"))
    return {"t": "F", "name": name, "kids": list(kids), **kw}


def _sides(name, sw, stroke):
    """A per-side stroke weight is four entries in REST order (top, right, bottom, left): a Semantic name, a `lit(...)` that is not 0, or None (no stroke on that side); at least one side has one."""
    if not stroke:
        raise ValueError(f"{name}: per-side stroke weights need a `stroke` colour")
    if len(sw) != 4 or all(w is None for w in sw):
        raise ValueError(f"{name}: sw as a list is [top, right, bottom, left]: four entries and one side at least, got {sw!r}")
    for w in sw:
        if not (w is None or (isinstance(w, str) and w) or (isinstance(w, tuple) and w[:1] == ("lit",) and w[1] != 0)):
            raise ValueError(f"{name}: a side is a Semantic name, a lit(...) other than 0, or None, got {w!r}")


def _corners(name, radius):
    """A per-corner radius is four entries in REST order (top left, top right, bottom right, bottom left): a Semantic name, a `lit(...)` that is not 0, or None for a square corner; one square
    corner at least stays square or round, so a uniform radius is written as a name, not as four."""
    if len(radius) != 4:
        raise ValueError(f"{name}: radius as a list is [top left, top right, bottom right, bottom left]: four entries, got {len(radius)}")
    for c in radius:
        if not (c is None or (isinstance(c, str) and c) or (isinstance(c, tuple) and c[:1] == ("lit",) and c[1] != 0)):
            raise ValueError(f"{name}: a corner is a Semantic name, a lit(...) other than 0, or None (a square corner), got {c!r}")
    if all(c is None for c in radius):
        raise ValueError(f"{name}: every corner is square: leave `radius` out")


ALIGN_H = {"left": "LEFT", "center": "CENTER", "right": "RIGHT", "justified": "JUSTIFIED"}


def T(name, content, style, color, **kw):
    if "align_h" in kw and kw["align_h"] not in ALIGN_H:
        raise ValueError(f"{name}: align_h must be one of {tuple(ALIGN_H)}, got {kw['align_h']!r}")
    if "rot" in kw and (kw["rot"] is True or kw["rot"] != -90):
        raise ValueError(f"{name}: rot is -90 (a quarter turn counter-clockwise) or absent, got {kw['rot']!r}")
    if kw.get("wrap") and not kw.get("w"):
        raise ValueError(f"{name}: a wrapping text needs the width `w` it wraps at (its estimate and its first layout)")
    return {"t": "T", "name": name, "text": content, "style": style, "color": color, **kw}


def I(name, icon, size, color, **kw):
    return {"t": "I", "name": name, "icon": icon, "size": size, "color": color, **kw}


def E(name, w, h, **kw):
    return {"t": "E", "name": name, "w": w, "h": h, "kids": [], **kw}


def V(name, path, w, h, fill=None, **kw):
    if kw.get("cap", "none") not in CAPS or kw.get("join", "miter") not in JOINS:
        raise ValueError(f"{name}: cap is one of {CAPS} and join one of {JOINS}")
    return {"t": "V", "name": name, "path": path, "w": w, "h": h, "fill": fill, "kids": [], **kw}


def X(name, set_name, variant, props=None, **kw):
    if props is not None and not isinstance(props, dict):
        raise ValueError(f"{name}: props must be a dict of property name -> value")
    for size, sizing in (("w", "sx"), ("h", "sy")):
        if size in kw and (not isinstance(kw[size], (int, float)) or isinstance(kw[size], bool) or kw[size] <= 0):
            raise ValueError(f"{name}: {size} is the copy's own fixed size in px, a positive number, got {kw[size]!r}")
        if size in kw and kw.get(sizing, "fixed") != "fixed":
            raise ValueError(f"{name}: {size}= is a fixed size, so {sizing}={kw[sizing]!r} contradicts it")
    if "slots" in kw:
        raise ValueError(f"{name}: slots= cannot fill a nested copy's Slot on designos-mcp 0.7.5: REST gives an instance no partial content, "
                         "and the batch op `set_slot` builds into a scratch copy of the document and drops the command (probed: ok, nothing created). "
                         "A screen fills a Slot with `apply_spec` `slots` on the instance it places.")
    if "radius" in kw and not (isinstance(kw["radius"], tuple) and kw["radius"][0] == "lit"):
        raise ValueError(f"{name}: a ring instance pins its radius as a lit(...) literal: host radius + SPACE_FACT is a sum no variable holds")
    return {"t": "X", "name": name, "set": set_name, "variant": variant, "props": dict(props or {}), **kw}


def pads(node):
    p = node.get("pad")
    return [p] * 4 if isinstance(p, str) else list(p or [None] * 4)


def walk(node):
    yield node
    for k in node.get("kids", []):
        yield from walk(k)


def estimate(node, text_w=lambda s, size: 0.56 * size * len(s), mains=None):
    """(w, h) of a node before layout: fixed sizes as given, hug sizes from the children. Only a first guess: the engine lays
    the real widths out after import (see library_layout). `mains` maps (set name, variant name) to the size of an already lowered
    main: a nested instance without `abs` is as big as its main."""
    if node.get("abs"):
        return node["abs"][2], node["abs"][3]
    if node["t"] in ("E", "V"):
        return node["w"], node["h"]
    if node["t"] == "X":
        where = f"{node['name']}: instance of {node['set']} / {node['variant']}"
        if mains is None:
            raise KeyError(f"{where} needs the sizes of the lowered mains: call `env.estimate(node)` (or pass `mains=`), or give the instance `abs`")
        if (node["set"], node["variant"]) not in mains:
            raise KeyError(f"{where} needs `abs` or a main lowered before it (sets are lowered page 02 rows, then page 03 rows, in order: put the nested set in an earlier row)")
        main_w, main_h = mains[(node["set"], node["variant"])]
        return node.get("w", main_w), node.get("h", main_h)  # `w` / `h`: the copy's own fixed size
    if node["t"] == "I":
        return node["size"], node["size"]
    if node["t"] == "T":
        size = node.get("size", 14)
        line = node.get("line", round(size * 1.4))
        if node.get("wrap"):
            return node["w"], max(1, math.ceil(text_w(node["text"], size) / node["w"])) * line
        return text_w(node["text"], size), line
    kids = [k for k in node["kids"] if not k.get("abs") and not _hidden(k)]
    sizes = [estimate(k, text_w, mains) for k in kids]
    pt, pr, pb, pl = (0 if p is None else 1 for p in pads(node))
    d = node.get("dir")
    gap = 1 if node.get("gap") else 0
    px, py = (pl + pr) * 12, (pt + pb) * 6
    if d == "H":
        w, h = sum(s[0] for s in sizes) + gap * 4 * max(len(sizes) - 1, 0) + px, max([s[1] for s in sizes] or [0]) + py
    elif d == "V":
        w, h = max([s[0] for s in sizes] or [0]) + px, sum(s[1] for s in sizes) + gap * 4 * max(len(sizes) - 1, 0) + py
    else:
        w, h = max([s[0] for s in sizes] or [0]), max([s[1] for s in sizes] or [0])
    return node.get("w", w), node.get("h", h)


def _hidden(node):
    return node.get("hidden", False)


def clone(node):
    return copy.deepcopy(node)
